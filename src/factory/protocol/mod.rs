//! Inter-agent Communication Protocol
//!
//! Provides type-safe, extensible message passing between agents.
//! Supports dynamic message type registration for future extension.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Unique identifier for a message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessagePriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Message header containing routing and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHeader {
    pub id: MessageId,
    pub correlation_id: Option<MessageId>,
    pub sender: String,
    pub recipient: Option<String>,
    pub priority: MessagePriority,
    pub timestamp: DateTime<Utc>,
    pub ttl_seconds: Option<u64>,
    pub headers: HashMap<String, String>,
}

impl MessageHeader {
    pub fn new(sender: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            correlation_id: None,
            sender: sender.into(),
            recipient: None,
            priority: MessagePriority::default(),
            timestamp: Utc::now(),
            ttl_seconds: None,
            headers: HashMap::new(),
        }
    }

    pub fn to_recipient(mut self, recipient: impl Into<String>) -> Self {
        self.recipient = Some(recipient.into());
        self
    }

    pub fn with_correlation(mut self, correlation_id: MessageId) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = Some(ttl_seconds);
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Check if the message has expired based on TTL
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_seconds {
            let elapsed = Utc::now().signed_duration_since(self.timestamp);
            elapsed.num_seconds() > ttl as i64
        } else {
            false
        }
    }
}

/// Core message content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum MessageContent {
    /// Command to execute an action
    Command {
        command: String,
        args: Vec<String>,
    },
    /// Query requesting information
    Query {
        query_type: String,
        parameters: serde_json::Value,
    },
    /// Response to a query or command
    Response {
        success: bool,
        data: Option<serde_json::Value>,
        error: Option<String>,
    },
    /// Event notification (fire-and-forget)
    Event {
        event_type: String,
        payload: serde_json::Value,
    },
    /// Artifact update notification
    ArtifactUpdate {
        artifact_name: String,
        version: u64,
        summary: String,
    },
    /// Ping/Pong for health checks
    Ping,
    Pong,
    /// Custom message type for dynamic registration
    Custom {
        type_name: String,
        payload: serde_json::Value,
    },
}

impl MessageContent {
    pub fn command(name: impl Into<String>, args: Vec<String>) -> Self {
        Self::Command {
            command: name.into(),
            args,
        }
    }

    pub fn query(query_type: impl Into<String>, parameters: serde_json::Value) -> Self {
        Self::Query {
            query_type: query_type.into(),
            parameters,
        }
    }

    pub fn response_success(data: impl Serialize) -> anyhow::Result<Self> {
        let data = serde_json::to_value(data)?;
        Ok(Self::Response {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn response_error(error: impl Into<String>) -> Self {
        Self::Response {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }

    pub fn event(event_type: impl Into<String>, payload: impl Serialize) -> anyhow::Result<Self> {
        let payload = serde_json::to_value(payload)?;
        Ok(Self::Event {
            event_type: event_type.into(),
            payload,
        })
    }
}

/// Complete agent message with header and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub header: MessageHeader,
    pub content: MessageContent,
}

impl AgentMessage {
    pub fn new(sender: impl Into<String>, content: MessageContent) -> Self {
        Self {
            header: MessageHeader::new(sender),
            content,
        }
    }

    pub fn with_recipient(mut self, recipient: impl Into<String>) -> Self {
        self.header = self.header.to_recipient(recipient);
        self
    }

    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.header = self.header.with_priority(priority);
        self
    }

    pub fn with_correlation(mut self, correlation_id: MessageId) -> Self {
        self.header = self.header.with_correlation(correlation_id);
        self
    }

    /// Create a response to this message
    pub fn create_response(
        &self,
        success: bool,
        data: Option<serde_json::Value>,
        error: Option<String>,
    ) -> Self {
        Self {
            header: MessageHeader::new(&self.header.recipient.clone().unwrap_or_default())
                .to_recipient(&self.header.sender)
                .with_correlation(self.header.id)
                .with_priority(self.header.priority),
            content: MessageContent::Response {
                success,
                data,
                error,
            },
        }
    }

    pub fn id(&self) -> MessageId {
        self.header.id
    }
}

/// Errors that can occur in message handling
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Unknown message type: {0}")]
    UnknownMessageType(String),

    #[error("Message expired: {0}")]
    MessageExpired(MessageId),

    #[error("Invalid recipient: {0}")]
    InvalidRecipient(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Handler error: {0}")]
    Handler(String),
}

/// Trait for message handlers
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message
    async fn handle(&self, message: AgentMessage) -> Result<Option<AgentMessage>, ProtocolError>;

    /// Returns true if this handler can handle the given message content type
    fn can_handle(&self, content_type: &str) -> bool;
}

/// Registry for dynamic message type handlers
pub struct MessageHandlerRegistry {
    handlers: RwLock<HashMap<String, Arc<dyn MessageHandler>>>,
}

impl Default for MessageHandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageHandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a handler for a specific message type
    pub fn register<H>(&self, message_type: impl Into<String>, handler: H)
    where
        H: MessageHandler + 'static,
    {
        let mut handlers = self.handlers.write();
        handlers.insert(message_type.into(), Arc::new(handler));
    }

