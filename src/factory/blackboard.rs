//! Shared state management for the multi-agent factory.
//!
//! The Blackboard provides typed artifact storage for inter-agent communication
//! during factory workflow execution. Internally backed by [`InMemoryMessageBus`]
//! from the coordination module, using `ContextPatch` envelopes for versioned writes.

use crate::coordination::{CoordinationEnvelope, CoordinationPayload, InMemoryMessageBus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The agent identity used for all blackboard-originated envelopes.
const ORCHESTRATOR_AGENT: &str = "factory_orchestrator";
/// Conversation id shared across all blackboard traffic.
const CONVERSATION_ID: &str = "factory_blackboard";
/// Topic used for artifact context patches.
const TOPIC: &str = "artifact";

/// Artifact types produced and consumed by factory agents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Artifact {
    /// Product Requirements Document from Business Analyst.
    Prd,
    /// UI/UX design specification.
    DesignSpec,
    /// Source code manifest (file listing or summary).
    SourceCode,
    /// Test case definitions.
    TestCases,
    /// Test execution results (pass/fail).
    TestResults,
    /// Deployment configuration.
    DeployConfig,
}

impl Artifact {
    /// Map an artifact variant to its context key on the bus.
    fn context_key(&self) -> &'static str {
        match self {
            Artifact::Prd => "artifact:prd",
            Artifact::DesignSpec => "artifact:design_spec",
            Artifact::SourceCode => "artifact:source_code",
            Artifact::TestCases => "artifact:test_cases",
            Artifact::TestResults => "artifact:test_results",
            Artifact::DeployConfig => "artifact:deploy_config",
        }
    }
}

/// A versioned artifact entry on the blackboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub value: Value,
    pub version: u64,
    pub updated_by: String,
}

/// Shared state layer for inter-agent communication in the factory workflow.
///
/// Thread-safe, cloneable. Agents publish artifacts via `publish_artifact()` and
/// read them via `read_artifact()`. Internally delegates to [`InMemoryMessageBus`].
#[derive(Debug, Clone)]
pub struct Blackboard {
    bus: InMemoryMessageBus,
}

impl Blackboard {
    /// Create a new empty Blackboard.
    pub fn new() -> Self {
        let bus = InMemoryMessageBus::new();
        bus.register_agent(ORCHESTRATOR_AGENT)
            .expect("register factory_orchestrator agent");
        Self { bus }
    }

    /// Publish an artifact to the blackboard.
    ///
    /// Overwrites any existing value for the same artifact type.
    /// The version is incremented automatically via `ContextPatch`.
    pub fn publish_artifact(&self, artifact: Artifact, value: Value, _from: &str) {
        let key = artifact.context_key();
        let expected_version = self.bus.context_entry(key).map(|e| e.version).unwrap_or(0);

        let envelope = CoordinationEnvelope::new_broadcast(
            ORCHESTRATOR_AGENT,
            CONVERSATION_ID,
            TOPIC,
            CoordinationPayload::ContextPatch {
                key: key.to_string(),
                expected_version,
                value,
            },
        );

        self.bus
            .publish(envelope)
            .expect("blackboard publish must succeed");
    }

    /// Read an artifact value from the blackboard.
    ///
    /// Returns `None` if the artifact has not been published yet.
    pub fn read_artifact(&self, artifact: &Artifact) -> Option<Value> {
        self.bus
            .context_entry(artifact.context_key())
            .map(|e| e.value)
    }

    /// Read a full artifact entry (includes version metadata).
    pub fn read_entry(&self, artifact: &Artifact) -> Option<ArtifactEntry> {
        self.bus
            .context_entry(artifact.context_key())
            .map(|e| ArtifactEntry {
                value: e.value,
                version: e.version,
                updated_by: e.updated_by,
            })
    }

    /// Check if an artifact has been published.
    pub fn has_artifact(&self, artifact: &Artifact) -> bool {
        self.bus.context_entry(artifact.context_key()).is_some()
    }
}

impl Default for Blackboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn publish_and_read_artifact() {
        let board = Blackboard::new();
        let prd = json!({
            "title": "Todo App",
            "features": ["add tasks", "delete tasks"]
        });

        board.publish_artifact(Artifact::Prd, prd.clone(), "business_analyst");

        let read = board.read_artifact(&Artifact::Prd);
        assert_eq!(read, Some(prd));
    }

    #[test]
    fn read_missing_artifact_returns_none() {
        let board = Blackboard::new();
        assert!(board.read_artifact(&Artifact::DesignSpec).is_none());
    }

    #[test]
    fn has_artifact_tracks_publication() {
        let board = Blackboard::new();
        assert!(!board.has_artifact(&Artifact::TestResults));

        board.publish_artifact(Artifact::TestResults, json!({"passed": true}), "tester");

        assert!(board.has_artifact(&Artifact::TestResults));
    }

    #[test]
    fn overwrite_artifact_increments_version() {
        let board = Blackboard::new();

        board.publish_artifact(Artifact::SourceCode, json!({"v": 1}), "developer");
        let entry1 = board.read_entry(&Artifact::SourceCode).unwrap();
        assert_eq!(entry1.version, 1);

        board.publish_artifact(Artifact::SourceCode, json!({"v": 2}), "developer");
        let entry2 = board.read_entry(&Artifact::SourceCode).unwrap();
        assert_eq!(entry2.version, 2);
        assert_eq!(entry2.value, json!({"v": 2}));
    }

    #[test]
    fn artifact_keys_are_distinct() {
        let board = Blackboard::new();

        board.publish_artifact(Artifact::Prd, json!("prd"), "ba");
        board.publish_artifact(Artifact::DesignSpec, json!("design"), "uiux");

        assert_eq!(board.read_artifact(&Artifact::Prd), Some(json!("prd")));
        assert_eq!(
            board.read_artifact(&Artifact::DesignSpec),
            Some(json!("design"))
        );
    }

    #[test]
    fn entry_tracks_author() {
        let board = Blackboard::new();
        board.publish_artifact(Artifact::TestCases, json!("tests"), "tester");

        let entry = board.read_entry(&Artifact::TestCases).unwrap();
        assert_eq!(entry.updated_by, ORCHESTRATOR_AGENT);
    }

    #[test]
    fn thread_safe_clone() {
        let board = Blackboard::new();
        let board2 = board.clone();

        board.publish_artifact(Artifact::Prd, json!("shared"), "ba");
        assert_eq!(board2.read_artifact(&Artifact::Prd), Some(json!("shared")));
    }
}
