//! ZeroBuild: request_deploy tool.
//!
//! Pushes the current project snapshot directly to GitHub using the GitHub
//! REST API (git tree/commit/ref endpoints). No backend proxy required.
//!
//! Requires a GitHub token stored via the OAuth flow (`/auth/github`).

use super::traits::{Tool, ToolResult};
use crate::config::ZerobuildConfig;
use crate::store;
use async_trait::async_trait;
use base64::Engine as _;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const TOOL_NAME: &str = "request_deploy";
const GITHUB_API_BASE: &str = "https://api.github.com";

pub struct RequestDeployTool {
    config: Arc<ZerobuildConfig>,
}

impl RequestDeployTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for RequestDeployTool {
    fn name(&self) -> &str {
        TOOL_NAME
    }

    fn description(&self) -> &str {
        "Deploy the current project to GitHub by pushing the snapshot files. \
         Creates or updates a GitHub repository with the project code. \
         Requires GitHub authentication (use github_connect first). \
         Returns the repository URL."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "project_name": {
                    "type": "string",
                    "description": "Desired GitHub repo name (lowercase, hyphens). Example: my-landing-page"
                },
                "commit_message": {
                    "type": "string",
                    "description": "Git commit message. Default: 'Deploy from ZeroBuild'"
                },
                "private": {
                    "type": "boolean",
                    "description": "Create as private repository. Default: false (public)."
                }
            },
            "required": ["project_name"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);

