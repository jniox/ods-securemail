//! CloudEvents producer for the SecureMail service.
//!
//! Re-exports ods-common's CloudEvent, EventProducer trait, and implementations.

pub use ods_common::events::{CloudEvent, EventProducer, InMemoryProducer, RedpandaProducer};

/// Event type constants for SecureMail.
pub mod event_types {
    pub const EMAIL_QUEUED: &str = "email.queued";
    pub const EMAIL_SENT: &str = "email.sent";
    pub const EMAIL_DELIVERED: &str = "email.delivered";
    pub const EMAIL_OPENED: &str = "email.opened";
    pub const EMAIL_BOUNCED: &str = "email.bounced";
    pub const EMAIL_FAILED: &str = "email.failed";
    pub const INBOUND_RECEIVED: &str = "inbound.received";
    pub const CONFIG_CREATED: &str = "config.created";
    pub const CONFIG_VERIFIED: &str = "config.verified";
    pub const KEY_EXPIRING: &str = "key.expiring";
}

/// The event source identifier for the SecureMail service.
pub const EVENT_SOURCE: &str = "/ods/securemail";

/// The default Redpanda topic for securemail events.
pub const DEFAULT_TOPIC: &str = "ods.securemail.events";

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_cloud_event_new() {
        let tenant_id = Uuid::new_v4();
        let event = CloudEvent::new(
            EVENT_SOURCE,
            event_types::CONFIG_CREATED,
            "config-id",
            tenant_id,
            serde_json::json!({"name": "test config"}),
        );

        assert_eq!(event.specversion, "1.0");
        assert_eq!(event.source, "/ods/securemail");
        assert_eq!(event.event_type, "ods.securemail.config.created");
        assert_eq!(event.tenantid, tenant_id.to_string());
    }

    #[test]
    fn test_event_type_constants() {
        assert_eq!(event_types::EMAIL_QUEUED, "email.queued");
        assert_eq!(event_types::EMAIL_SENT, "email.sent");
        assert_eq!(event_types::CONFIG_CREATED, "config.created");
        assert_eq!(event_types::CONFIG_VERIFIED, "config.verified");
    }

    #[tokio::test]
    async fn test_in_memory_producer() {
        let producer = InMemoryProducer::new();
        let event = CloudEvent::new(
            EVENT_SOURCE,
            event_types::CONFIG_CREATED,
            "subject",
            Uuid::new_v4(),
            serde_json::json!({}),
        );

        producer.publish(event).await.unwrap();
        assert_eq!(producer.events().len(), 1);

        producer.clear();
        assert_eq!(producer.events().len(), 0);
    }
}
