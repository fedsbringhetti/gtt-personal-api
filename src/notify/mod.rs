//! Notifications: what is sent ([`Notification`]) and how ([`Notifier`]).

use std::{future::Future, pin::Pin};

use serde::Serialize;

use crate::push::PushError;

pub mod log;
pub mod vapid;
pub mod webpush;

/// Largest payload we send. Push services accept 4096 bytes after encryption; this leaves room
/// for the encryption overhead.
pub const MAX_PAYLOAD_BYTES: usize = 3000;

/// A notification for the service worker: it receives the JSON of [`Notification::payload`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notification {
    pub title: String,
    pub body: String,
    /// A path inside the app, e.g. `/notifiche`, opened when the notification is tapped.
    pub url: String,
    /// Notifications with the same tag replace each other.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl Notification {
    /// The JSON sent to the service worker, at most [`MAX_PAYLOAD_BYTES`].
    pub fn payload(&self) -> Result<Vec<u8>, NotifyError> {
        let payload = serde_json::to_vec(self)?;
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(NotifyError::PayloadTooLarge(payload.len()));
        }
        Ok(payload)
    }
}

/// The outcome of delivering one notification to every device.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SendReport {
    pub sent: usize,
    /// Subscriptions deleted because the push service said they no longer exist.
    pub removed: usize,
    pub failed: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    #[error("notification payload is {0} bytes, the limit is {MAX_PAYLOAD_BYTES}")]
    PayloadTooLarge(usize),
    #[error("cannot encode the notification: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("subscription store error: {0}")]
    Store(#[from] PushError),
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Delivers notifications. Dyn-compatible, so the application holds an `Arc<dyn Notifier>`.
pub trait Notifier: Send + Sync {
    /// `"webpush"` or `"log"`.
    fn channel(&self) -> &'static str;

    /// Delivers `notification` to every device. One device failing never stops the others.
    fn send<'a>(
        &'a self,
        notification: &'a Notification,
    ) -> BoxFuture<'a, Result<SendReport, NotifyError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notification(tag: Option<&str>) -> Notification {
        Notification {
            title: "Titolo".to_owned(),
            body: "Corpo".to_owned(),
            url: "/notifiche".to_owned(),
            tag: tag.map(str::to_owned),
        }
    }

    #[test]
    fn payload_has_the_documented_shape() {
        let payload = notification(Some("test")).payload().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "title": "Titolo", "body": "Corpo", "url": "/notifiche", "tag": "test"
            })
        );
    }

    #[test]
    fn payload_omits_a_missing_tag() {
        let payload = notification(None).payload().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert!(json.get("tag").is_none());
    }

    #[test]
    fn payload_too_large_is_refused() {
        let mut big = notification(None);
        big.body = "x".repeat(MAX_PAYLOAD_BYTES);
        assert!(matches!(
            big.payload(),
            Err(NotifyError::PayloadTooLarge(n)) if n > MAX_PAYLOAD_BYTES
        ));
    }
}
