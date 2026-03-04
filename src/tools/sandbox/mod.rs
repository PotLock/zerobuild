//! Sandbox tools for the ZeroBuild Agent.
//!
//! Provides 10+ tools backed by [`crate::sandbox::local::LocalProcessSandboxClient`]
//! — a native process sandbox that requires no external API key or Docker daemon.
//! The tools are thin delegators to the [`SandboxClient`] trait.

pub mod command;
pub mod create;
pub mod files;
pub mod kill;
pub mod package_manager;
pub mod preview;
pub mod restore;
pub mod snapshot;
pub mod tunnel;

pub use command::SandboxRunCommandTool;
pub use create::SandboxCreateTool;
pub use files::{SandboxListFilesTool, SandboxReadFileTool, SandboxWriteFileTool};
pub use kill::SandboxKillTool;
pub use package_manager::SandboxGetPackageManagerTool;
pub use preview::SandboxGetPreviewUrlTool;
pub use restore::SandboxRestoreSnapshotTool;
pub use snapshot::SandboxSaveSnapshotTool;
pub use tunnel::SandboxGetPublicUrlTool;

/// Tool name constants for reference.
pub const TOOL_CREATE: &str = "sandbox_create";
pub const TOOL_RUN_COMMAND: &str = "sandbox_run_command";
pub const TOOL_WRITE_FILE: &str = "sandbox_write_file";
pub const TOOL_READ_FILE: &str = "sandbox_read_file";
pub const TOOL_LIST_FILES: &str = "sandbox_list_files";
pub const TOOL_GET_PREVIEW_URL: &str = "sandbox_get_preview_url";
pub const TOOL_GET_PACKAGE_MANAGER: &str = "sandbox_get_package_manager";
pub const TOOL_SAVE_SNAPSHOT: &str = "sandbox_save_snapshot";
pub const TOOL_RESTORE_SNAPSHOT: &str = "sandbox_restore_snapshot";
pub const TOOL_KILL: &str = "sandbox_kill";
pub const TOOL_TUNNEL: &str = "sandbox_get_public_url";
