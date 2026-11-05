//! Web Push subscriptions: storage, validation of the keys a browser sends, and the policy on
//! which push services the server may contact.

use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

/// A device that enabled notifications.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Subscription {
    pub id: i64,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    /// Device kind taken from the user agent, e.g. `iPhone`.
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewSubscription {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub label: Option<String>,
}

const SELECT_SUBSCRIPTION: &str =
    "SELECT id, endpoint, p256dh, auth, label, created_at FROM push_subscriptions";

#[derive(Debug, Clone)]
pub struct SubscriptionStore {
    pool: SqlitePool,
}

impl SubscriptionStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts the subscription, or updates the keys and label of the one with the same
    /// endpoint (its `created_at` is kept). The `bool` is `true` when it was created.
    pub async fn upsert(
        &self,
        new: NewSubscription,
        now: DateTime<Utc>,
    ) -> Result<(Subscription, bool), PushError> {
        // The first statement is a write, so the transaction takes SQLite's write lock up
        // front: two concurrent requests for the same endpoint are serialized and cannot both
        // take the insert path and hit the UNIQUE constraint (a read-then-write transaction
        // could instead fail with SQLITE_BUSY on the lock upgrade). `DO NOTHING` reports
        // exactly whether a row was inserted, which is how `created` is derived: a single
        // `ON CONFLICT DO UPDATE ... RETURNING` cannot tell the two cases apart (comparing
        // `created_at` with `now` is wrong for a re-subscription in the same instant, and a
        // subquery in RETURNING sees the row already written). `created_at` is kept on update.
        let mut tx = self.pool.begin().await?;
        let created = sqlx::query(
            "INSERT INTO push_subscriptions (endpoint, p256dh, auth, label, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(endpoint) DO NOTHING",
        )
        .bind(&new.endpoint)
        .bind(&new.p256dh)
        .bind(&new.auth)
        .bind(&new.label)
        .bind(now)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        let subscription: Subscription = sqlx::query_as(
            "UPDATE push_subscriptions SET p256dh = ?, auth = ?, label = ? WHERE endpoint = ?
             RETURNING id, endpoint, p256dh, auth, label, created_at",
        )
        .bind(&new.p256dh)
        .bind(&new.auth)
        .bind(&new.label)
        .bind(&new.endpoint)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((subscription, created))
    }

    /// All subscriptions, oldest first.
    pub async fn list(&self) -> Result<Vec<Subscription>, PushError> {
        Ok(sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT_SUBSCRIPTION} ORDER BY id"
        )))
        .fetch_all(&self.pool)
        .await?)
    }

    /// `false` when the subscription does not exist.
    pub async fn delete(&self, id: i64) -> Result<bool, PushError> {
        let result = sqlx::query("DELETE FROM push_subscriptions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// `false` when no subscription has this endpoint.
    pub async fn delete_by_endpoint(&self, endpoint: &str) -> Result<bool, PushError> {
        let result = sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ?")
            .bind(endpoint)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Decodes the keys of a browser's `PushSubscription`: `p256dh` must be a valid uncompressed
/// P-256 point, `auth` exactly 16 bytes (both base64url, with or without padding). Check
/// this before `web_push_native::Auth::clone_from_slice`, which panics on other lengths.
pub fn decode_keys(p256dh: &str, auth: &str) -> Result<(p256::PublicKey, [u8; 16]), String> {
    let point = decode_base64url(p256dh).map_err(|err| format!("p256dh: {err}"))?;
    if point.len() != 65 || point[0] != 0x04 {
        return Err("p256dh: not an uncompressed P-256 point".to_owned());
    }
    let public_key = p256::PublicKey::from_sec1_bytes(&point)
        .map_err(|_| "p256dh: not a valid P-256 point".to_owned())?;
    let auth = decode_base64url(auth).map_err(|err| format!("auth: {err}"))?;
    let auth: [u8; 16] = auth
        .try_into()
        .map_err(|_| "auth: must be 16 bytes".to_owned())?;
    Ok((public_key, auth))
}

/// Browsers use base64url without padding, but some libraries pad: accept both.
fn decode_base64url(value: &str) -> Result<Vec<u8>, String> {
    let value = value.trim_end_matches('=');
    Base64UrlUnpadded::decode_vec(value).map_err(|_| "not valid base64url".to_owned())
}

const MAX_ENDPOINT_LEN: usize = 2048;

/// Hosts of the push services of the browsers we support (decision AM).
const PUSH_SERVICE_HOSTS: [&str; 4] = [
    "fcm.googleapis.com",
    "push.apple.com",
    "push.services.mozilla.com",
    "notify.windows.com",
];

/// Which endpoints the server may POST to. A subscription endpoint comes from the client, so
/// without a policy the server could be made to contact any URL.
#[derive(Debug, Clone)]
pub struct EndpointPolicy {
    hosts: Vec<String>,
    require_https: bool,
}

impl EndpointPolicy {
    /// The production policy: `https` endpoints of the known push services.
    pub fn push_services() -> Self {
        Self::new(
            PUSH_SERVICE_HOSTS
                .iter()
                .map(|host| (*host).to_owned())
                .collect(),
            true,
        )
    }

    /// A custom policy, for tests: each host (or a subdomain of it) is allowed.
    pub fn new(hosts: Vec<String>, require_https: bool) -> Self {
        Self {
            hosts: hosts
                .into_iter()
                .map(|host| host.to_ascii_lowercase())
                .collect(),
            require_https,
        }
    }

    /// Parses `endpoint` and checks it against the policy. Returns the normalized URL: store
    /// and use `url.as_str()`, not the raw string.
    pub fn check(&self, endpoint: &str) -> Result<reqwest::Url, String> {
        if endpoint.len() > MAX_ENDPOINT_LEN {
            return Err(format!(
                "endpoint longer than {MAX_ENDPOINT_LEN} characters"
            ));
        }
        let url: reqwest::Url = endpoint
            .parse()
            .map_err(|_| "endpoint is not a valid URL".to_owned())?;
        let scheme_ok = match url.scheme() {
            "https" => true,
            "http" => !self.require_https,
            _ => false,
        };
        if !scheme_ok {
            return Err(if self.require_https {
                "endpoint must use https".to_owned()
            } else {
                "endpoint must use http or https".to_owned()
            });
        }
        // `url.port()` is `None` for the scheme's default port, so any value is explicit.
        if self.require_https && url.port().is_some() {
            return Err("endpoint must not use a custom port".to_owned());
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("endpoint must not contain credentials".to_owned());
        }
        let host = url
            .host_str()
            .ok_or_else(|| "endpoint has no host".to_owned())?
            .to_ascii_lowercase();
        let allowed = self
            .hosts
            .iter()
            .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")));
        if !allowed {
            return Err("endpoint is not a known push service".to_owned());
        }
        // `WebPushBuilder` needs an `http::Uri` at send time: refuse what it cannot parse now.
        url.as_str()
            .parse::<axum::http::Uri>()
            .map_err(|_| "endpoint is not a valid URI".to_owned())?;
        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use chrono::TimeZone;
    use p256::elliptic_curve::sec1::ToEncodedPoint;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    fn new_sub(endpoint: &str, key: &str) -> NewSubscription {
        NewSubscription {
            endpoint: endpoint.to_owned(),
            p256dh: key.to_owned(),
            auth: "auth".to_owned(),
            label: Some("iPhone".to_owned()),
        }
    }

    async fn store() -> SubscriptionStore {
        SubscriptionStore::new(db::connect_in_memory().await.unwrap())
    }

    #[tokio::test]
    async fn upsert_inserts_then_updates_keeping_created_at() {
        let store = store().await;
        let (first, created) = store
            .upsert(new_sub("https://fcm.googleapis.com/a", "k1"), at(100))
            .await
            .unwrap();
        assert!(created);
        assert_eq!(first.created_at, at(100));
        assert_eq!(first.label.as_deref(), Some("iPhone"));

        let mut changed = new_sub("https://fcm.googleapis.com/a", "k2");
        changed.auth = "auth2".to_owned();
        changed.label = None;
        let (second, created) = store.upsert(changed, at(200)).await.unwrap();
        assert!(!created);
        assert_eq!(second.id, first.id);
        assert_eq!(second.p256dh, "k2");
        assert_eq!(second.auth, "auth2");
        assert_eq!(second.label, None);
        assert_eq!(second.created_at, at(100));
        assert_eq!(store.list().await.unwrap(), vec![second]);
    }

    #[tokio::test]
    async fn concurrent_upserts_of_one_endpoint_leave_one_row() {
        // The in-memory pool has a single connection, so the two statements are serialized
        // here: this checks that neither fails on the UNIQUE constraint and that exactly one
        // reports `created`. The atomicity itself comes from the write-first transaction.
        let store = store().await;
        let (a, b) = tokio::join!(
            store.upsert(new_sub("https://fcm.googleapis.com/a", "k1"), at(100)),
            store.upsert(new_sub("https://fcm.googleapis.com/a", "k2"), at(200)),
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!(a.0.id, b.0.id);
        assert_eq!([a.1, b.1].iter().filter(|created| **created).count(), 1);
        assert_eq!(store.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn list_and_delete() {
        let store = store().await;
        let (a, _) = store
            .upsert(new_sub("https://fcm.googleapis.com/a", "k"), at(1))
            .await
            .unwrap();
        let (b, _) = store
            .upsert(new_sub("https://fcm.googleapis.com/b", "k"), at(2))
            .await
            .unwrap();
        assert_eq!(store.list().await.unwrap(), vec![a.clone(), b.clone()]);

        assert!(store.delete(a.id).await.unwrap());
        assert!(!store.delete(a.id).await.unwrap());
        assert_eq!(store.list().await.unwrap(), vec![b]);
    }

    #[tokio::test]
    async fn delete_by_endpoint() {
        let store = store().await;
        store
            .upsert(new_sub("https://fcm.googleapis.com/a", "k"), at(1))
            .await
            .unwrap();
        assert!(
            !store
                .delete_by_endpoint("https://fcm.googleapis.com/other")
                .await
                .unwrap()
        );
        assert!(
            store
                .delete_by_endpoint("https://fcm.googleapis.com/a")
                .await
                .unwrap()
        );
        assert!(store.list().await.unwrap().is_empty());
    }

    fn browser_key() -> (String, p256::PublicKey) {
        let secret = p256::SecretKey::random(&mut p256::elliptic_curve::rand_core::OsRng);
        let public = secret.public_key();
        let encoded = Base64UrlUnpadded::encode_string(public.to_encoded_point(false).as_bytes());
        (encoded, public)
    }

    #[test]
    fn decode_keys_accepts_a_real_key() {
        let (encoded, public) = browser_key();
        let auth = Base64UrlUnpadded::encode_string(&[7u8; 16]);
        let (decoded, auth) = decode_keys(&encoded, &auth).unwrap();
        assert_eq!(decoded, public);
        assert_eq!(auth, [7u8; 16]);
        // Padding is tolerated.
        let padded = format!("{}==", Base64UrlUnpadded::encode_string(&[7u8; 16]));
        assert!(decode_keys(&encoded, &padded).is_ok());
    }

    #[test]
    fn decode_keys_refuses_bad_input() {
        let (encoded, _) = browser_key();
        let auth = |n: usize| Base64UrlUnpadded::encode_string(&vec![1u8; n]);
        // Bad base64.
        assert!(decode_keys("not base64!", &auth(16)).is_err());
        assert!(decode_keys(&encoded, "not base64!").is_err());
        // A 65-byte value that is not on the curve.
        let mut off_curve = vec![4u8];
        off_curve.extend([1u8; 64]);
        assert!(decode_keys(&Base64UrlUnpadded::encode_string(&off_curve), &auth(16)).is_err());
        // Wrong length for a point.
        assert!(decode_keys(&Base64UrlUnpadded::encode_string(&[4u8; 10]), &auth(16)).is_err());
        // Auth of the wrong length (this must not panic).
        for n in [0, 15, 17] {
            assert!(decode_keys(&encoded, &auth(n)).is_err(), "{n}");
        }
    }

    #[test]
    fn policy_allows_the_push_services_and_their_subdomains() {
        let policy = EndpointPolicy::push_services();
        for endpoint in [
            "https://fcm.googleapis.com/fcm/send/abc",
            "https://web.push.apple.com/QGx",
            "https://push.apple.com/x",
            "https://updates.push.services.mozilla.com/wpush/v2/abc",
            "https://db5p.notify.windows.com/w/?token=x",
        ] {
            assert!(policy.check(endpoint).is_ok(), "{endpoint}");
        }
    }

    #[test]
    fn policy_refuses_everything_else() {
        let policy = EndpointPolicy::push_services();
        let too_long = format!("https://fcm.googleapis.com/{}", "a".repeat(2048));
        for endpoint in [
            "https://evilfcm.googleapis.com.example.com/x",
            "https://fcm.googleapis.com.evil.com/x",
            "https://evilfcm.googleapis.com/x",
            "http://fcm.googleapis.com/x",
            "https://user:pw@fcm.googleapis.com/x",
            "https://user@fcm.googleapis.com/x",
            "ftp://fcm.googleapis.com/x",
            "https://127.0.0.1/x",
            "https://fcm.googleapis.com:22/x",
            "https://fcm.googleapis.com:8443/x",
            "not a url",
            too_long.as_str(),
        ] {
            assert!(policy.check(endpoint).is_err(), "{endpoint}");
        }
    }

    #[test]
    fn policy_accepts_the_default_port_and_returns_the_normalized_url() {
        let policy = EndpointPolicy::push_services();
        let url = policy
            .check("https://FCM.googleapis.com:443/fcm/send/a")
            .unwrap();
        assert_eq!(url.as_str(), "https://fcm.googleapis.com/fcm/send/a");
    }

    #[test]
    fn policy_for_tests_can_allow_http_localhost() {
        let policy = EndpointPolicy::new(vec!["127.0.0.1".to_owned()], false);
        assert!(policy.check("http://127.0.0.1:1234/push").is_ok());
        assert!(policy.check("http://localhost:1234/push").is_err());
        let strict = EndpointPolicy::new(vec!["127.0.0.1".to_owned()], true);
        assert!(strict.check("http://127.0.0.1:1234/push").is_err());
    }
}
