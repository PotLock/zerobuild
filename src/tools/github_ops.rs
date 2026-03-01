//! GitHub connector - operation tools: issue, PR, review, list repos, etc.
//!
//! These tools are part of the GitHub connector. They call the GitHub REST API
//! directly using the OAuth token stored in the local SQLite database.
//! No backend proxy required for these operations.
//!
//! Token is loaded from `config.db_path` on each execute call.

use super::traits::{Tool, ToolResult};
use crate::config::ZerobuildConfig;
use crate::store;
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

const GITHUB_API_BASE: &str = "https://api.github.com";

// ── Issue/PR Format Constants ──────────────────────────────────────────────────

/// Valid issue title prefixes (bracketed format)
const VALID_ISSUE_PREFIXES: &[&str] = &[
    "[Feature]:",
    "[Bug]:",
    "[Chore]:",
    "[Docs]:",
    "[Security]:",
    "[Refactor]:",
    "[Test]:",
    "[Perf]:",
];

/// Valid type labels that must be present on every issue/PR
const VALID_TYPE_LABELS: &[&str] = &[
    "feature", "bug", "chore", "docs", "security", "refactor", "test", "perf",
];

/// Required sections for issue body
const REQUIRED_ISSUE_SECTIONS: &[&str] = &[
    "## Summary",
    "## Problem Statement",
    "## Proposed Solution",
    "## Non-goals / Out of Scope",
    "## Acceptance Criteria",
    "## Architecture Impact",
    "## Risk and Rollback",
    "## Breaking Change",
    "## Data Hygiene Checks",
];

/// Required sections for PR body
const REQUIRED_PR_SECTIONS: &[&str] = &[
    "## Summary",
    "## Problem",
    "## Root Cause",
    "## Changes",
    "## Validation",
    "## Scope",
    "## Risk",
    "## Rollback",
];

/// Validates that an issue title follows the bracketed prefix format
fn validate_issue_title(title: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("Issue title is required".to_string());
    }
    
    let has_valid_prefix = VALID_ISSUE_PREFIXES
        .iter()
        .any(|prefix| title.trim().starts_with(prefix));
    
    if !has_valid_prefix {
        return Err(format!(
            "Issue title must start with a bracketed type prefix. Valid prefixes: {}. \
             Example: '[Feature]: Add user authentication'",
            VALID_ISSUE_PREFIXES.join(", ")
        ));
    }
    
    Ok(())
}

/// Validates that at least one type label is present
fn validate_labels(labels: &[String]) -> Result<(), String> {
    if labels.is_empty() {
        return Err(
            "At least one label is required. Must include a type label: \
             feature, bug, chore, docs, security, refactor, test, or perf".to_string()
        );
    }
    
    let has_type_label = labels
        .iter()
        .any(|label| VALID_TYPE_LABELS.contains(&label.as_str()));
    
    if !has_type_label {
        return Err(format!(
            "Must include at least one type label: {}",
            VALID_TYPE_LABELS.join(", ")
        ));
    }
    
    Ok(())
}

/// Validates that PR title follows conventional commit format
fn validate_pr_title(title: &str) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("PR title is required".to_string());
    }
    
    // Conventional commit pattern: type(scope): description
    let conventional_pattern = regex::Regex::new(
        r"^(feat|fix|chore|docs|style|refactor|perf|test|build|ci|revert)(\([^)]+\))?: .+"
    ).unwrap();
    
    if !conventional_pattern.is_match(title.trim()) {
        return Err(
            "PR title must follow conventional commit format: 'type(scope): description'. \
             Valid types: feat, fix, chore, docs, style, refactor, perf, test, build, ci, revert. \
             Example: 'feat(auth): add OAuth2 token refresh'".to_string()
        );
    }
    
    Ok(())
}

/// Checks if body contains required sections (returns missing sections)
fn check_required_sections(body: &str, required: &[&str]) -> Vec<String> {
    required
        .iter()
        .filter(|section| !body.contains(**section))
        .map(|s| s.to_string())
        .collect()
}

/// Extract a summary from the title by removing the prefix
fn extract_summary_from_title(title: &str) -> String {
    // Remove bracketed prefix like "[Feature]:" or "[Bug]:"
    let re = regex::Regex::new(r"^\[[^\]]+\]:\s*").unwrap();
    re.replace(title, "").to_string()
}

