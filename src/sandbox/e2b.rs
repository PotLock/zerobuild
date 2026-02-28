//! E2B sandbox provider — HTTP client for the E2B REST API.
//!
//! Migrates the HTTP logic that was previously embedded in each tool's
//! `execute()` body into a single reusable `E2bSandboxClient` struct that
//! implements [`SandboxClient`].

use super::{CommandOutput, SandboxClient};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Base URL for the E2B REST API.
pub const E2B_API_BASE: &str = "https://api.e2b.dev";

/// Directories skipped when collecting a snapshot.
const SKIP_DIRS: &[&str] = &["node_modules", ".next", ".git", "dist", "build", ".cache"];

/// Shared HTTP client for E2B API calls.
pub struct E2bSandboxClient {
    pub api_key: String,
    pub sandbox_id: Arc<Mutex<Option<String>>>,
    pub http: reqwest::Client,
}

impl E2bSandboxClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            api_key: api_key.into(),
            sandbox_id: Arc::new(Mutex::new(None)),
            http,
        }
    }

    fn effective_key(&self) -> String {
        std::env::var("E2B_API_KEY").unwrap_or_else(|_| self.api_key.clone())
    }
}

#[async_trait]
impl SandboxClient for E2bSandboxClient {
    async fn create_sandbox(
        &self,
        reset: bool,
        template: &str,
        timeout_ms: u64,
    ) -> anyhow::Result<String> {
        if !reset {
            if let Some(id) = self.sandbox_id.lock().clone() {
                return Ok(id);
            }
        }

        let api_key = self.effective_key();
        anyhow::ensure!(!api_key.is_empty(), "E2B_API_KEY is not set");

        let url = format!("{E2B_API_BASE}/v0/sandboxes");
        let body = serde_json::json!({
            "templateID": template,
            "timeout": timeout_ms / 1000,
        });

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B create sandbox request failed: {e}"))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        anyhow::ensure!(
            status.is_success(),
            "E2B API returned {status}: {body_text}"
        );

        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse E2B response: {e}\nBody: {body_text}"))?;

        let sandbox_id = parsed["sandboxID"]
            .as_str()
            .or_else(|| parsed["sandbox_id"].as_str())
            .unwrap_or("")
            .to_string();

        anyhow::ensure!(
            !sandbox_id.is_empty(),
            "E2B returned no sandbox_id. Response: {body_text}"
        );

