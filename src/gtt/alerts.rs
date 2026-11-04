//! GTT's service alerts (Phase 3, decision O): read every few minutes, linked to stops and
//! lines, kept in memory.
//!
//! An alert names the stops it concerns by GTT's `stop_id` and the lines by GTT's `route_id`.
//! A line alert ("Linea 9 deviata") lists nearly every stop of its line; a stop alert
//! ("Fermata 750 sospesa") lists one stop and every line serving it. So an alert is linked by
//! stop when it lists stops, and by line only when it lists none (or when the static data that
//! maps GTT's stop ids is missing): linked by line, a stop alert would show at every stop of a
//! dozen lines.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::Duration,
};

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use tokio::sync::Notify;

use super::{
    GttRealtime,
    proto::{self, TranslatedString},
    static_data::GttStatic,
};
use crate::{
    stops::{IndexError, LineRef, StopIndex, natural_cmp},
    time::{Clock, TIME_ZONE},
    upstream::UpstreamError,
};

/// Lines are named from the stop index of this feed.
const FEED: &str = "gtt";
const LANGUAGE: &str = "it";

/// An alert as the API returns it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Alert {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    /// GTT's page about it; only `http` and `https` links are kept.
    pub url: Option<String>,
    /// The lines it names, sorted; empty when it names none.
    pub lines: Vec<LineRef>,
    /// The active period that includes now; `None` when open-ended.
    pub start: Option<DateTime<Tz>>,
    pub end: Option<DateTime<Tz>>,
}