    /// Get a handler for a specific message type
    pub fn get_handler(&self, message_type: &str) -> Option<Arc<dyn MessageHandler>> {
        let handlers = self.handlers.read();
        handlers.get(message_type).cloned()
    }

    /// Unregister a handler
    pub fn unregister(&self, message_type: &str) -> Option<Arc<dyn MessageHandler>> {
        let mut handlers = self.handlers.write();
        handlers.remove(message_type)
    }

    /// List all registered message types
    pub fn registered_types(&self) -> Vec<String> {
        let handlers = self.handlers.read();
        handlers.keys().cloned().collect()
    }
}

/// Message bus for routing messages between agents
pub struct MessageBus {
    registry: Arc<MessageHandlerRegistry>,
    interceptors: RwLock<Vec<Box<dyn MessageInterceptor>>>,
}

/// Trait for message interceptors (middleware)
pub trait MessageInterceptor: Send + Sync {
    /// Intercept and optionally modify a message before it's handled
    fn intercept_inbound(&self, message: AgentMessage) -> Result<AgentMessage, ProtocolError>;

    /// Intercept and optionally modify a message before it's sent
    fn intercept_outbound(&self, message: AgentMessage) -> Result<AgentMessage, ProtocolError>;
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBus {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(MessageHandlerRegistry::new()),
            interceptors: RwLock::new(Vec::new()),
        }
    }

    pub fn with_registry(registry: Arc<MessageHandlerRegistry>) -> Self {
        Self {
            registry,
            interceptors: RwLock::new(Vec::new()),
        }
    }

    /// Add an interceptor to the message pipeline
    pub fn add_interceptor<I>(&self, interceptor: I)
    where
        I: MessageInterceptor + 'static,
    {
        let mut interceptors = self.interceptors.write();
        interceptors.push(Box::new(interceptor));
    }

    /// Send a message through the bus
    pub async fn send(&self, message: AgentMessage) -> Result<Option<AgentMessage>, ProtocolError> {
        // Check for expiration first, before any moves
        if message.header.is_expired() {
            let id = message.id();
            return Err(ProtocolError::MessageExpired(id));
        }

        // Get content type before applying interceptors (avoids borrow issues)
        let content_type = match &message.content {
            MessageContent::Command { .. } => "command".to_string(),
            MessageContent::Query { query_type, .. } => query_type.clone(),
            MessageContent::Response { .. } => "response".to_string(),
            MessageContent::Event { event_type, .. } => event_type.clone(),
            MessageContent::ArtifactUpdate { .. } => "artifact_update".to_string(),
            MessageContent::Ping => "ping".to_string(),
            MessageContent::Pong => "pong".to_string(),
            MessageContent::Custom { type_name, .. } => type_name.clone(),
        };

        // Apply outbound interceptors
        let mut message = message;
        for interceptor in self.interceptors.read().iter() {
            message = interceptor.intercept_outbound(message)?;
        }

        // Apply inbound interceptors
        for interceptor in self.interceptors.read().iter() {
            message = interceptor.intercept_inbound(message)?;
        }

        // Find and invoke handler
        if let Some(handler) = self.registry.get_handler(&content_type) {
            handler.handle(message).await
        } else {
            // No handler registered, return Ok(None) for fire-and-forget
            Ok(None)
        }
    }

    /// Register a message handler
    pub fn register_handler<H>(&self, message_type: impl Into<String>, handler: H)
    where
        H: MessageHandler + 'static,
    {
        self.registry.register(message_type, handler);
    }

    /// Get access to the registry
    pub fn registry(&self) -> Arc<MessageHandlerRegistry> {
        Arc::clone(&self.registry)
    }
}

