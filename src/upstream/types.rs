//! Shapes of the upstream GraphQL responses, limited to the fields we query.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StopData {
    /// `null` when the stop does not exist.
    pub stop: Option<Stop>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub gtfs_id: String,
    /// `null` for GTT metro stops.
    pub code: Option<String>,
    pub name: String,
    #[serde(default)]
    pub stoptimes_without_patterns: Vec<Stoptime>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stoptime {
    /// Seconds from `service_day`.
    pub scheduled_departure: i64,
    /// Seconds from `service_day`.
    pub realtime_departure: i64,
    pub realtime: bool,
    /// `SCHEDULED`, `UPDATED`, `CANCELED`, ...
    pub realtime_state: Option<String>,
    pub service_day: i64,
    /// Position of the stop in the run; GTT's trip updates use the same numbers.
    pub stop_sequence: Option<u32>,
    pub headsign: Option<String>,
    pub trip: Trip,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trip {
    pub gtfs_id: String,
    pub trip_headsign: Option<String>,
    pub route: Route,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// Line name as shown to passengers: `4`, `9B`, `M1S`, `68+`.
    pub short_name: Option<String>,
    /// `BUS`, `TRAM`, `SUBWAY`, `RAIL`, ...
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AllStopsData {
    pub stops: Vec<IndexStop>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStop {
    pub gtfs_id: String,
    pub code: Option<String>,
    pub name: Option<String>,
    pub desc: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub vehicle_mode: Option<String>,
    pub routes: Option<Vec<RouteRef>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRef {
    pub gtfs_id: String,
    pub short_name: Option<String>,
    pub mode: Option<String>,
}