impl Alert {
    /// Whether it names the line `name` (`4`, `68+`), ignoring case.
    pub fn names_line(&self, name: &str) -> bool {
        self.lines
            .iter()
            .any(|line| line.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Period {
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
}

impl Period {
    fn contains(&self, now: DateTime<Utc>) -> bool {
        self.start.is_none_or(|start| start <= now) && self.end.is_none_or(|end| now <= end)
    }
}

#[derive(Debug)]
struct Entry {
    alert: Alert,
    /// Empty means always active.
    periods: Vec<Period>,
    /// Linked by line, to every stop of its lines, instead of by stop.
    by_line: bool,
}

impl Entry {
    /// The alert with its current period, when it is active at `now`.
    fn active(&self, now: DateTime<Utc>) -> Option<Alert> {
        let period = if self.periods.is_empty() {
            Period {
                start: None,
                end: None,
            }
        } else {
            *self.periods.iter().find(|period| period.contains(now))?
        };
        let local = |at: Option<DateTime<Utc>>| at.map(|at| at.with_timezone(&TIME_ZONE));
        Some(Alert {
            start: local(period.start),
            end: local(period.end),
            ..self.alert.clone()
        })
    }
}

/// The alerts of one read of the feed, linked to stops and lines.
#[derive(Debug, Default)]
pub struct Alerts {
    entries: Vec<Entry>,
    /// Our stop id → the entries linked to it by stop.
    by_stop: HashMap<String, Vec<usize>>,
    summary: Option<AlertsSummary>,
}

/// What the last read found, for `/api/status`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AlertsSummary {
    pub read_at: DateTime<Utc>,
    /// Alerts in the feed, active or not.
    pub alerts: usize,
    /// GTT stop ids named by alerts but missing from the static data (temporary stops, or
    /// static data not downloaded yet).
    pub unknown_stops: usize,
}

impl Alerts {
    /// `stop_ids` maps GTT's stop ids to ours (empty without static data: every alert is then
    /// linked by line); `lines` names GTT's routes.
    pub fn from_feed(
        feed: proto::FeedMessage,
        stop_ids: &HashMap<String, String>,
        lines: &[LineRef],
        read_at: DateTime<Utc>,
    ) -> Self {
        let lines: HashMap<&str, &LineRef> =
            lines.iter().map(|line| (line.id.as_str(), line)).collect();
        let mut unknown_stops = HashSet::new();
        let mut entries = Vec::new();
        let mut by_stop: HashMap<String, Vec<usize>> = HashMap::new();

        for entity in feed.entity {
            let Some(alert) = entity.alert else { continue };
            if entity.is_deleted == Some(true) {
                continue;
            }
            let description = text(alert.description_text.as_ref());
            let Some(title) = text(alert.header_text.as_ref()).or_else(|| {
                description
                    .as_deref()
                    .and_then(|text| text.lines().next())
                    .map(str::to_owned)
            }) else {
                continue;
            };

            let mut lists_stops = false;
            let mut stops = Vec::new();
            let mut routes = Vec::new();
            for informed in &alert.informed_entity {
                if let Some(stop) = &informed.stop_id {
                    lists_stops = true;
                    match stop_ids.get(stop) {
                        Some(id) if !stops.contains(id) => stops.push(id.clone()),
                        Some(_) => {}
                        None => {
                            unknown_stops.insert(stop.clone());
                        }
                    }
                }
                if let Some(route) = &informed.route_id {
                    routes.push(line_ref(route, &lines));
                }
            }
            routes.sort_by(|a, b| natural_cmp(&a.name, &b.name));
            routes.dedup_by(|a, b| a.name == b.name);

            let index = entries.len();
            for stop in &stops {
                by_stop.entry(stop.clone()).or_default().push(index);
            }
            entries.push(Entry {
                alert: Alert {
                    id: entity.id,
                    title,
                    description,
                    url: text(alert.url.as_ref()).filter(|url| is_web_link(url)),
                    lines: routes,
                    start: None,
                    end: None,
                },
                periods: shown_periods(&alert)
                    .iter()
                    .map(|period| Period {
                        start: period.start.and_then(timestamp),
                        end: period.end.and_then(timestamp),
                    })
                    .collect(),
                by_line: !lists_stops || stop_ids.is_empty(),
            });
        }

        let summary = AlertsSummary {
            read_at,
            alerts: entries.len(),
            unknown_stops: unknown_stops.len(),
        };
        Self {
            entries,
            by_stop,
            summary: Some(summary),
        }
    }

    /// `None` before the first successful read.
    pub fn summary(&self) -> Option<AlertsSummary> {
        self.summary
    }

    /// Every alert active at `now`.
    pub fn all(&self, now: DateTime<Utc>) -> Vec<Alert> {
        self.entries
            .iter()
            .filter_map(|entry| entry.active(now))
            .collect()
    }

    /// Alerts active at `now` that name `line`.
    pub fn for_line(&self, line: &str, now: DateTime<Utc>) -> Vec<Alert> {
        self.entries
            .iter()
            .filter(|entry| entry.alert.names_line(line))
            .filter_map(|entry| entry.active(now))
            .collect()
    }

    /// Alerts active at `now` for a stop: those that list it, and those linked by line on a
    /// line serving it (`stop_lines`, from the stop index). Two alerts with the same title
    /// and text are returned once.
    pub fn for_stop(
        &self,
        stop_id: &str,
        stop_lines: &[LineRef],
        now: DateTime<Utc>,
    ) -> Vec<Alert> {
        let listed = self.by_stop.get(stop_id).into_iter().flatten().copied();
        let by_line = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let serves = stop_lines
                    .iter()
                    .any(|line| entry.alert.names_line(&line.name));
                (entry.by_line && serves).then_some(index)
            });
        let mut indexes: Vec<usize> = listed.chain(by_line).collect();
        indexes.sort_unstable();
        indexes.dedup();

        let mut seen = HashSet::new();
        indexes
            .into_iter()
            .filter_map(|index| self.entries[index].active(now))
            .filter(|alert| seen.insert((alert.title.clone(), alert.description.clone())))
            .collect()
    }
}

/// When the alert should be shown. Newer feeds say it in `communication_period`; GTT still
/// uses `active_period`, deprecated by the specification but kept for compatibility.
#[allow(deprecated)]
fn shown_periods(alert: &proto::Alert) -> &[proto::TimeRange] {
    if alert.communication_period.is_empty() {
        &alert.active_period
    } else {
        &alert.communication_period
    }
}

