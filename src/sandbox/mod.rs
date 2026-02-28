//! Sandbox abstraction layer for ZeroBuild.
//!
//! Defines the [`SandboxClient`] trait and [`CommandOutput`] type that all
//! sandbox providers must implement. Currently two providers exist:
//!
//! - [`e2b::E2bSandboxClient`] — E2B cloud MicroVM (requires `E2B_API_KEY`)
//! - [`docker::DockerSandboxClient`] — local Docker container (no API key needed)
//!
//! The factory in [`crate::tools::mod`] auto-selects the provider at startup.

pub mod docker;
pub mod e2b;

use async_trait::async_trait;
use std::collections::HashMap;

/// Output from a command executed inside a sandbox.
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i64,
}

/// Provider-agnostic sandbox interface.
///
/// All methods are async and require an active sandbox (created via
/// [`create_sandbox`]). The `current_id` / `set_id` / `clear_id` helpers
/// manage the live sandbox identifier in interior-mutable state.
#[async_trait]
pub trait SandboxClient: Send + Sync {
    /// Create (or reset) a sandbox. Returns the sandbox/container ID.
    async fn create_sandbox(
        &self,
        reset: bool,
        template: &str,
        timeout_ms: u64,
    ) -> anyhow::Result<String>;

    /// Terminate the active sandbox. Returns a status message.
    async fn kill_sandbox(&self) -> anyhow::Result<String>;

    /// Run a shell command inside the sandbox.
    async fn run_command(
        &self,
        command: &str,
        workdir: &str,
        timeout_ms: u64,
    ) -> anyhow::Result<CommandOutput>;

    /// Write content to a file path inside the sandbox.
    async fn write_file(&self, path: &str, content: &str) -> anyhow::Result<()>;

    /// Read a file from the sandbox and return its content as a UTF-8 string.
    async fn read_file(&self, path: &str) -> anyhow::Result<String>;

    /// List entries at a directory path. Returns a human-readable string.
    async fn list_files(&self, path: &str) -> anyhow::Result<String>;

    /// Return the public preview URL for a given port.
    async fn get_preview_url(&self, port: u16) -> anyhow::Result<String>;

    /// Walk `workdir` (skipping build artifacts) and return a map of
    /// `path → content` for all source files.
    async fn collect_snapshot_files(
        &self,
        workdir: &str,
    ) -> anyhow::Result<HashMap<String, String>>;

    /// Return the current sandbox/container ID, if any.
    fn current_id(&self) -> Option<String>;

    /// Store a new sandbox/container ID.
    fn set_id(&self, id: String);

    /// Clear the current ID (sandbox has been terminated).
    fn clear_id(&self);

    /// Return the current ID or an error message suitable for `ToolResult`.
    fn require_id(&self) -> Result<String, String> {
        self.current_id()
            .ok_or_else(|| "No active sandbox. Call sandbox_create first.".to_string())
    }
}