/// Generates a full issue template from a brief summary
fn generate_issue_template(title: &str, summary: &str) -> String {
    format!(r#"## Summary
{}

## Problem Statement
[Describe the current behavior, gap, or pain point. For bugs: include exact reproduction steps and error messages.]

## Proposed Solution
[For features: what the new behavior should look like. For bugs: what correct behavior looks like.]

## Non-goals / Out of Scope
- [Explicitly list what this issue will NOT address.]

## Alternatives Considered
- [Alternatives evaluated and why they were not chosen.]

## Acceptance Criteria
- [ ] [Concrete, testable condition 1]
- [ ] [Concrete, testable condition 2]

## Architecture Impact
- Affected subsystems: [list modules, traits, tools, or channels impacted]
- New dependencies: [none or list]
- Config/schema changes: [yes/no — if yes, describe]

## Risk and Rollback
- Risk: [low / medium / high — and why]
- Rollback: [how to revert if the fix or feature causes a regression]

## Breaking Change?
- [ ] Yes — describe impact and migration path
- [ ] No

## Data Hygiene Checks
- [ ] I removed personal/sensitive data from examples, payloads, and logs.
- [ ] I used neutral, project-focused wording and placeholders.
"#, summary.trim())
}

/// Generates a full PR template from a brief summary
fn generate_pr_template(title: &str, summary: &str) -> String {
    format!(r#"## Summary
{}

## Problem
[What broken/missing behavior or gap does this PR address?]

## Root Cause
[For bug fixes: what was the underlying cause? For features: what need or gap drove this?]

## Changes
- [Concrete change 1 — module / file / behavior]
- [Concrete change 2]

## Validation
- [ ] `cargo fmt --all -- --check` passed
- [ ] `cargo clippy --all-targets -- -D warnings` passed
- [ ] `cargo test` passed
- [ ] Manual test / scenario: [describe]

## Scope
- Affected subsystems: [list]
- Files changed: [count or list key files]

## Risk
- Risk tier: [low / medium / high]
- Blast radius: [which subsystems or users could be affected by a regression]

## Rollback
- Revert strategy: [`git revert <commit>` or specific steps]
- Migration needed on rollback: [yes / no — if yes, describe]
"#, summary.trim())
}

// ── Shared helpers ─────────────────────────────────────────────────────────────

/// Load the GitHub token from the local store.
fn load_token(db_path: &PathBuf) -> Result<crate::store::tokens::GitHubToken, ToolResult> {
    let conn = store::init_db(db_path).map_err(|e| ToolResult {
        success: false,
        output: String::new(),
        error: Some(format!("Failed to open store DB: {e}")),
        error_hint: None,
    })?;

    match store::tokens::load_github_token(&conn) {
        Ok(Some(tok)) => Ok(tok),
        Ok(None) => Err(ToolResult {
            success: false,
            output: String::new(),
            error: Some(
                "GitHub is not connected. Use github_connect to authenticate first.".to_string(),
            ),
            error_hint: None,
        }),
        Err(e) => Err(ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("Failed to load GitHub token: {e}")),
            error_hint: None,
        }),
    }
}

/// Build a pre-configured reqwest client for GitHub API calls.
fn gh_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("ZeroBuild/0.1")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {e}"))
}

/// GET a GitHub API endpoint and return the response body as ToolResult.
async fn github_get(token: &str, url: &str) -> anyhow::Result<ToolResult> {
    let client = gh_client()?;
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GitHub API request failed: {e}"))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .unwrap_or_else(|_| "<unreadable>".to_string());

    if !status.is_success() {
        return Ok(ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("GitHub API returned {status}: {body}")),
            error_hint: None,
        });
    }

    Ok(ToolResult {
        success: true,
        output: body,
        error: None,
        error_hint: None,
    })
}

/// POST to a GitHub API endpoint and return the response body as ToolResult.
async fn github_post_api(
    token: &str,
    url: &str,
    body: serde_json::Value,
) -> anyhow::Result<ToolResult> {
    let client = gh_client()?;
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GitHub API request failed: {e}"))?;

    let status = resp.status();
    let resp_body = resp
        .text()
        .await
        .unwrap_or_else(|_| "<unreadable>".to_string());

    if !status.is_success() {
        return Ok(ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("GitHub API returned {status}: {resp_body}")),
            error_hint: None,
        });
    }

    Ok(ToolResult {
        success: true,
        output: resp_body,
        error: None,
        error_hint: None,
    })
}

/// PATCH a GitHub API endpoint and return the response body as ToolResult.
async fn github_patch_api(
    token: &str,
    url: &str,
    body: serde_json::Value,
) -> anyhow::Result<ToolResult> {
    let client = gh_client()?;
    let resp = client
        .patch(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("GitHub API request failed: {e}"))?;

    let status = resp.status();
    let resp_body = resp
        .text()
        .await
        .unwrap_or_else(|_| "<unreadable>".to_string());

    if !status.is_success() {
        return Ok(ToolResult {
            success: false,
            output: String::new(),
            error: Some(format!("GitHub API returned {status}: {resp_body}")),
            error_hint: None,
        });
    }

    Ok(ToolResult {
        success: true,
        output: resp_body,
        error: None,
        error_hint: None,
    })
}

/// Resolve the `owner` field: use provided value or fall back to stored username.
fn resolve_owner(
    args: &serde_json::Value,
    stored_username: Option<&str>,
) -> Result<String, ToolResult> {
    if let Some(o) = args["owner"].as_str().filter(|s| !s.is_empty()) {
        return Ok(o.to_string());
    }
    stored_username
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| ToolResult {
            success: false,
            output: String::new(),
            error: Some(
                "owner is required (or will be inferred from the authenticated user). \
                 Try reconnecting GitHub via github_connect."
                    .to_string(),
            ),
            error_hint: None,
        })
}

// ── Helper: Extract hashtags from text ────────────────────────────────────────

fn extract_hashtags(text: &str) -> Vec<String> {
    let mut hashtags = Vec::new();
    for word in text.split_whitespace() {
        if word.starts_with('#') && word.len() > 1 {
            let tag = word[1..]
                .trim_matches(|c: char| c.is_ascii_punctuation())
                .to_lowercase();
            if !tag.is_empty() && !hashtags.contains(&tag) {
                hashtags.push(tag);
            }
        }
    }
    hashtags
}

// ── github_create_issue ────────────────────────────────────────────────────────

pub struct GitHubCreateIssueTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubCreateIssueTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubCreateIssueTool {
    fn name(&self) -> &str {
        "github_create_issue"
    }