/// The Italian text, or the first one; `None` when empty.
fn text(translated: Option<&TranslatedString>) -> Option<String> {
    let translations = &translated?.translation;
    let chosen = translations
        .iter()
        .find(|translation| translation.language.as_deref() == Some(LANGUAGE))
        .or_else(|| translations.first())?;
    let text = chosen.text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// Only links a browser opens as a page: never `javascript:` and the like.
fn is_web_link(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn timestamp(seconds: u64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(i64::try_from(seconds).ok()?, 0)
}

/// A GTT route (`54U`) as a line of the stop index (`gtt:54U`, "54"). A route no stop serves
/// keeps its id without the final `U` (urban) or `E` (extra-urban) as a name.
fn line_ref(route: &str, lines: &HashMap<&str, &LineRef>) -> LineRef {
    let id = format!("{FEED}:{route}");
    match lines.get(id.as_str()) {
        Some(line) => (*line).clone(),
        None => LineRef {
            name: route
                .strip_suffix(['U', 'E'])
                .filter(|name| !name.is_empty())
                .unwrap_or(route)
                .to_owned(),
            id,
            mode: None,
        },
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AlertsError {
    #[error(transparent)]
    Feed(#[from] UpstreamError),
    #[error("cannot read the GTT stop ids: {0}")]
    StopIds(#[from] sqlx::Error),
    #[error("cannot read the lines: {0}")]
    Lines(#[from] IndexError),
}

/// Reads the alerts and keeps the last ones read.
pub struct AlertsService {
    gtt: Arc<GttRealtime>,
    stops: StopIndex,
    gtt_static: Option<GttStatic>,
    current: RwLock<Arc<Alerts>>,
    wake: Notify,
}

impl AlertsService {
    pub fn new(gtt: Arc<GttRealtime>, stops: StopIndex, gtt_static: Option<GttStatic>) -> Self {
        Self {
            gtt,
            stops,
            gtt_static,
            current: RwLock::default(),
            wake: Notify::new(),
        }
    }

    /// The last alerts read; empty before the first successful read.
    pub fn current(&self) -> Arc<Alerts> {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Reads the feed and links it. On failure the previous alerts stay.
    pub async fn refresh(&self, now: DateTime<Utc>) -> Result<usize, AlertsError> {
        let feed = self.gtt.alerts().await?;
        let stop_ids = match &self.gtt_static {
            Some(gtt_static) => gtt_static.stop_ids().await?,
            None => HashMap::new(),
        };
        let lines = self.stops.lines(FEED).await?;
        let alerts = Alerts::from_feed(feed, &stop_ids, &lines, now);
        let count = alerts.entries.len();
        *self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Arc::new(alerts);
        Ok(count)
    }

    /// Asks the background task to read the alerts again now, e.g. after new static data.
    pub fn refresh_soon(&self) {
        self.wake.notify_one();
    }
}

/// Runs forever in the background: reads the alerts at startup, then every `every`, or sooner
/// when asked with `refresh_soon`.
pub fn spawn(
    service: Arc<AlertsService>,
    clock: Arc<dyn Clock>,
    every: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match service.refresh(clock.now()).await {
                Ok(count) => tracing::debug!("GTT alerts read: {count}"),
                Err(err) => tracing::warn!("GTT alerts unavailable: {err}"),
            }
            tokio::select! {
                () = tokio::time::sleep(every) => {}
                () = service.wake.notified() => {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    /// Recorded on 2026-11-04 at 16:03:58 (+01:00); 119 alerts.
    fn recorded_feed() -> proto::FeedMessage {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/gtt/alerts.pb"
        ))
        .unwrap();
        proto::FeedMessage::decode(bytes.as_slice()).unwrap()
    }

    const RECORDED: i64 = 1_793_804_638;

    fn at(timestamp: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(timestamp, 0).unwrap()
    }

    fn line(id: &str, name: &str, mode: &str) -> LineRef {
        LineRef {
            id: id.to_owned(),
            name: name.to_owned(),
            mode: Some(mode.to_owned()),
        }
    }

    fn translated(text: &str) -> Option<TranslatedString> {
        Some(TranslatedString {
            translation: vec![proto::translated_string::Translation {
                text: text.to_owned(),
                language: Some(LANGUAGE.to_owned()),
            }],
        })
    }

    /// An alert naming `routes` and GTT `stops`, active from `start` to `end`.
    fn alert(
        id: &str,
        title: &str,
        routes: &[&str],
        stops: &[&str],
        period: (Option<u64>, Option<u64>),
    ) -> proto::FeedEntity {
        let mut informed: Vec<proto::EntitySelector> = routes
            .iter()
            .map(|route| proto::EntitySelector {
                route_id: Some((*route).to_owned()),
                ..Default::default()
            })
            .collect();
        informed.extend(stops.iter().map(|stop| proto::EntitySelector {
            stop_id: Some((*stop).to_owned()),
            ..Default::default()
        }));
        #[allow(deprecated)]
        proto::FeedEntity {
            id: id.to_owned(),
            alert: Some(proto::Alert {
                active_period: vec![proto::TimeRange {
                    start: period.0,
                    end: period.1,
                }],
                informed_entity: informed,
                header_text: translated(title),
                description_text: translated(&format!("{title}: details")),
                url: translated("https://www.gtt.to.it/cms/percorari/urbano"),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn feed(entities: Vec<proto::FeedEntity>) -> proto::FeedMessage {
        proto::FeedMessage {
            entity: entities,
            ..Default::default()
        }
    }

    fn stop_ids() -> HashMap<String, String> {
        [("2843", "gtt:39"), ("1000", "gtt:750"), ("2000", "gtt:40")]
            .into_iter()
            .map(|(gtt, ours)| (gtt.to_owned(), ours.to_owned()))
            .collect()
    }

    fn index_lines() -> Vec<LineRef> {
        vec![
            line("gtt:4U", "4", "TRAM"),
            line("gtt:9U", "9", "TRAM"),
            line("gtt:67U", "67", "BUS"),
            line("gtt:17BU", "17/", "BUS"),
        ]
    }

    fn titles(alerts: &[Alert]) -> Vec<&str> {
        alerts.iter().map(|alert| alert.title.as_str()).collect()
    }

    const ALWAYS: (Option<u64>, Option<u64>) = (None, None);

    #[test]
    fn links_by_stop_when_stops_are_listed_otherwise_by_line() {
        let alerts = Alerts::from_feed(
            feed(vec![
                // A stop alert: one stop, every line serving it.
                alert(
                    "a",
                    "Fermata 750 sospesa",
                    &["4U", "67U"],
                    &["1000"],
                    ALWAYS,
                ),
                // A line alert: its stops.
                alert("b", "Linea 67 deviata", &["67U"], &["2843", "2000"], ALWAYS),
                // Lines only.
                alert("c", "Linea 9 limitata", &["9U"], &[], ALWAYS),
                // Only stops missing from the static data (temporary ones): no stop to show it.
                alert("d", "Linea 4 deviata", &["4U"], &["777"], ALWAYS),
            ]),
            &stop_ids(),
            &index_lines(),
            at(RECORDED),
        );
        let now = at(RECORDED);
        let lines_39 = [line("gtt:4U", "4", "TRAM"), line("gtt:67U", "67", "BUS")];
        assert_eq!(
            titles(&alerts.for_stop("gtt:39", &lines_39, now)),
            ["Linea 67 deviata"],
            "the stop alert of 750 stays at 750"
        );
        assert_eq!(
            titles(&alerts.for_stop("gtt:750", &lines_39, now)),
            ["Fermata 750 sospesa"]
        );
        assert_eq!(
            titles(&alerts.for_line("4", now)),
            ["Fermata 750 sospesa", "Linea 4 deviata"]
        );
        let lines_9 = [line("gtt:9U", "9", "TRAM")];
        assert_eq!(
            titles(&alerts.for_stop("gtt:1", &lines_9, now)),
            ["Linea 9 limitata"]
        );
        assert_eq!(
            titles(&alerts.for_line("67", now)),
            ["Fermata 750 sospesa", "Linea 67 deviata"]
        );
        let summary = alerts.summary().unwrap();
        assert_eq!((summary.alerts, summary.unknown_stops), (4, 1));
    }

    #[test]
    fn without_static_data_every_alert_is_linked_by_line() {
        let alerts = Alerts::from_feed(
            feed(vec![alert(
                "a",
                "Fermata 750 sospesa",
                &["67U"],
                &["1000"],
                ALWAYS,
            )]),
            &HashMap::new(),
            &index_lines(),
            at(RECORDED),
        );
        let lines = [line("gtt:67U", "67", "BUS")];
        assert_eq!(alerts.for_stop("gtt:39", &lines, at(RECORDED)).len(), 1);
    }

    #[test]
    fn shows_only_active_alerts_with_their_period() {
        let now = RECORDED as u64;
        let alerts = Alerts::from_feed(
            feed(vec![
                alert(
                    "past",
                    "Finito",
                    &["4U"],
                    &[],
                    (Some(now - 7200), Some(now - 3600)),
                ),
                alert(
                    "now",
                    "In corso",
                    &["4U"],
                    &[],
                    (Some(now - 3600), Some(now + 3600)),
                ),
                alert("open", "Senza fine", &["4U"], &[], (Some(now - 60), None)),
                alert("future", "Domani", &["4U"], &[], (Some(now + 86_400), None)),
            ]),
            &stop_ids(),
            &index_lines(),
            at(RECORDED),
        );
        let active = alerts.all(at(RECORDED));
        assert_eq!(titles(&active), ["In corso", "Senza fine"]);
        assert_eq!(
            active[0].start.unwrap().to_rfc3339(),
            "2026-11-04T15:03:58+01:00"
        );
        assert_eq!(active[1].end, None);
    }

    #[test]
    fn names_lines_from_the_index_and_falls_back_to_the_route_id() {
        let alerts = Alerts::from_feed(
            feed(vec![alert(
                "a",
                "Linee deviate",
                &["67U", "17BU", "94U", "S45U", "1068E", "4U", "4U"],
                &[],
                ALWAYS,
            )]),
            &stop_ids(),
            &index_lines(),
            at(RECORDED),
        );
        let alert = &alerts.all(at(RECORDED))[0];
        let names: Vec<_> = alert.lines.iter().map(|line| line.name.as_str()).collect();
        assert_eq!(names, ["4", "17/", "67", "94", "1068", "S45"]);
        assert_eq!(alert.lines[1], line("gtt:17BU", "17/", "BUS"));
        assert_eq!(alert.lines[3].mode, None);
        assert_eq!(
            titles(&alerts.for_line("17/", at(RECORDED))),
            ["Linee deviate"]
        );
    }

    #[test]
    fn shows_identical_alerts_once_at_a_stop() {
        let alerts = Alerts::from_feed(
            feed(vec![
                alert("a", "Linea 94 deviata", &["67U"], &["2843"], ALWAYS),
                alert("b", "Linea 94 deviata", &["67U"], &["2843"], ALWAYS),
            ]),
            &stop_ids(),
            &index_lines(),
            at(RECORDED),
        );
        assert_eq!(alerts.for_stop("gtt:39", &[], at(RECORDED)).len(), 1);
        assert_eq!(alerts.all(at(RECORDED)).len(), 2);
    }

    #[test]
    fn keeps_only_web_links_and_falls_back_to_the_description_for_the_title() {
        let mut entity = alert("a", "", &["4U"], &[], ALWAYS);
        let inner = entity.alert.as_mut().unwrap();
        inner.header_text = None;
        inner.description_text = translated("Linea 4 deviata\nDa lunedì.");
        inner.url = translated("javascript:alert(1)");
        let alerts = Alerts::from_feed(feed(vec![entity]), &stop_ids(), &[], at(RECORDED));
        let alert = &alerts.all(at(RECORDED))[0];
        assert_eq!(alert.title, "Linea 4 deviata");
        assert_eq!(alert.url, None);
    }

    #[test]
    fn reads_the_recorded_feed() {
        let alerts = Alerts::from_feed(recorded_feed(), &HashMap::new(), &[], at(RECORDED));
        let summary = alerts.summary().unwrap();
        assert_eq!(summary.alerts, 119);
        let active = alerts.all(at(RECORDED));
        assert_eq!(active.len(), 119);
        assert!(active.iter().all(|alert| alert.url.is_some()));
        let detour = active
            .iter()
            .find(|alert| alert.title.starts_with("Linea 9 deviata"))
            .unwrap();
        assert_eq!(detour.lines.len(), 1);
        assert_eq!(detour.lines[0].name, "9");
        assert_eq!(detour.lines[0].id, "gtt:9U");
    }
}
