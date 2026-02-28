//! Sandbox tools for the ZeroBuild Master Agent.
//!
//! Provides 8 provider-agnostic tools that work with both E2B cloud sandboxes
//! and local Docker containers. The tools are thin delegators to the
//! [`SandboxClient`] trait implementations.
//!
//! API-agnostic: All HTTP/Docker logic lives in [`crate::sandbox`] modules.

pub mod command;
pub mod create;
pub mod files;
pub mod kill;
pub mod preview;
pub mod snapshot;

pub use command::SandboxRunCommandTool;
pub use create::SandboxCreateTool;
pub use files::{SandboxListFilesTool, SandboxReadFileTool, SandboxWriteFileTool};
pub use kill::SandboxKillTool;
pub use preview::SandboxGetPreviewUrlTool;
pub use snapshot::SandboxSaveSnapshotTool;

/// Tool name constants for reference.
pub const TOOL_CREATE: &str = "sandbox_create";
pub const TOOL_RUN_COMMAND: &str = "sandbox_run_command";
pub const TOOL_WRITE_FILE: &str = "sandbox_write_file";
pub const TOOL_READ_FILE: &str = "sandbox_read_file";
pub const TOOL_LIST_FILES: &str = "sandbox_list_files";
pub const TOOL_GET_PREVIEW_URL: &str = "sandbox_get_preview_url";
pub const TOOL_SAVE_SNAPSHOT: &str = "sandbox_save_snapshot";
pub const TOOL_KILL: &str = "sandbox_kill";
