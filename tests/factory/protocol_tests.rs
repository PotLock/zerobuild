//! Integration tests for factory protocol module
//!
//! Tests message passing, routing, and handler registration.

use std::sync::Arc;
use zerobuild::factory::protocol::{
    AgentMessage, MessageBus, MessageContent, MessageHandler, MessageHandlerRegistry,
    MessageHeader, MessageId, MessagePriority, ProtocolError,
};

struct EchoHandler;

#[async_trait::async_trait]
impl MessageHandler for EchoHandler {
    async fn handle(&self, message: AgentMessage) -> Result<Option<AgentMessage>, ProtocolError> {
        // Echo back a response
        let response = message.create_response(true, Some(serde_json::json!({"echo": true})), None);
        Ok(Some(response))
    }

    fn can_handle(&self, content_type: &str) -> bool {
        content_type == "test"
    }
}

struct LoggingHandler;

#[async_trait::async_trait]
impl MessageHandler for LoggingHandler {
    async fn handle(&self, _message: AgentMessage) -> Result<Option<AgentMessage>, ProtocolError> {
        // Just log, no response
        Ok(None)
    }

    fn can_handle(&self, content_type: &str) -> bool {
        content_type == "log"
    }
}

#[tokio::test]
async fn test_message_creation() {
    let message = AgentMessage::new(
        "sender_agent",
        MessageContent::command("test", vec!["arg1".to_string()]),
    );

    assert_eq!(message.header.sender, "sender_agent");
    assert!(!message.id().0.to_string().is_empty());
}

#[tokio::test]
async fn test_message_builder() {
    let correlation_id = MessageId::new();
    let message = AgentMessage::new("sender", MessageContent::command("test", vec![]))
        .with_recipient("recipient")
        .with_priority(MessagePriority::High)
        .with_correlation(correlation_id);

    assert_eq!(message.header.recipient, Some("recipient".to_string()));
    assert_eq!(message.header.priority, MessagePriority::High);
    assert_eq!(message.header.correlation_id, Some(correlation_id));
}

#[tokio::test]
async fn test_message_expiration() {
    let message =
        AgentMessage::new("sender", MessageContent::Ping).with_priority(MessagePriority::Normal);

    // Message without TTL should not be expired
    assert!(!message.header.is_expired());
}

#[tokio::test]
async fn test_handler_registry() {
    let registry = MessageHandlerRegistry::new();

    // Register handlers
    registry.register("test", EchoHandler);
    registry.register("log", LoggingHandler);

    // Check registered types
    let types = registry.registered_types();
    assert!(types.contains(&"test".to_string()));
    assert!(types.contains(&"log".to_string()));

    // Get handlers
    assert!(registry.get_handler("test").is_some());
    assert!(registry.get_handler("log").is_some());
    assert!(registry.get_handler("unknown").is_none());

    // Unregister
    registry.unregister("test");
    assert!(registry.get_handler("test").is_none());
}

#[tokio::test]
async fn test_message_bus_basic() {
    let bus = MessageBus::new();

    // Register echo handler
    bus.register_handler("command", EchoHandler);

    // Send message
    let message = AgentMessage::new("sender", MessageContent::command("test", vec![]));
    let result = bus.send(message).await;

    // Should get response from handler
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.is_some());
}

#[tokio::test]
async fn test_message_bus_no_handler() {
    let bus = MessageBus::new();

    // Send message without handler
    let message = AgentMessage::new("sender", MessageContent::Ping);
    let result = bus.send(message).await;

    // Should succeed with None (fire-and-forget)
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[tokio::test]
async fn test_message_content_helpers() {
    // Test command
    let cmd = MessageContent::command("run", vec!["--verbose".to_string()]);
    match cmd {
        MessageContent::Command { command, args } => {
            assert_eq!(command, "run");
            assert_eq!(args, vec!["--verbose"]);
        }
        _ => panic!("Expected Command variant"),
    }

    // Test response success
    let resp = MessageContent::response_success(serde_json::json!({"data": 123})).unwrap();
    match resp {
        MessageContent::Response {
            success,
            data,
            error,
        } => {
            assert!(success);
            assert!(data.is_some());
            assert!(error.is_none());
        }
        _ => panic!("Expected Response variant"),
    }

    // Test response error
    let resp = MessageContent::response_error("Something went wrong");
    match resp {
        MessageContent::Response {
            success,
            data,
            error,
        } => {
            assert!(!success);
            assert!(data.is_none());
            assert_eq!(error, Some("Something went wrong".to_string()));
        }
        _ => panic!("Expected Response variant"),
    }

    // Test event
    let event =
        MessageContent::event("user_action", serde_json::json!({"button": "click"})).unwrap();
    match event {
        MessageContent::Event {
            event_type,
            payload,
        } => {
            assert_eq!(event_type, "user_action");
            assert!(payload.get("button").is_some());
        }
        _ => panic!("Expected Event variant"),
    }
}

#[tokio::test]
async fn test_message_correlation() {
    let original = AgentMessage::new(
        "sender",
        MessageContent::Query {
            query_type: "status".to_string(),
            parameters: serde_json::json!({}),
        },
    );

    let correlation_id = original.id();

    // Create response with correlation
    let response = original.create_response(true, Some(serde_json::json!({"status": "ok"})), None);

    assert_eq!(response.header.correlation_id, Some(correlation_id));
    assert_eq!(response.header.recipient, Some("sender".to_string()));
}
