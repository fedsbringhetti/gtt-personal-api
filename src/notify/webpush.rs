//! Delivery through the browsers' push services, encrypted with `aes128gcm` (RFC 8291) and
//! authenticated with VAPID (RFC 8292).

use std::{sync::Arc, time::Duration};

use web_push_native::{Auth, WebPushBuilder};

use super::{BoxFuture, Notification, Notifier, NotifyError, SendReport, vapid::VapidKey};
use crate::{
    push::{Subscription, SubscriptionStore, decode_keys},
    time::Clock,
};

/// How long a push service keeps a message for a device that is offline (decision AN): a
/// departure reminder is useless after that.
const TTL: Duration = Duration::from_secs(15 * 60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// How much of a push service's error body is logged.
const LOGGED_BODY_CHARS: usize = 200;

pub struct WebPushNotifier {
    store: SubscriptionStore,
    key: VapidKey,
    subject: String,
    client: reqwest::Client,
    clock: Arc<dyn Clock>,
}

/// What happened to one subscription.
enum Outcome {
    Sent,
    Removed,
    Failed,
}

impl WebPushNotifier {
    pub fn new(
        store: SubscriptionStore,
        key: VapidKey,
        subject: String,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            // A redirect would send the encrypted body to a host outside the endpoint
            // allowlist (decision AM): a 3xx is a refusal, counted as failed.
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            store,
            key,
            subject,
            client,
            clock,
        })
    }

    /// Builds the encrypted, signed request for one subscription.
    fn request(
        &self,
        subscription: &Subscription,
        payload: &[u8],
    ) -> Result<reqwest::Request, String> {
        let (public_key, auth) = decode_keys(&subscription.p256dh, &subscription.auth)?;
        let url: reqwest::Url = subscription
            .endpoint
            .parse()
            .map_err(|_| "stored endpoint is not a valid URL".to_owned())?;
        let uri = subscription
            .endpoint
            .parse()
            .map_err(|_| "stored endpoint is not a valid URI".to_owned())?;
        // `decode_keys` checked the length: `Auth::clone_from_slice` would panic otherwise.
        let auth = Auth::from(auth);
        let mut request = WebPushBuilder::new(uri, public_key, auth)
            .with_valid_duration(TTL)
            .build(payload.to_vec())
            .map_err(|err| format!("cannot encrypt the payload: {err}"))?;

        let authorization = self
            .key
            .authorization(&url, &self.subject, self.clock.now())
            .parse()
            .map_err(|_| "invalid Authorization header".to_owned())?;
        request
            .headers_mut()
            .insert(reqwest::header::AUTHORIZATION, authorization);
        request
            .headers_mut()
            .insert("Urgency", reqwest::header::HeaderValue::from_static("high"));
        reqwest::Request::try_from(request).map_err(|err| err.to_string())
    }

    async fn deliver(&self, subscription: &Subscription, payload: &[u8]) -> Outcome {
        let id = subscription.id;
        let request = match self.request(subscription, payload) {
            Ok(request) => request,
            Err(reason) => {
                tracing::warn!(subscription = id, "cannot build the push request: {reason}");
                return Outcome::Failed;
            }
        };
        let response = match self.client.execute(request).await {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!(
                    subscription = id,
                    "push request failed: {}",
                    err.without_url()
                );
                return Outcome::Failed;
            }
        };
        let status = response.status();
        if status.is_success() {
            return Outcome::Sent;
        }
        if matches!(status.as_u16(), 404 | 410) {
            return match self.store.delete(id).await {
                Ok(_) => {
                    tracing::info!(subscription = id, %status, "subscription expired, removed");
                    Outcome::Removed
                }
                Err(err) => {
                    tracing::warn!(subscription = id, "cannot remove the subscription: {err}");
                    Outcome::Failed
                }
            };
        }
        let body = response.text().await.unwrap_or_default();
        let body: String = body.chars().take(LOGGED_BODY_CHARS).collect();
        tracing::warn!(subscription = id, %status, body, "push service refused the message");
        Outcome::Failed
    }
}

impl Notifier for WebPushNotifier {
    fn channel(&self) -> &'static str {
        "webpush"
    }

    fn send<'a>(
        &'a self,
        notification: &'a Notification,
    ) -> BoxFuture<'a, Result<SendReport, NotifyError>> {
        Box::pin(async move {
            let payload = notification.payload()?;
            let subscriptions = self.store.list().await?;
            // One after the other: a personal service has a handful of devices, so there is
            // no need for concurrency (or for a `futures` dependency).
            let mut report = SendReport::default();
            for subscription in &subscriptions {
                match self.deliver(subscription, &payload).await {
                    Outcome::Sent => report.sent += 1,
                    Outcome::Removed => report.removed += 1,
                    Outcome::Failed => report.failed += 1,
                }
            }
            Ok(report)
        })
    }
}