    fn description(&self) -> &str {
        "CREATE A GITHUB ISSUE - Use this when user says '#issue', '#bug', 'create issue', or wants to report a bug/request a feature. \
         \
         TRIGGER PHRASES: '#issue', '#bug', '#feature', 'create issue', 'file issue', 'report bug'. \
         \
         REQUIRED FORMAT (ENFORCED): \
         - Title MUST start with bracketed prefix: [Feature]:, [Bug]:, [Chore]:, [Docs]:, [Security]:, [Refactor]:, [Test]:, or [Perf]: \
         - At least one type label is REQUIRED (feature, bug, chore, docs, security, refactor, test, perf) \
         - Body should follow the standard template with sections: Summary, Problem Statement, Proposed Solution, etc. \
         \
         DO NOT use this for file searches or reading code - use file_read or glob_search instead. \
         All content MUST be in English. \
         The user must have connected their GitHub account first."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": {
                    "type": "string",
                    "description": "Repository name (e.g. my-app)"
                },
                "owner": {
                    "type": "string",
                    "description": "Repository owner (GitHub username or org). Defaults to the authenticated user."
                },
                "title": {
                    "type": "string",
                    "description": "Issue title. MUST use format: [Feature]: ..., [Bug]: ..., [Chore]: ..., [Docs]: ..., [Security]: ..., [Refactor]: ..., [Test]: ..., [Perf]: ..."
                },
                "body": {
                    "type": "string",
                    "description": "Issue body (Markdown). Should include: ## Summary, ## Problem Statement, ## Proposed Solution, ## Non-goals / Out of Scope, ## Acceptance Criteria, ## Architecture Impact, ## Risk and Rollback, ## Breaking Change, ## Data Hygiene Checks"
                },
                "labels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "REQUIRED: At least one type label. Valid: feature, bug, chore, docs, security, refactor, test, perf"
                }
            },
            "required": ["repo", "title", "labels"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let title = args["title"].as_str().unwrap_or("").trim().to_string();
        
        // Validate title format
        if let Err(e) = validate_issue_title(&title) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
                error_hint: Some(format!(
                    "Valid prefixes: {}. Example: '[Feature]: Add dark mode toggle'",
                    VALID_ISSUE_PREFIXES.join(", ")
                )),
            });
        }
        
        if repo.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo is required".to_string()),
                error_hint: None,
            });
        }

        // Extract and validate labels
        let labels: Vec<String> = args["labels"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        
        if let Err(e) = validate_labels(&labels) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
                error_hint: Some(format!(
                    "Required type labels: {}. You may also add scope labels like: provider, channel, tool, gateway, memory, runtime, config, ci",
                    VALID_TYPE_LABELS.join(", ")
                )),
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };

        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues");
        
        // Get body - auto-generate template if missing or insufficient
        let body_content = args["body"].as_str().unwrap_or("").trim().to_string();
        let final_body = if body_content.is_empty() || check_required_sections(&body_content, REQUIRED_ISSUE_SECTIONS).len() > 5 {
            // Auto-generate template from title/summary
            let summary = extract_summary_from_title(&title);
            generate_issue_template(&title, &summary)
        } else {
            body_content
        };
        
        let mut body = json!({ "title": title, "body": final_body });
        if !labels.is_empty() {
            body["labels"] = json!(labels);
        }

        let result = github_post_api(&tok.token, &url, body).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let issue_url = parsed["html_url"].as_str().unwrap_or("");
        let issue_num = parsed["number"].as_u64().unwrap_or(0);

        Ok(ToolResult {
            success: true,
            output: format!("Issue #{issue_num} created: {issue_url}"),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_create_pr ──────────────────────────────────────────────────────────

pub struct GitHubCreatePRTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubCreatePRTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubCreatePRTool {
    fn name(&self) -> &str {
        "github_create_pr"
    }

    fn description(&self) -> &str {
        "CREATE A GITHUB PULL REQUEST - Use this when user says '#pr', '#pullrequest', 'create PR', or wants to submit code for review. \
         \
         TRIGGER PHRASES: '#pr', '#pullrequest', 'create PR', 'open PR', 'submit PR', 'make pull request'. \
         \
         REQUIRED FORMAT (ENFORCED): \
         - Title MUST follow conventional commit format: 'type(scope): description' \
           Valid types: feat, fix, chore, docs, style, refactor, perf, test, build, ci, revert \
           Example: 'feat(auth): add OAuth2 token refresh' \
         - At least one type label is REQUIRED (feature, bug, chore, docs, security, refactor, test, perf) \
         - Body should follow the standard template with sections: Summary, Problem, Root Cause, Changes, Validation, Scope, Risk, Rollback \
         \
         DO NOT use this for creating issues or general queries. \
         All content MUST be in English. \
         The user must have connected their GitHub account first."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "title": { "type": "string", "description": "Pull request title. MUST use conventional commit format: 'type(scope): description'. Valid types: feat, fix, chore, docs, style, refactor, perf, test, build, ci, revert" },
                "body": { "type": "string", "description": "Pull request description (Markdown). Should include: ## Summary, ## Problem, ## Root Cause, ## Changes, ## Validation, ## Scope, ## Risk, ## Rollback" },
                "head": { "type": "string", "description": "Branch to merge from" },
                "base": { "type": "string", "description": "Branch to merge into. Default: main." },
                "labels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "REQUIRED: At least one type label. Valid: feature, bug, chore, docs, security, refactor, test, perf. Also recommended: size labels (size: XS/S/M/L/XL)"
                }
            },
            "required": ["repo", "title", "head", "labels"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let title = args["title"].as_str().unwrap_or("").trim().to_string();
        let head = args["head"].as_str().unwrap_or("").trim().to_string();

        // Validate PR title format (conventional commits)
        if let Err(e) = validate_pr_title(&title) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
                error_hint: Some(
                    "Use format: 'type(scope): description'. \
                     Valid types: feat, fix, chore, docs, style, refactor, perf, test, build, ci, revert. \
                     Examples: 'feat(auth): add OAuth flow', 'fix(api): resolve null pointer'".to_string()
                ),
            });
        }

        if repo.is_empty() || head.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo and head are required".to_string()),
                error_hint: None,
            });
        }

        // Extract and validate labels
        let labels: Vec<String> = args["labels"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        
        if let Err(e) = validate_labels(&labels) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
                error_hint: Some(format!(
                    "Required type labels: {}. \
                     Also recommended: size labels (size: XS, size: S, size: M, size: L, size: XL)",
                    VALID_TYPE_LABELS.join(", ")
                )),
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        let base = args["base"].as_str().unwrap_or("main").to_string();

        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/pulls");
        let mut body = json!({ "title": title, "head": head, "base": base });
        if let Some(v) = args["body"].as_str() {
            body["body"] = json!(v);
        }

        let result = github_post_api(&tok.token, &url, body).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let pr_url = parsed["html_url"].as_str().unwrap_or("");
        let pr_num = parsed["number"].as_u64().unwrap_or(0);

        // Apply labels
        if !labels.is_empty() {
            let labels_url =
                format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues/{pr_num}/labels");
            let _ = github_post_api(&tok.token, &labels_url, json!({ "labels": labels })).await;
        }

        Ok(ToolResult {
            success: true,
            output: format!("Pull request #{pr_num} created: {pr_url}"),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_review_pr ──────────────────────────────────────────────────────────

pub struct GitHubReviewPRTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubReviewPRTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubReviewPRTool {
    fn name(&self) -> &str {
        "github_review_pr"
    }

    fn description(&self) -> &str {
        "Submit a review on a GitHub pull request. \
         Can approve, request changes, or leave a comment."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "pr_number": { "type": "integer", "description": "Pull request number" },
                "body": { "type": "string", "description": "Review comment body" },
                "event": {
                    "type": "string",
                    "enum": ["APPROVE", "REQUEST_CHANGES", "COMMENT"],
                    "description": "Review action"
                }
            },
            "required": ["repo", "pr_number", "event"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let pr_number = args["pr_number"].as_u64().unwrap_or(0);
        let event = args["event"].as_str().unwrap_or("").trim().to_string();

        if repo.is_empty() || pr_number == 0 || event.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo, pr_number, and event are required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };

        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/pulls/{pr_number}/reviews");
        let mut body = json!({ "event": event });
        if let Some(v) = args["body"].as_str() {
            body["body"] = json!(v);
        }

        let result = github_post_api(&tok.token, &url, body).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let state = parsed["state"].as_str().unwrap_or(&event);
        let review_id = parsed["id"].as_u64().unwrap_or(0);

        Ok(ToolResult {
            success: true,
            output: format!("Review #{review_id} submitted ({state}) on PR #{pr_number}"),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_list_repos ─────────────────────────────────────────────────────────

pub struct GitHubListReposTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubListReposTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubListReposTool {
    fn name(&self) -> &str {
        "github_list_repos"
    }

    fn description(&self) -> &str {
        "List GitHub repositories for the authenticated user."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["all", "owner", "member"],
                    "description": "Filter by type (default: owner)"
                },
                "sort": {
                    "type": "string",
                    "enum": ["created", "updated", "pushed", "full_name"],
                    "description": "Sort field (default: updated)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum repos to return (default: 30, max: 100)"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo_type = args["type"].as_str().unwrap_or("owner");
        let sort = args["sort"].as_str().unwrap_or("updated");
        let limit = args["limit"].as_u64().unwrap_or(30).min(100);

        let url =
            format!("{GITHUB_API_BASE}/user/repos?type={repo_type}&sort={sort}&per_page={limit}");
        let result = github_get(&tok.token, &url).await?;
        if !result.success {
            return Ok(result);
        }

        let repos: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let repo_arr = repos.as_array().map(|a| a.as_slice()).unwrap_or_default();

        if repo_arr.is_empty() {
            return Ok(ToolResult {
                success: true,
                output: "No repositories found.".to_string(),
                error: None,
                error_hint: None,
            });
        }

        let lines: Vec<String> = repo_arr
            .iter()
            .map(|r| {
                let name = r["full_name"].as_str().unwrap_or("?");
                let desc = r["description"].as_str().unwrap_or("");
                let url = r["html_url"].as_str().unwrap_or("");
                if desc.is_empty() {
                    format!("• {name} — {url}")
                } else {
                    format!("• {name} — {desc} ({url})")
                }
            })
            .collect();

        Ok(ToolResult {
            success: true,
            output: format!("Repositories ({}):\n{}", lines.len(), lines.join("\n")),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_list_issues ────────────────────────────────────────────────────────

pub struct GitHubListIssuesTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubListIssuesTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubListIssuesTool {
    fn name(&self) -> &str {
        "github_list_issues"
    }

    fn description(&self) -> &str {
        "List issues in a GitHub repository."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "state": {
                    "type": "string",
                    "enum": ["open", "closed", "all"],
                    "description": "Filter by state (default: open)"
                },
                "labels": { "type": "string", "description": "Comma-separated labels to filter by" },
                "limit": { "type": "integer", "description": "Maximum issues to return (default: 30)" }
            },
            "required": ["repo"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        if repo.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo is required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        let state = args["state"].as_str().unwrap_or("open");
        let limit = args["limit"].as_u64().unwrap_or(30);
        let mut url =
            format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues?state={state}&per_page={limit}");
        if let Some(l) = args["labels"].as_str() {
            url.push_str(&format!("&labels={}", urlencoding::encode(l)));
        }

        github_get(&tok.token, &url).await
    }
}

// ── github_list_prs ───────────────────────────────────────────────────────────

pub struct GitHubListPRsTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubListPRsTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubListPRsTool {
    fn name(&self) -> &str {
        "github_list_prs"
    }

    fn description(&self) -> &str {
        "List pull requests in a GitHub repository."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "state": {
                    "type": "string",
                    "enum": ["open", "closed", "all"],
                    "description": "Filter by state (default: open)"
                },
                "limit": { "type": "integer", "description": "Maximum PRs to return (default: 30)" }
            },
            "required": ["repo"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        if repo.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo is required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        let state = args["state"].as_str().unwrap_or("open");
        let limit = args["limit"].as_u64().unwrap_or(30);
        let url =
            format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/pulls?state={state}&per_page={limit}");

        github_get(&tok.token, &url).await
    }
}

// ── github_get_issue ──────────────────────────────────────────────────────────

pub struct GitHubGetIssueTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubGetIssueTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubGetIssueTool {
    fn name(&self) -> &str {
        "github_get_issue"
    }

    fn description(&self) -> &str {
        "Get detailed information about a specific GitHub issue."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "issue_number": { "type": "integer", "description": "Issue number" }
            },
            "required": ["repo", "issue_number"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let issue_number = args["issue_number"].as_u64().unwrap_or(0);
        if repo.is_empty() || issue_number == 0 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo and issue_number are required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues/{issue_number}");
        github_get(&tok.token, &url).await
    }
}

// ── github_get_pr ─────────────────────────────────────────────────────────────

pub struct GitHubGetPRTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubGetPRTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubGetPRTool {
    fn name(&self) -> &str {
        "github_get_pr"
    }

    fn description(&self) -> &str {
        "Get detailed information about a specific GitHub pull request."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "pr_number": { "type": "integer", "description": "Pull request number" }
            },
            "required": ["repo", "pr_number"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let pr_number = args["pr_number"].as_u64().unwrap_or(0);
        if repo.is_empty() || pr_number == 0 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo and pr_number are required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/pulls/{pr_number}");
        github_get(&tok.token, &url).await
    }
}

// ── github_connect ─────────────────────────────────────────────────────────────

pub struct GitHubConnectTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubConnectTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubConnectTool {
    fn name(&self) -> &str {
        "github_connect"
    }

    fn description(&self) -> &str {
        "CHECK GITHUB CONNECTION STATUS - Use this FIRST before any GitHub operation (issue, PR, etc.) to verify user is authenticated. \
         \
         TRIGGER: 'github connect', 'check github', 'am i connected', or before any github_create_issue/github_create_pr call. \
         \
         If not connected, returns OAuth URL for user to authenticate. \
         ALWAYS use this when the user asks about GitHub connection or authentication."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let conn = match store::init_db(&db_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to open store DB: {e}")),
                    error_hint: None,
                })
            }
        };

        match store::tokens::load_github_token(&conn) {
            Ok(Some(tok)) => {
                let username = tok.username.as_deref().unwrap_or("(unknown)");
                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "GitHub is connected (user: {username}). \
                         You can now create issues, PRs, and manage repositories."
                    ),
                    error: None,
                    error_hint: None,
                })
            }
            Ok(None) => {
                let auth_url = "/auth/github";
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "GitHub is not connected.\n\
                         To connect your GitHub account, visit: {auth_url}\n\n\
                         After authenticating, tell me and I will retry."
                    )),
                    error_hint: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to check GitHub token: {e}")),
                error_hint: None,
            }),
        }
    }
}

// ── github_review_pr_with_checklist ──────────────────────────────────────────

pub struct GitHubReviewPRWithChecklistTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubReviewPRWithChecklistTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubReviewPRWithChecklistTool {
    fn name(&self) -> &str {
        "github_review_pr_with_checklist"
    }

    fn description(&self) -> &str {
        "Submit a review on a GitHub pull request using hashtag checklist format. \
         Example: '#security PASS - no auth changes, #tests NEEDS_WORK - add unit tests'"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "pr_number": { "type": "integer", "description": "Pull request number" },
                "checklist": { "type": "string", "description": "Review checklist with hashtags. Example: '#security PASS, #tests NEEDS_WORK'" },
                "event": {
                    "type": "string",
                    "enum": ["APPROVE", "REQUEST_CHANGES", "COMMENT"],
                    "description": "Review action"
                },
                "summary": { "type": "string", "description": "Optional summary before checklist" }
            },
            "required": ["repo", "pr_number", "checklist", "event"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let pr_number = args["pr_number"].as_u64().unwrap_or(0);
        let checklist = args["checklist"].as_str().unwrap_or("").trim().to_string();
        let event = args["event"].as_str().unwrap_or("").trim().to_string();

        if repo.is_empty() || pr_number == 0 || checklist.is_empty() || event.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo, pr_number, checklist, and event are required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };

        // Build review body from summary + formatted checklist
        let mut review_body = String::new();
        if let Some(summary) = args["summary"].as_str() {
            review_body.push_str(summary.trim());
            review_body.push_str("\n\n");
        }
        review_body.push_str("## Review Checklist\n\n");

        for line in checklist.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(hash_pos) = line.find('#') {
                let after_hash = &line[hash_pos..];
                let parts: Vec<&str> = after_hash.split_whitespace().collect();
                if parts.len() >= 2 {
                    let tag = parts[0].trim_matches(|c: char| c.is_ascii_punctuation());
                    let status = parts[1].to_uppercase();
                    let comment = parts[2..].join(" ");
                    let emoji = match status.as_str() {
                        "PASS" | "PASSED" => "✅",
                        "FAIL" | "FAILED" => "❌",
                        "NEEDS_WORK" | "NEEDS-WORK" => "⚠️",
                        "SKIP" | "SKIPPED" => "⏭️",
                        _ => "⏳",
                    };
                    review_body.push_str(&format!("{emoji} **{tag}**"));
                    if !comment.is_empty() {
                        review_body.push_str(&format!(" – {comment}"));
                    }
                    review_body.push('\n');
                } else {
                    review_body.push_str(line);
                    review_body.push('\n');
                }
            } else {
                review_body.push_str(line);
                review_body.push('\n');
            }
        }

        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/pulls/{pr_number}/reviews");
        let body = json!({ "body": review_body, "event": event });
        let result = github_post_api(&tok.token, &url, body).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let state = parsed["state"].as_str().unwrap_or(&event);
        let review_id = parsed["id"].as_u64().unwrap_or(0);

        Ok(ToolResult {
            success: true,
            output: format!(
                "Review #{review_id} submitted ({state}) on PR #{pr_number} with checklist"
            ),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_create_issue_with_hashtags ─────────────────────────────────────────

pub struct GitHubCreateIssueWithHashtagsTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubCreateIssueWithHashtagsTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubCreateIssueWithHashtagsTool {
    fn name(&self) -> &str {
        "github_create_issue_with_hashtags"
    }

    fn description(&self) -> &str {
        "Create a GitHub issue with auto-extracted labels from hashtags. \
         \
         REQUIRED FORMAT (ENFORCED): \
         - Title MUST start with bracketed prefix: [Feature]:, [Bug]:, [Chore]:, [Docs]:, [Security]:, [Refactor]:, [Test]:, or [Perf]: \
         - At least one hashtag type label is REQUIRED in the message (#feature, #bug, #chore, #docs, #security, #refactor, #test, #perf) \
         \
         Example: '#bug [Bug]: Login returns 500 error' creates an issue with 'bug' label. \
         All content MUST be in English."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "message": { "type": "string", "description": "Message with hashtag labels. Must include at least one type hashtag: #feature, #bug, #chore, #docs, #security, #refactor, #test, #perf. Example: '#bug [Bug]: Login not working'" },
                "title": { "type": "string", "description": "Optional explicit title. If provided, MUST use format: [Feature]: ..., [Bug]: ..., etc. Extracted from message if omitted." },
                "body": { "type": "string", "description": "Optional issue body (Markdown). Should follow standard template with required sections." }
            },
            "required": ["repo", "message"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let message = args["message"].as_str().unwrap_or("").trim().to_string();
        if repo.is_empty() || message.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo and message are required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        
        let labels = extract_hashtags(&message);
        
        // Validate that at least one type label is present
        if let Err(e) = validate_labels(&labels) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "{}. Your message must include at least one type hashtag: #feature, #bug, #chore, #docs, #security, #refactor, #test, #perf",
                    e
                )),
                error_hint: Some("Example: '#bug [Bug]: Login returns 500 error' or '#feature [Feature]: Add dark mode'".to_string()),
            });
        }
        
        let title = if let Some(t) = args["title"].as_str().filter(|s| !s.is_empty()) {
            t.to_string()
        } else {
            message
                .split_whitespace()
                .filter(|w| !w.starts_with('#'))
                .collect::<Vec<_>>()
                .join(" ")
        };

        if title.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("Could not extract title. Provide an explicit title.".to_string()),
                error_hint: None,
            });
        }
        
        // Validate title format
        if let Err(e) = validate_issue_title(&title) {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e),
                error_hint: Some(format!(
                    "Extracted title: '{}'. Valid prefixes: {}. \
                     Example message: '#bug [Bug]: Login not working'",
                    title,
                    VALID_ISSUE_PREFIXES.join(", ")
                )),
            });
        }

        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues");
        
        // Get body - auto-generate template if missing or insufficient
        let body_content = args["body"].as_str().unwrap_or("").trim().to_string();
        let final_body = if body_content.is_empty() || check_required_sections(&body_content, REQUIRED_ISSUE_SECTIONS).len() > 5 {
            // Auto-generate template from title
            let summary = extract_summary_from_title(&title);
            generate_issue_template(&title, &summary)
        } else {
            body_content
        };
        
        let mut body = json!({ "title": title, "body": final_body });
        if !labels.is_empty() {
            body["labels"] = json!(labels);
        }

        let result = github_post_api(&tok.token, &url, body).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let issue_url = parsed["html_url"].as_str().unwrap_or("");
        let issue_num = parsed["number"].as_u64().unwrap_or(0);
        let labels_str = labels.join(", ");

        Ok(ToolResult {
            success: true,
            output: format!("Issue #{issue_num} created: {issue_url} (labels: {labels_str})"),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_analyze_pr ─────────────────────────────────────────────────────────

pub struct GitHubAnalyzePRTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubAnalyzePRTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubAnalyzePRTool {
    fn name(&self) -> &str {
        "github_analyze_pr"
    }

    fn description(&self) -> &str {
        "Analyze a GitHub PR and suggest which hashtag review categories are needed."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": { "type": "string", "description": "Repository name" },
                "owner": { "type": "string", "description": "Repository owner. Defaults to authenticated user." },
                "pr_number": { "type": "integer", "description": "Pull request number" }
            },
            "required": ["repo", "pr_number"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let pr_number = args["pr_number"].as_u64().unwrap_or(0);
        if repo.is_empty() || pr_number == 0 {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo and pr_number are required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };
        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/pulls/{pr_number}");
        let result = github_get(&tok.token, &url).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let title = parsed["title"].as_str().unwrap_or("").to_lowercase();
        let body_text = parsed["body"].as_str().unwrap_or("").to_lowercase();
        let combined = format!("{title} {body_text}");

        let mut suggestions = Vec::new();
        if combined.contains("security") || combined.contains("auth") {
            suggestions.push(("#security", "PR contains security-related changes"));
        }
        if combined.contains("test") || combined.contains("spec") {
            suggestions.push(("#tests", "PR may need test coverage verification"));
        }
        if combined.contains("performance") || combined.contains("cache") {
            suggestions.push(("#performance", "PR contains performance-related changes"));
        }
        if combined.contains("ui") || combined.contains("css") || combined.contains("style") {
            suggestions.push(("#ui", "PR contains UI/visual changes"));
        }
        if combined.contains("doc") || combined.contains("readme") {
            suggestions.push(("#docs", "PR contains documentation changes"));
        }
        if combined.contains("api") || combined.contains("endpoint") {
            suggestions.push(("#api", "PR contains API changes"));
        }
        if combined.contains("database") || combined.contains("sql") {
            suggestions.push(("#database", "PR contains database changes"));
        }
        if combined.contains("bug") || combined.contains("fix") {
            suggestions.push(("#bug", "PR is a bug fix"));
        }
        if combined.contains("feature") || combined.contains("add") {
            suggestions.push(("#feature", "PR is a new feature"));
        }
        suggestions.push(("#code", "General code review needed"));

        let output = format!(
            "PR #{pr_number} Analysis — Suggested Review Checklist:\n\n{}",
            suggestions
                .iter()
                .map(|(tag, reason)| format!("{tag} – {reason}"))
                .collect::<Vec<_>>()
                .join("\n")
        );

        Ok(ToolResult {
            success: true,
            output,
            error: None,
            error_hint: None,
        })
    }
}

// ── github_upload_image ───────────────────────────────────────────────────────

pub struct GitHubUploadImageTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubUploadImageTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubUploadImageTool {
    fn name(&self) -> &str {
        "github_upload_image"
    }

    fn description(&self) -> &str {
        "Upload an image to Imgur (anonymous) for use in GitHub issues/PRs. \
         Returns a public URL that can be embedded in Markdown."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image_data": { "type": "string", "description": "Base64-encoded image data" },
                "filename": { "type": "string", "description": "Original filename (e.g., screenshot.png)" },
                "title": { "type": "string", "description": "Optional title for the image" }
            },
            "required": ["image_data", "filename"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let image_data = args["image_data"].as_str().unwrap_or("").trim();
        let filename = args["filename"].as_str().unwrap_or("").trim();

        if image_data.is_empty() || filename.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("image_data and filename are required".to_string()),
                error_hint: None,
            });
        }

        // Upload to Imgur (anonymous, no API key required)
        let client = gh_client()?;
        let body = json!({
            "image": image_data,
            "type": "base64",
            "title": args["title"].as_str().unwrap_or(filename),
        });

        let resp = client
            .post("https://api.imgur.com/3/image")
            .header("Authorization", "Client-ID 546c25a59c58ad7")
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Imgur upload failed: {e}"))?;

        let status = resp.status();
        let resp_body = resp
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable>".to_string());

        if !status.is_success() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Imgur API returned {status}: {resp_body}")),
                error_hint: None,
            });
        }

        let parsed: serde_json::Value = serde_json::from_str(&resp_body).unwrap_or_default();
        let link = parsed["data"]["link"].as_str().unwrap_or("");
        let markdown = format!("![{filename}]({link})");

        Ok(ToolResult {
            success: true,
            output: format!("Image uploaded: {link}\nMarkdown: {markdown}"),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_edit_issue ──────────────────────────────────────────────────────────

pub struct GitHubEditIssueTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubEditIssueTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubEditIssueTool {
    fn name(&self) -> &str {
        "github_edit_issue"
    }

    fn description(&self) -> &str {
        "Edit an existing GitHub issue: update its title, body, labels, or state. \
         Use this to correct issues after creation rather than closing and recreating them. \
         All issue content must be written in English."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": {
                    "type": "string",
                    "description": "Repository name (e.g. my-app)"
                },
                "owner": {
                    "type": "string",
                    "description": "Repository owner (GitHub username or org). Defaults to the authenticated user."
                },
                "issue_number": {
                    "type": "integer",
                    "description": "Issue number to edit"
                },
                "title": {
                    "type": "string",
                    "description": "New issue title. Must use format: [Feature]: ..., [Bug]: ..., [Chore]: ..., [Docs]: ..., etc."
                },
                "body": {
                    "type": "string",
                    "description": "New issue body (Markdown). Must follow the standard issue template."
                },
                "labels": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Replacement label list to apply to the issue"
                },
                "state": {
                    "type": "string",
                    "enum": ["open", "closed"],
                    "description": "New state for the issue: open or closed"
                }
            },
            "required": ["repo", "issue_number"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let issue_number = match args["issue_number"].as_u64() {
            Some(n) => n,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("issue_number is required".to_string()),
                    error_hint: None,
                })
            }
        };
        if repo.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo is required".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };

        let url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues/{issue_number}");
        let mut patch = json!({});
        if let Some(v) = args["title"].as_str() {
            patch["title"] = json!(v);
        }
        if let Some(v) = args["body"].as_str() {
            patch["body"] = json!(v);
        }
        if let Some(v) = args["labels"].as_array() {
            patch["labels"] = json!(v);
        }
        if let Some(v) = args["state"].as_str() {
            patch["state"] = json!(v);
        }

        let result = github_patch_api(&tok.token, &url, patch).await?;
        if !result.success {
            return Ok(result);
        }

        let parsed: serde_json::Value = serde_json::from_str(&result.output).unwrap_or_default();
        let issue_url = parsed["html_url"].as_str().unwrap_or("");

        Ok(ToolResult {
            success: true,
            output: format!("Issue #{issue_number} updated: {issue_url}"),
            error: None,
            error_hint: None,
        })
    }
}

