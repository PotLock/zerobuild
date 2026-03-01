//! Local process sandbox provider — runs commands in an isolated temp directory.
//!
//! No external API key or Docker daemon required. Creates a temporary directory
//! under `$TMPDIR/zerobuild-sandbox-{uuid}/`, runs commands via
//! `tokio::process::Command` with a restricted environment, and constrains all
//! file operations to the sandbox directory (rejects `..` path components).
//!
//! **Isolation model:**
//! - Filesystem: path-constrained to sandbox dir; `..` components are rejected.
//! - Environment: cleared + minimal safe set (`PATH`, `LANG`, `TERM`) + redirects
//!   (`HOME`, `TMPDIR`, `NPM_CONFIG_CACHE`, `NPM_CONFIG_PREFIX` → sandbox dir).
//! - Network: unrestricted (npm downloads must work).
//! - Timeout: `tokio::time::timeout` + `kill_on_drop(true)` for child cleanup.
//!
//! **Trade-off:** processes run as the same OS user — no kernel-level syscall
//! filtering. Acceptable for a trusted LLM building its own code. Main
//! protection: prevents accidental writes outside sandbox dir and credential
//! leaks via HOME.

use super::{CommandOutput, SandboxClient};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Directories skipped when collecting a snapshot.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".next",
    ".git",
    "dist",
    "build",
    ".cache",
    ".npm-cache",
];

/// Local-process sandbox client.
///
/// Stores the absolute path to the active sandbox directory as its "ID".
pub struct LocalProcessSandboxClient {
    /// Absolute path of the active sandbox directory, stored as its ID.
    sandbox_id: Arc<Mutex<Option<String>>>,
}

impl LocalProcessSandboxClient {
    /// Create a new client with no active sandbox.
    pub fn new() -> Self {
        Self {
            sandbox_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Resolve `relative` against `sandbox_dir`, rejecting any `..` components.
    ///
    /// Returns an error if `relative` attempts to escape the sandbox.
    fn safe_join(sandbox_dir: &Path, relative: &str) -> anyhow::Result<PathBuf> {
        // Strip leading '/' so we treat the path as relative
        let stripped = relative.trim_start_matches('/');
        let mut result = sandbox_dir.to_path_buf();

        for component in Path::new(stripped).components() {
            match component {
                Component::ParentDir => {
                    anyhow::bail!(
                        "Path traversal rejected: '{}' contains '..' components",
                        relative
                    );
                }
                Component::Normal(part) => {
                    result.push(part);
                }
                // RootDir / Prefix / CurDir are all no-ops in this context
                _ => {}
            }
        }

        Ok(result)
    }
}

impl Default for LocalProcessSandboxClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxClient for LocalProcessSandboxClient {
    async fn create_sandbox(
        &self,
        reset: bool,
        _template: &str,
        _timeout_ms: u64,
    ) -> anyhow::Result<String> {
        // Reuse existing sandbox unless reset is requested
        if !reset {
            if let Some(id) = self.sandbox_id.lock().clone() {
                if std::path::Path::new(&id).exists() {
                    return Ok(id);
                }
            }
        }

        // Remove old sandbox dir if present
        let old_id = self.sandbox_id.lock().clone();
        if let Some(ref old_path) = old_id {
            let _ = std::fs::remove_dir_all(old_path);
        }
        *self.sandbox_id.lock() = None;

        // Create new sandbox dir: $TMPDIR/zerobuild-sandbox-{uuid}/
        let tmp_base = std::env::temp_dir();
        let sandbox_dir = tmp_base.join(format!("zerobuild-sandbox-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&sandbox_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create sandbox dir: {e}"))?;

        // Pre-create sub-directories used for npm cache redirection
        for sub in &[".npm-cache", ".npm-global", "tmp"] {
            std::fs::create_dir_all(sandbox_dir.join(sub))
                .map_err(|e| anyhow::anyhow!("Failed to create sandbox subdir '{sub}': {e}"))?;
        }

        let id = sandbox_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Sandbox path is not valid UTF-8"))?
            .to_string();

        *self.sandbox_id.lock() = Some(id.clone());
        tracing::info!("Local sandbox created at {id}");
        Ok(id)
    }

    async fn kill_sandbox(&self) -> anyhow::Result<String> {
        let id = match self.sandbox_id.lock().clone() {
            Some(id) => id,
            None => return Ok("No active local sandbox to kill.".to_string()),
        };

        *self.sandbox_id.lock() = None;
        let _ = std::fs::remove_dir_all(&id);
        tracing::info!("Local sandbox removed: {id}");
        Ok(format!("Local sandbox {id} removed."))
    }

    async fn run_command(
        &self,
        command: &str,
        workdir: &str,
        timeout_ms: u64,
    ) -> anyhow::Result<CommandOutput> {
        let sandbox_dir = self.sandbox_id.lock().clone().ok_or_else(|| {
            anyhow::anyhow!("No active local sandbox. Call sandbox_create first.")
        })?;

        let sandbox_path = PathBuf::from(&sandbox_dir);

        // Resolve workdir inside sandbox, creating it if necessary
        let resolved_workdir = if workdir.is_empty() || workdir == "/" {
            sandbox_path.clone()
        } else {
            Self::safe_join(&sandbox_path, workdir)?
        };
        std::fs::create_dir_all(&resolved_workdir)
            .map_err(|e| anyhow::anyhow!("Failed to create workdir: {e}"))?;

        // Build restricted environment
        let path_val =
            std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string());
        let lang_val = std::env::var("LANG").unwrap_or_else(|_| "en_US.UTF-8".to_string());
        let npm_cache = sandbox_path.join(".npm-cache");
        let npm_global = sandbox_path.join(".npm-global");
        let tmp_dir = sandbox_path.join("tmp");

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&resolved_workdir)
            .env_clear()
            .env("PATH", &path_val)
            .env("HOME", &sandbox_path)
            .env("TMPDIR", &tmp_dir)
            .env("NPM_CONFIG_CACHE", &npm_cache)
            .env("NPM_CONFIG_PREFIX", &npm_global)
            .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
            .env("NEXT_TELEMETRY_DISABLED", "1")
            .env("CI", "1")
            .env("LANG", &lang_val)
            .env("TERM", "xterm-256color")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn command: {e}"))?;

        let timeout_result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            child.wait_with_output(),
        )
        .await;

