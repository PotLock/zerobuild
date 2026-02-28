//! GitHub OAuth flow handlers: `/auth/github` and `/auth/github/callback`.
//!
//! These routes allow the user to connect their GitHub account by going through
//! the standard OAuth 2.0 authorization code flow. The resulting token is
//! stored in the local SQLite database.

use super::AppState;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

/// Query parameters returned by GitHub's OAuth redirect.
#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// GET /auth/github — redirect the user to GitHub's OAuth authorization page.
pub async fn handle_github_auth(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.lock().zerobuild.clone();

    if cfg.github_client_id.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub OAuth is not configured. Set github_client_id and github_client_secret in config.".to_string(),
        )
            .into_response();
    }

    let scope = "repo,read:user,user:email";
    let redirect_uri = ""; // GitHub will use the app's registered callback
    let auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={client_id}&scope={scope}",
        client_id = cfg.github_client_id,
        scope = urlencoding::encode(scope),
    );

    let _ = redirect_uri; // unused — GitHub uses registered callback

    (
        StatusCode::FOUND,
        [(header::LOCATION, auth_url)],
    )
        .into_response()
}

/// GET /auth/github/callback — exchange OAuth code for access token and store it.
pub async fn handle_github_callback(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    if let Some(err) = params.error {
        let desc = params.error_description.as_deref().unwrap_or("unknown error");
        return (
            StatusCode::BAD_REQUEST,
            format!("GitHub OAuth error: {err} — {desc}"),
        )
            .into_response();
    }

    let code = match params.code {
        Some(c) if !c.is_empty() => c,
        _ => {
            return (StatusCode::BAD_REQUEST, "Missing OAuth code.".to_string()).into_response();
        }
    };

    let cfg = state.config.lock().zerobuild.clone();

    if cfg.github_client_id.is_empty() || cfg.github_client_secret.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub OAuth is not configured.".to_string(),
        )
            .into_response();
    }

    // Exchange code for token
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("ZeroBuild/0.1")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("HTTP client error: {e}"),
            )
                .into_response();
        }
    };

    let token_resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", cfg.github_client_id.as_str()),
            ("client_secret", cfg.github_client_secret.as_str()),
            ("code", code.as_str()),
        ])
        .send()
        .await;

    let resp = match token_resp {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Failed to exchange code with GitHub: {e}"),
            )
                .into_response();
        }
    };

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return (
            StatusCode::BAD_GATEWAY,
            format!("GitHub token exchange failed: {err}"),
        )
            .into_response();
    }

    let token_data: serde_json::Value = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Failed to parse GitHub token response: {e}"),
            )
                .into_response();
        }
    };

    let access_token = match token_data["access_token"].as_str() {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => {
            let err = token_data["error"].as_str().unwrap_or("unknown");
            return (
                StatusCode::BAD_GATEWAY,
                format!("GitHub returned no access token: {err}"),
            )
                .into_response();
        }
    };

    // Fetch the authenticated user's login name
    let username = fetch_github_username(&client, &access_token).await;

    // Store token in SQLite
    let db_path = std::path::PathBuf::from(&cfg.db_path);
    match crate::store::init_db(&db_path) {
        Ok(conn) => {
            if let Err(e) = crate::store::tokens::save_github_token(
                &conn,
                &access_token,
                username.as_deref(),
            ) {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to save token: {e}"),
                )
                    .into_response();
            }
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to open database: {e}"),
            )
                .into_response();
        }
    }

    let display_name = username.as_deref().unwrap_or("unknown user");
    tracing::info!("GitHub OAuth: connected as {display_name}");

    // Return a simple success page
    let html = format!(
        "<!DOCTYPE html><html><body style='font-family:sans-serif;text-align:center;padding:40px'>
        <h2>✅ GitHub Connected!</h2>
        <p>Connected as <strong>{display_name}</strong></p>
        <p>You can close this window and return to Telegram.</p>
        </body></html>"
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html")],
        html,
    )
        .into_response()
}

async fn fetch_github_username(client: &reqwest::Client, token: &str) -> Option<String> {
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;

    let data: serde_json::Value = resp.json().await.ok()?;
    data["login"].as_str().map(|s| s.to_string())
}