/// Default interceptor that logs all messages
pub struct LoggingInterceptor;

impl MessageInterceptor for LoggingInterceptor {
    fn intercept_inbound(&self, message: AgentMessage) -> Result<AgentMessage, ProtocolError> {
        tracing::debug!(
            "[INBOUND] {} -> {}: {:?}",
            message.header.sender,
            message.header.recipient.as_deref().unwrap_or("broadcast"),
            std::mem::discriminant(&message.content)
        );
        Ok(message)
    }

    fn intercept_outbound(&self, message: AgentMessage) -> Result<AgentMessage, ProtocolError> {
        tracing::debug!(
            "[OUTBOUND] {} -> {}: {:?}",
            message.header.sender,
            message.header.recipient.as_deref().unwrap_or("broadcast"),
            std::mem::discriminant(&message.content)
        );
        Ok(message)
    }
}

/// TTL interceptor that filters expired messages
pub struct TtlInterceptor;

impl MessageInterceptor for TtlInterceptor {
    fn intercept_inbound(&self, message: AgentMessage) -> Result<AgentMessage, ProtocolError> {
        if message.header.is_expired() {
            Err(ProtocolError::MessageExpired(message.id()))
        } else {
            Ok(message)
        }
    }

    fn intercept_outbound(&self, message: AgentMessage) -> Result<AgentMessage, ProtocolError> {
        // TTL check on outbound too
        if message.header.is_expired() {
            Err(ProtocolError::MessageExpired(message.id()))
        } else {
            Ok(message)
        }
    }
}

/// Create a default message bus with standard interceptors
pub fn create_default_bus() -> MessageBus {
    let bus = MessageBus::new();
    bus.add_interceptor(LoggingInterceptor);
    bus.add_interceptor(TtlInterceptor);
    bus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header_builder() {
        let header = MessageHeader::new("test_agent")
            .to_recipient("recipient_agent")
            .with_priority(MessagePriority::High)
            .with_header("key", "value");

        assert_eq!(header.sender, "test_agent");
        assert_eq!(header.recipient, Some("recipient_agent".to_string()));
        assert_eq!(header.priority, MessagePriority::High);
        assert_eq!(header.headers.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_message_expiration() {
        let header = MessageHeader::new("test").with_ttl(0); // 0 seconds TTL

        // Should be expired immediately due to time passing
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(header.is_expired());

        let header = MessageHeader::new("test");
        assert!(!header.is_expired()); // No TTL = never expires
    }

    #[test]
    fn test_message_content_helpers() {
        let cmd = MessageContent::command("test", vec!["arg1".to_string()]);
        match cmd {
            MessageContent::Command { command, args } => {
                assert_eq!(command, "test");
                assert_eq!(args, vec!["arg1"]);
            }
            _ => panic!("Expected Command variant"),
        }

        let resp = MessageContent::response_success("data").unwrap();
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
    }

    #[test]
    fn test_handler_registry() {
        let registry = MessageHandlerRegistry::new();

        struct TestHandler;
        #[async_trait::async_trait]
        impl MessageHandler for TestHandler {
            async fn handle(
                &self,
                _message: AgentMessage,
            ) -> Result<Option<AgentMessage>, ProtocolError> {
                Ok(None)
            }
            fn can_handle(&self, _content_type: &str) -> bool {
                true
            }
        }

        registry.register("test", TestHandler);
        assert!(registry.get_handler("test").is_some());
        assert!(registry.get_handler("unknown").is_none());

        let types = registry.registered_types();
        assert!(types.contains(&"test".to_string()));
    }
}
