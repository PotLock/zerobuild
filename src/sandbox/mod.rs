//! Sandbox abstraction layer for ZeroBuild.
//!
//! Defines the [`SandboxClient`] trait and [`CommandOutput`] type that all
//! sandbox providers must implement. Currently one provider exists:
//!
//! - [`local::LocalProcessSandboxClient`] — native process sandbox (no external deps)
//!
//! The factory in [`crate::tools::mod`] selects the provider at startup.

pub mod local;

use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;

/// Package manager types supported by the sandbox, ordered by priority.
/// Priority: pnpm > yarn > npm
///
/// Note: Default is Npm as the safe fallback, but detection prioritizes pnpm/yarn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackageManager {
    /// pnpm - fastest, most disk efficient (highest priority)
    Pnpm,
    /// yarn - good performance, widely adopted
    Yarn,
    /// npm - default fallback, always available
    #[default]
    Npm,
}

impl PackageManager {
    /// Get the command name for this package manager.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Npm => "npm",
        }
    }

    /// Get the install command for this package manager.
    pub fn install_cmd(&self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm install",
            Self::Yarn => "yarn install",
            Self::Npm => "npm install",
        }
    }

    /// Get the add command prefix for this package manager.
    pub fn add_cmd(&self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm add",
            Self::Yarn => "yarn add",
            Self::Npm => "npm install",
        }
    }

    /// Get the run command prefix for this package manager.
    pub fn run_cmd(&self) -> &'static str {
        match self {
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Npm => "npm run",
        }
    }

    /// Detect available package managers in order of priority.
    /// Returns the highest priority available manager.
    pub async fn detect() -> Self {
        // Check pnpm first
        if Self::is_available("pnpm").await {
            return Self::Pnpm;
        }
        // Then yarn
        if Self::is_available("yarn").await {
            return Self::Yarn;
        }
        // Fallback to npm
        Self::Npm
    }

    /// Check if a command is available in PATH.
    async fn is_available(cmd: &str) -> bool {
        match tokio::process::Command::new("which")
            .arg(cmd)
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
}

impl fmt::Display for PackageManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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

    /// Start a Cloudflare Quick Tunnel to expose a local port publicly.
    /// Returns the public `https://xxx.trycloudflare.com` URL.
    /// Default impl bails — only LocalProcessSandboxClient implements this.
    async fn start_tunnel(&self, _port: u16) -> anyhow::Result<String> {
        anyhow::bail!("Public tunnel not supported by this sandbox provider")
    }

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

    /// Get the detected package manager for this sandbox.
    /// Returns the highest priority available: pnpm > yarn > npm
    fn package_manager(&self) -> PackageManager;

    /// Set the package manager for this sandbox.
    fn set_package_manager(&self, pm: PackageManager);

    /// Detect and set the best available package manager.
    async fn detect_package_manager(&self) -> PackageManager {
        let pm = PackageManager::detect().await;
        self.set_package_manager(pm);
        pm
    }
}
