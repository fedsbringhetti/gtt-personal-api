//! The notifier used without VAPID keys: it only writes the notification to the log.

use super::{BoxFuture, Notification, Notifier, NotifyError, SendReport};

#[derive(Debug, Clone, Copy, Default)]
pub struct LogNotifier;

impl Notifier for LogNotifier {
    fn channel(&self) -> &'static str {
        "log"
    }

    fn send<'a>(
        &'a self,
        notification: &'a Notification,
    ) -> BoxFuture<'a, Result<SendReport, NotifyError>> {
        Box::pin(async move {
            tracing::info!(
                title = %notification.title,
                body = %notification.body,
                url = %notification.url,
                "notification (log channel)"
            );
            Ok(SendReport {
                sent: 1,
                ..SendReport::default()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_one_sent() {
        let notification = Notification {
            title: "t".to_owned(),
            body: "b".to_owned(),
            url: "/".to_owned(),
            tag: None,
        };
        let report = LogNotifier.send(&notification).await.unwrap();
        assert_eq!(
            report,
            SendReport {
                sent: 1,
                removed: 0,
                failed: 0
            }
        );
        assert_eq!(LogNotifier.channel(), "log");
    }
}