        // 1. Load GitHub token
        let conn = store::init_db(&db_path)
            .map_err(|e| anyhow::anyhow!("Failed to open store DB: {e}"))?;
        let tok = match store::tokens::load_github_token(&conn) {
            Ok(Some(t)) => t,
            Ok(None) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(
                        "GitHub is not connected. Use github_connect to authenticate first.".to_string(),
                    ),
                })
            }
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to load GitHub token: {e}")),
                })
            }
        };

        // 2. Load snapshot
        let snapshot: (std::collections::HashMap<String, String>, Option<String>) = match store::snapshot::load_snapshot(&conn) {
            Ok(Some(s)) => s,
            Ok(None) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(
                        "No project snapshot found. Build a project first with the E2B tools, \
                         then call e2b_save_snapshot before deploying."
                            .to_string(),
                    ),
                })
            }
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to load snapshot: {e}")),
                })
            }
        };

        let (files, _project_type) = snapshot;
        let project_name = args["project_name"]
            .as_str()
            .unwrap_or("zerobuild-project")
            .trim()
            .to_lowercase()
            .replace(' ', "-");
        let commit_message = args["commit_message"]
            .as_str()
            .unwrap_or("Deploy from ZeroBuild");
        let private = args["private"].as_bool().unwrap_or(false);

        let owner = tok.username.as_deref().unwrap_or("").to_string();
        if owner.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "GitHub username not found. Please reconnect GitHub via github_connect.".to_string(),
                ),
            });
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent("ZeroBuild/0.1")
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))?;

        let token = &tok.token;

        // 3. Ensure repo exists (create if needed)
        let repo_url = format!("{GITHUB_API_BASE}/repos/{owner}/{project_name}");
        let repo_check = client
            .get(&repo_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("GitHub API request failed: {e}"))?;

        if repo_check.status() == reqwest::StatusCode::NOT_FOUND {
            // Create new repo
            let create_url = format!("{GITHUB_API_BASE}/user/repos");
            let create_body = json!({
                "name": project_name,
                "private": private,
                "auto_init": false,
                "description": "Built with ZeroBuild",
            });
            let create_resp = client
                .post(&create_url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Accept", "application/vnd.github+json")
                .json(&create_body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("GitHub API request failed: {e}"))?;

            if !create_resp.status().is_success() {
                let err = create_resp.text().await.unwrap_or_default();
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to create repository: {err}")),
                });
            }
        }

        // 4. Get or create default branch ref
        let base_tree_sha = get_or_create_base_tree(
            &client, token, &owner, &project_name,
        ).await?;

        // 5. Create git blobs for all files
        let mut tree_entries: Vec<serde_json::Value> = Vec::new();
        for (file_path, content) in &files {
            // Strip leading workdir prefix from paths (e.g. /home/user/project/)
            let relative_path = strip_workdir_prefix(file_path);
            if relative_path.is_empty() { continue; }

            let blob_url = format!("{GITHUB_API_BASE}/repos/{owner}/{project_name}/git/blobs");
            let blob_body = json!({
                "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes() as &[u8]),
                "encoding": "base64",
            });
            let blob_resp = client
                .post(&blob_url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Accept", "application/vnd.github+json")
                .json(&blob_body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create blob: {e}"))?;

            if !blob_resp.status().is_success() {
                continue; // Skip files we can't upload
            }

            let blob_data: serde_json::Value = blob_resp.json().await.unwrap_or_default();
            let blob_sha = blob_data["sha"].as_str().unwrap_or("").to_string();
            if blob_sha.is_empty() { continue; }

            tree_entries.push(json!({
                "path": relative_path,
                "mode": "100644",
                "type": "blob",
                "sha": blob_sha,
            }));
        }

        if tree_entries.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("No files to deploy. Snapshot may be empty.".to_string()),
            });
        }

        // 6. Create git tree
        let tree_url = format!("{GITHUB_API_BASE}/repos/{owner}/{project_name}/git/trees");
        let mut tree_body = json!({ "tree": tree_entries });
        if let Some(ref sha) = base_tree_sha {
            tree_body["base_tree"] = json!(sha);
        }

        let tree_resp = client
            .post(&tree_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .json(&tree_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create tree: {e}"))?;

        if !tree_resp.status().is_success() {
            let err = tree_resp.text().await.unwrap_or_default();
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to create git tree: {err}")),
            });
        }

        let tree_data: serde_json::Value = tree_resp.json().await.unwrap_or_default();
        let tree_sha = tree_data["sha"].as_str().unwrap_or("").to_string();

        // 7. Create commit
        let commit_url = format!("{GITHUB_API_BASE}/repos/{owner}/{project_name}/git/commits");
        let mut commit_body = json!({
            "message": commit_message,
            "tree": tree_sha,
        });
        if base_tree_sha.is_some() {
            // Get parent commit SHA
            if let Ok(parent_sha) = get_latest_commit_sha(&client, token, &owner, &project_name).await {
                commit_body["parents"] = json!([parent_sha]);
            }
        }

        let commit_resp = client
            .post(&commit_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .json(&commit_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create commit: {e}"))?;

        if !commit_resp.status().is_success() {
            let err = commit_resp.text().await.unwrap_or_default();
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to create commit: {err}")),
            });
        }

        let commit_data: serde_json::Value = commit_resp.json().await.unwrap_or_default();
        let commit_sha = commit_data["sha"].as_str().unwrap_or("").to_string();

        // 8. Update or create branch ref
        let ref_url = format!("{GITHUB_API_BASE}/repos/{owner}/{project_name}/git/refs/heads/main");
        let ref_body = json!({ "sha": commit_sha, "force": true });

        let ref_resp = client
            .patch(&ref_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/vnd.github+json")
            .json(&ref_body)
            .send()
            .await;

        match ref_resp {
            Ok(r) if r.status().is_success() => {}
            _ => {
                // Ref might not exist yet — create it
                let create_ref_url = format!("{GITHUB_API_BASE}/repos/{owner}/{project_name}/git/refs");
                let create_ref_body = json!({ "ref": "refs/heads/main", "sha": commit_sha });
                let _ = client
                    .post(&create_ref_url)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Accept", "application/vnd.github+json")
                    .json(&create_ref_body)
                    .send()
                    .await;
            }
        }

        let repo_html_url = format!("https://github.com/{owner}/{project_name}");
        let files_count = files.len();

        Ok(ToolResult {
            success: true,
            output: format!(
                "Deployed {files_count} files to GitHub!\n\
                 Repository: {repo_html_url}\n\
                 Commit: {commit_sha}"
            ),
            error: None,
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn get_or_create_base_tree(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> anyhow::Result<Option<String>> {
    let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/git/refs/heads/main");
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let data: serde_json::Value = resp.json().await.unwrap_or_default();
    let sha = data["object"]["sha"].as_str().map(|s| s.to_string());
    Ok(sha)
}

async fn get_latest_commit_sha(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> anyhow::Result<String> {
    let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/git/refs/heads/main");
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await.unwrap_or_default();
    data["object"]["sha"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("Cannot find latest commit SHA"))
}

/// Strip common sandbox working directory prefixes to get a relative path.
fn strip_workdir_prefix(path: &str) -> String {
    let prefixes = ["/home/user/project/", "/home/user/"];
    for prefix in &prefixes {
        if let Some(rel) = path.strip_prefix(prefix) {
            return rel.to_string();
        }
    }
    // If no known prefix, use the path as-is but strip leading /
    path.trim_start_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tool(tmp: &TempDir) -> RequestDeployTool {
        let config = Arc::new(ZerobuildConfig {
            db_path: tmp.path().join("test.db").to_string_lossy().to_string(),
            ..ZerobuildConfig::default()
        });
        RequestDeployTool::new(config)
    }

    #[test]
    fn tool_name() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(make_tool(&tmp).name(), TOOL_NAME);
    }

    #[tokio::test]
    async fn returns_error_without_token() {
        let tmp = TempDir::new().unwrap();
        let tool = make_tool(&tmp);
        store::init_db(&PathBuf::from(&tool.config.db_path)).unwrap();
        let result = tool
            .execute(json!({"project_name": "test-project"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.as_deref().unwrap_or("").contains("not connected"));
    }

    #[test]
    fn strip_workdir_prefix_works() {
        assert_eq!(strip_workdir_prefix("/home/user/project/src/app/page.tsx"), "src/app/page.tsx");
        assert_eq!(strip_workdir_prefix("/home/user/file.txt"), "file.txt");
        assert_eq!(strip_workdir_prefix("/root/other.rs"), "root/other.rs");
    }
}