        match timeout_result {
            Err(_elapsed) => {
                // kill_on_drop handles the process; return a timeout indicator
                Ok(CommandOutput {
                    stdout: String::new(),
                    stderr: format!("Command timed out after {timeout_ms}ms"),
                    exit_code: -1,
                })
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Command execution failed: {e}")),
            Ok(Ok(output)) => {
                let exit_code = output.status.code().map(i64::from).unwrap_or(-1);
                Ok(CommandOutput {
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                    exit_code,
                })
            }
        }
    }

    async fn write_file(&self, path: &str, content: &str) -> anyhow::Result<()> {
        let sandbox_dir = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active local sandbox."))?;

        let target = Self::safe_join(Path::new(&sandbox_dir), path)?;

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Failed to create parent dirs for '{path}': {e}"))?;
        }

        std::fs::write(&target, content)
            .map_err(|e| anyhow::anyhow!("Failed to write file '{path}': {e}"))
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let sandbox_dir = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active local sandbox."))?;

        let target = Self::safe_join(Path::new(&sandbox_dir), path)?;

        std::fs::read_to_string(&target)
            .map_err(|e| anyhow::anyhow!("Failed to read file '{path}': {e}"))
    }

    async fn list_files(&self, path: &str) -> anyhow::Result<String> {
        let sandbox_dir = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active local sandbox."))?;

        let target = Self::safe_join(Path::new(&sandbox_dir), path)?;

        let mut entries: Vec<String> = std::fs::read_dir(&target)
            .map_err(|e| anyhow::anyhow!("Failed to list directory '{path}': {e}"))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let file_type = entry.file_type().ok()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if file_type.is_dir() {
                    Some(format!("dir\t{name}"))
                } else {
                    Some(format!("file\t{name}"))
                }
            })
            .collect();

        entries.sort();
        Ok(entries.join("\n"))
    }

    async fn get_preview_url(&self, port: u16) -> anyhow::Result<String> {
        Ok(format!("http://localhost:{port}"))
    }

    async fn collect_snapshot_files(
        &self,
        workdir: &str,
    ) -> anyhow::Result<HashMap<String, String>> {
        let sandbox_dir = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active local sandbox."))?;

        let base = Self::safe_join(Path::new(&sandbox_dir), workdir)?;

        let mut files = HashMap::new();
        collect_files_recursive(&base, &base, &mut files);
        Ok(files)
    }

    fn current_id(&self) -> Option<String> {
        self.sandbox_id.lock().clone()
    }

    fn set_id(&self, id: String) {
        *self.sandbox_id.lock() = Some(id);
    }

    fn clear_id(&self) {
        *self.sandbox_id.lock() = None;
    }
}