// ── github_close_issue ─────────────────────────────────────────────────────────

pub struct GitHubCloseIssueTool {
    config: Arc<ZerobuildConfig>,
}

impl GitHubCloseIssueTool {
    pub fn new(config: Arc<ZerobuildConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GitHubCloseIssueTool {
    fn name(&self) -> &str {
        "github_close_issue"
    }

    fn description(&self) -> &str {
        "Close a GitHub issue and post a resolution comment explaining the outcome \
         (fixed, won't fix, duplicate, etc.). \
         All comments must be written in English."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "repo": {
                    "type": "string",
                    "description": "Repository name (e.g. my-app)"
                },
                "owner": {
                    "type": "string",
                    "description": "Repository owner (GitHub username or org). Defaults to the authenticated user."
                },
                "issue_number": {
                    "type": "integer",
                    "description": "Issue number to close"
                },
                "reason": {
                    "type": "string",
                    "enum": ["completed", "not_planned"],
                    "description": "Reason for closing: 'completed' (fixed/done) or 'not_planned' (won't fix / out of scope)"
                },
                "comment": {
                    "type": "string",
                    "description": "Resolution comment to post before closing. Required. Explain outcome clearly in English (e.g. 'Fixed in #42', 'Duplicate of #10', 'Out of scope — see discussion')."
                }
            },
            "required": ["repo", "issue_number", "comment"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let db_path = PathBuf::from(&self.config.db_path);
        let tok = match load_token(&db_path) {
            Ok(t) => t,
            Err(e) => return Ok(e),
        };

        let repo = args["repo"].as_str().unwrap_or("").trim().to_string();
        let issue_number = match args["issue_number"].as_u64() {
            Some(n) => n,
            None => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("issue_number is required".to_string()),
                    error_hint: None,
                })
            }
        };
        let comment = args["comment"].as_str().unwrap_or("").trim().to_string();
        if repo.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("repo is required".to_string()),
                error_hint: None,
            });
        }
        if comment.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("comment is required — explain the resolution in English".to_string()),
                error_hint: None,
            });
        }

        let owner = match resolve_owner(&args, tok.username.as_deref()) {
            Ok(o) => o,
            Err(e) => return Ok(e),
        };

        // 1. Post the resolution comment first.
        let comment_url =
            format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues/{issue_number}/comments");
        let comment_result =
            github_post_api(&tok.token, &comment_url, json!({ "body": comment })).await?;
        if !comment_result.success {
            return Ok(comment_result);
        }

        // 2. Close the issue via PATCH.
        let issue_url = format!("{GITHUB_API_BASE}/repos/{owner}/{repo}/issues/{issue_number}");
        let mut patch = json!({ "state": "closed" });
        if let Some(reason) = args["reason"].as_str() {
            patch["state_reason"] = json!(reason);
        }
        let close_result = github_patch_api(&tok.token, &issue_url, patch).await?;
        if !close_result.success {
            return Ok(close_result);
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&close_result.output).unwrap_or_default();
        let html_url = parsed["html_url"].as_str().unwrap_or("");

        Ok(ToolResult {
            success: true,
            output: format!("Issue #{issue_number} closed: {html_url}"),
            error: None,
            error_hint: None,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config(tmp: &TempDir) -> Arc<ZerobuildConfig> {
        Arc::new(ZerobuildConfig {
            db_path: tmp.path().join("test.db").to_string_lossy().to_string(),
            ..ZerobuildConfig::default()
        })
    }

    #[tokio::test]
    async fn connect_returns_not_connected_without_token() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        // Init DB so the connect check works
        let db_path = PathBuf::from(&config.db_path);
        store::init_db(&db_path).unwrap();
        let tool = GitHubConnectTool::new(config);
        let result = tool.execute(json!({})).await.unwrap();
        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not connected"));
    }

    #[tokio::test]
    async fn connect_returns_connected_with_token() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let db_path = PathBuf::from(&config.db_path);
        let conn = store::init_db(&db_path).unwrap();
        store::tokens::save_github_token(&conn, "test-token", Some("zerobuild_user")).unwrap();
        let tool = GitHubConnectTool::new(config);
        let result = tool.execute(json!({})).await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("zerobuild_user"));
    }

    #[tokio::test]
    async fn create_issue_returns_error_without_token() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        store::init_db(&PathBuf::from(&config.db_path)).unwrap();
        let tool = GitHubCreateIssueTool::new(config);
        let result = tool
            .execute(json!({"repo": "test", "title": "Test"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result
            .error
            .as_deref()
            .unwrap_or("")
            .contains("not connected"));
    }

    #[test]
    fn extract_hashtags_works() {
        let tags = extract_hashtags("#bug Login not working #feature");
        assert!(tags.contains(&"bug".to_string()));
        assert!(tags.contains(&"feature".to_string()));
    }
}