        *self.sandbox_id.lock() = Some(sandbox_id.clone());
        Ok(sandbox_id)
    }

    async fn kill_sandbox(&self) -> anyhow::Result<String> {
        let sandbox_id = match self.sandbox_id.lock().clone() {
            Some(id) => id,
            None => return Ok("No active sandbox to kill.".to_string()),
        };

        let api_key = self.effective_key();
        let url = format!("{E2B_API_BASE}/v0/sandboxes/{sandbox_id}");

        let resp = self
            .http
            .delete(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B kill request failed: {e}"))?;

        let status = resp.status();

        if status.is_success() || status == reqwest::StatusCode::NOT_FOUND {
            *self.sandbox_id.lock() = None;
            Ok(format!("Sandbox {sandbox_id} terminated."))
        } else {
            let body_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            anyhow::bail!("E2B API returned {status}: {body_text}")
        }
    }

    async fn run_command(
        &self,
        command: &str,
        workdir: &str,
        timeout_ms: u64,
    ) -> anyhow::Result<CommandOutput> {
        let sandbox_id = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active sandbox"))?;

        let api_key = self.effective_key();
        let url = format!("{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/commands");

        let body = serde_json::json!({
            "cmd": command,
            "workdir": workdir,
            "timeout": timeout_ms / 1000,
        });

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B run_command request failed: {e}"))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        anyhow::ensure!(
            status.is_success(),
            "E2B API returned {status}: {body_text}"
        );

        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse E2B response: {e}\nBody: {body_text}"))?;

        Ok(CommandOutput {
            stdout: parsed["stdout"].as_str().unwrap_or("").to_string(),
            stderr: parsed["stderr"].as_str().unwrap_or("").to_string(),
            exit_code: parsed["exitCode"]
                .as_i64()
                .or_else(|| parsed["exit_code"].as_i64())
                .unwrap_or(0),
        })
    }

    async fn write_file(&self, path: &str, content: &str) -> anyhow::Result<()> {
        let sandbox_id = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active sandbox"))?;

        let api_key = self.effective_key();
        let url = format!("{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/files");

        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        let form = reqwest::multipart::Form::new()
            .text("path", path.to_string())
            .part(
                "file",
                reqwest::multipart::Part::bytes(content.as_bytes().to_vec())
                    .file_name(file_name)
                    .mime_str("text/plain")
                    .map_err(|e| anyhow::anyhow!("MIME type error: {e}"))?,
            );

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .multipart(form)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B write_file request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            anyhow::bail!("E2B API returned {status}: {body_text}");
        }

        Ok(())
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let sandbox_id = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active sandbox"))?;

        let api_key = self.effective_key();
        let url = format!(
            "{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/files?path={encoded}",
            encoded = urlencoding::encode(path)
        );

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B read_file request failed: {e}"))?;

        let status = resp.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!("File not found: {path}");
        }

        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        anyhow::ensure!(
            status.is_success(),
            "E2B API returned {status}: {body_text}"
        );

        Ok(body_text)
    }

    async fn list_files(&self, path: &str) -> anyhow::Result<String> {
        let sandbox_id = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active sandbox"))?;

        let api_key = self.effective_key();
        let url = format!(
            "{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/files?path={encoded}",
            encoded = urlencoding::encode(path)
        );

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B list_files request failed: {e}"))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        anyhow::ensure!(
            status.is_success(),
            "E2B API returned {status}: {body_text}"
        );

        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).unwrap_or(serde_json::json!([]));

        if let Some(entries) = parsed.as_array() {
            let mut lines = vec![format!("Files in {path}:")];
            for entry in entries {
                let name = entry["name"].as_str().unwrap_or("<unnamed>");
                let entry_type = entry["type"].as_str().unwrap_or("file");
                lines.push(format!("  [{entry_type}] {name}"));
            }
            Ok(lines.join("\n"))
        } else {
            Ok(body_text)
        }
    }

    async fn get_preview_url(&self, port: u16) -> anyhow::Result<String> {
        let sandbox_id = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active sandbox"))?;

        let api_key = self.effective_key();
        let url = format!("{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/hosts/{port}");

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("E2B get_preview_url request failed: {e}"))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        anyhow::ensure!(
            status.is_success(),
            "E2B API returned {status}: {body_text}\nMake sure the dev server is running on port {port}."
        );

        let preview_url =
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body_text) {
                parsed["url"]
                    .as_str()
                    .or_else(|| parsed["host"].as_str())
                    .map(|u| {
                        if u.starts_with("http") {
                            u.to_string()
                        } else {
                            format!("https://{u}")
                        }
                    })
                    .unwrap_or_else(|| body_text.trim().trim_matches('"').to_string())
            } else {
                let raw = body_text.trim().trim_matches('"').to_string();
                if raw.starts_with("http") {
                    raw
                } else {
                    format!("https://{raw}")
                }
            };

        Ok(preview_url)
    }

    async fn collect_snapshot_files(
        &self,
        workdir: &str,
    ) -> anyhow::Result<HashMap<String, String>> {
        let sandbox_id = self
            .sandbox_id
            .lock()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active sandbox"))?;

        let api_key = self.effective_key();
        let mut files: HashMap<String, String> = HashMap::new();
        let mut dirs_to_visit = vec![workdir.to_string()];

        while let Some(dir) = dirs_to_visit.pop() {
            let list_url = format!(
                "{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/files?path={encoded}",
                encoded = urlencoding::encode(&dir)
            );

            let list_resp = match self
                .http
                .get(&list_url)
                .header("Authorization", format!("Bearer {api_key}"))
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => continue,
            };

            if !list_resp.status().is_success() {
                continue;
            }

            let list_text = list_resp.text().await.unwrap_or_default();
            let entries: serde_json::Value =
                serde_json::from_str(&list_text).unwrap_or(serde_json::json!([]));

            let Some(entry_arr) = entries.as_array() else {
                continue;
            };

            for entry in entry_arr {
                let name = entry["name"].as_str().unwrap_or_default();
                let entry_type = entry["type"].as_str().unwrap_or("file");

                if SKIP_DIRS.contains(&name) {
                    continue;
                }

                let full_path = format!("{dir}/{name}");

                if entry_type == "dir" || entry_type == "directory" {
                    dirs_to_visit.push(full_path);
                } else {
                    let file_url = format!(
                        "{E2B_API_BASE}/v0/sandboxes/{sandbox_id}/files?path={encoded}",
                        encoded = urlencoding::encode(&full_path)
                    );

                    if let Ok(file_resp) = self
                        .http
                        .get(&file_url)
                        .header("Authorization", format!("Bearer {api_key}"))
                        .send()
                        .await
                    {
                        if file_resp.status().is_success() {
                            if let Ok(content) = file_resp.text().await {
                                files.insert(full_path, content);
                            }
                        }
                    }
                }
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_has_no_id() {
        let client = E2bSandboxClient::new("");
        assert!(client.current_id().is_none());
    }

    #[test]
    fn set_and_clear_id() {
        let client = E2bSandboxClient::new("");
        client.set_id("sandbox-123".to_string());
        assert_eq!(client.current_id().as_deref(), Some("sandbox-123"));
        client.clear_id();
        assert!(client.current_id().is_none());
    }

    #[test]
    fn require_id_errors_when_none() {
        let client = E2bSandboxClient::new("");
        assert!(client.require_id().is_err());
    }

    #[test]
    fn require_id_returns_id_when_set() {
        let client = E2bSandboxClient::new("");
        client.set_id("sb-abc".to_string());
        assert_eq!(client.require_id().unwrap(), "sb-abc");
    }

    #[tokio::test]
    async fn create_sandbox_reuses_existing_when_no_reset() {
        let client = E2bSandboxClient::new("");
        client.set_id("existing-id".to_string());
        let result = client
            .create_sandbox(false, "code-interpreter-v1", 600_000)
            .await
            .unwrap();
        assert_eq!(result, "existing-id");
    }

    #[tokio::test]
    async fn create_sandbox_fails_without_api_key() {
        std::env::remove_var("E2B_API_KEY");
        let client = E2bSandboxClient::new("");
        let result = client
            .create_sandbox(true, "code-interpreter-v1", 600_000)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("E2B_API_KEY is not set"));
    }
}