/// Recursively walk `dir`, skip [`SKIP_DIRS`], and collect readable text files
/// into `out` keyed by path relative to `base`.
fn collect_files_recursive(base: &Path, dir: &Path, out: &mut HashMap<String, String>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::debug!("Skipping unreadable dir {}: {e}", dir.display());
            return;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if SKIP_DIRS.contains(&name_str.as_ref()) {
            continue;
        }

        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            collect_files_recursive(base, &path, out);
        } else if file_type.is_file() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    // Key is path relative to base
                    let rel = path
                        .strip_prefix(base)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
                    out.insert(rel, content);
                }
                Err(e) => {
                    tracing::debug!("Skipping non-text file {}: {e}", path.display());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_normal_path() {
        let base = PathBuf::from("/tmp/sandbox");
        let result = LocalProcessSandboxClient::safe_join(&base, "src/main.rs").unwrap();
        assert_eq!(result, PathBuf::from("/tmp/sandbox/src/main.rs"));
    }

    #[test]
    fn safe_join_strips_leading_slash() {
        let base = PathBuf::from("/tmp/sandbox");
        let result = LocalProcessSandboxClient::safe_join(&base, "/src/main.rs").unwrap();
        assert_eq!(result, PathBuf::from("/tmp/sandbox/src/main.rs"));
    }

    #[test]
    fn safe_join_rejects_parent_dir() {
        let base = PathBuf::from("/tmp/sandbox");
        let err = LocalProcessSandboxClient::safe_join(&base, "../etc/passwd").unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn safe_join_rejects_embedded_parent_dir() {
        let base = PathBuf::from("/tmp/sandbox");
        let err = LocalProcessSandboxClient::safe_join(&base, "src/../../etc/passwd").unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn safe_join_empty_relative() {
        let base = PathBuf::from("/tmp/sandbox");
        let result = LocalProcessSandboxClient::safe_join(&base, "").unwrap();
        assert_eq!(result, PathBuf::from("/tmp/sandbox"));
    }

    #[test]
    fn new_client_has_no_id() {
        let client = LocalProcessSandboxClient::new();
        assert!(client.current_id().is_none());
    }

    #[test]
    fn set_and_clear_id() {
        let client = LocalProcessSandboxClient::new();
        client.set_id("/tmp/zerobuild-sandbox-test".to_string());
        assert_eq!(
            client.current_id().as_deref(),
            Some("/tmp/zerobuild-sandbox-test")
        );
        client.clear_id();
        assert!(client.current_id().is_none());
    }

    #[tokio::test]
    async fn create_and_kill_sandbox() {
        let client = LocalProcessSandboxClient::new();
        let id = client.create_sandbox(false, "", 30_000).await.unwrap();
        assert!(std::path::Path::new(&id).exists());
        let msg = client.kill_sandbox().await.unwrap();
        assert!(msg.contains(&id));
        assert!(!std::path::Path::new(&id).exists());
    }

    #[tokio::test]
    async fn create_sandbox_reuses_existing() {
        let client = LocalProcessSandboxClient::new();
        let id1 = client.create_sandbox(false, "", 30_000).await.unwrap();
        let id2 = client.create_sandbox(false, "", 30_000).await.unwrap();
        assert_eq!(id1, id2);
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn create_sandbox_reset_creates_new() {
        let client = LocalProcessSandboxClient::new();
        let id1 = client.create_sandbox(false, "", 30_000).await.unwrap();
        let id2 = client.create_sandbox(true, "", 30_000).await.unwrap();
        assert_ne!(id1, id2);
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn write_and_read_file() {
        let client = LocalProcessSandboxClient::new();
        client.create_sandbox(false, "", 30_000).await.unwrap();
        client
            .write_file("hello.txt", "Hello, sandbox!")
            .await
            .unwrap();
        let content = client.read_file("hello.txt").await.unwrap();
        assert_eq!(content, "Hello, sandbox!");
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn write_file_rejects_path_traversal() {
        let client = LocalProcessSandboxClient::new();
        client.create_sandbox(false, "", 30_000).await.unwrap();
        let err = client.write_file("../escape.txt", "bad").await.unwrap_err();
        assert!(err.to_string().contains(".."));
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn run_command_captures_output() {
        let client = LocalProcessSandboxClient::new();
        client.create_sandbox(false, "", 30_000).await.unwrap();
        let out = client.run_command("echo hello", "", 10_000).await.unwrap();
        assert_eq!(out.stdout.trim(), "hello");
        assert_eq!(out.exit_code, 0);
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn run_command_timeout() {
        let client = LocalProcessSandboxClient::new();
        client.create_sandbox(false, "", 30_000).await.unwrap();
        let out = client.run_command("sleep 10", "", 100).await.unwrap();
        assert_eq!(out.exit_code, -1);
        assert!(out.stderr.contains("timed out"));
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn list_files_returns_entries() {
        let client = LocalProcessSandboxClient::new();
        client.create_sandbox(false, "", 30_000).await.unwrap();
        client.write_file("a.txt", "a").await.unwrap();
        client.write_file("b.txt", "b").await.unwrap();
        let listing = client.list_files("").await.unwrap();
        assert!(listing.contains("a.txt"));
        assert!(listing.contains("b.txt"));
        client.kill_sandbox().await.unwrap();
    }

    #[tokio::test]
    async fn get_preview_url_returns_localhost() {
        let client = LocalProcessSandboxClient::new();
        let url = client.get_preview_url(3000).await.unwrap();
        assert_eq!(url, "http://localhost:3000");
    }
}
