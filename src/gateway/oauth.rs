//! GitHub Connector - OAuth flow handlers: `/auth/github` and `/auth/github/callback`.
//!
//! This is the GitHub connector that allows ZeroBuild to create repos, push code,
//! open issues, and manage PRs on behalf of the user.
//!
//! Supports two modes:
//! 1. User's own OAuth App (advanced) - requires github_client_id and github_client_secret
//! 2. Official ZeroBuild OAuth Proxy (default) - seamless, no setup required
//!
//! The proxy mode allows users to connect GitHub without creating their own OAuth App.
//! The proxy service securely stores the CLIENT_SECRET and handles the OAuth exchange.

use super::AppState;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

/// Query parameters returned by OAuth redirect.
/// Can come from GitHub directly or from the OAuth Proxy.
#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    /// Authorization code (from GitHub direct flow)
    pub code: Option<String>,
    /// Access token (from OAuth Proxy - already exchanged)
    pub token: Option<String>,
    /// GitHub username (from OAuth Proxy)
    pub username: Option<String>,
    /// Error message
    pub error: Option<String>,
    /// Error description
    pub error_description: Option<String>,
}

/// GET /auth/github — redirect to GitHub OAuth or OAuth Proxy.
pub async fn handle_github_auth(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.lock().zerobuild.clone();

    // If user has their own OAuth app configured, use direct GitHub OAuth
    if !cfg.github_client_id.is_empty() {
        let scope = "repo,read:user,user:email";
        let auth_url = format!(
            "https://github.com/login/oauth/authorize?client_id={client_id}&scope={scope}",
            client_id = cfg.github_client_id,
            scope = urlencoding::encode(scope),
        );
        return (StatusCode::FOUND, [(header::LOCATION, auth_url)]).into_response();
    }

    // Otherwise, use the official OAuth Proxy for seamless connection
    let proxy_url = cfg.github_oauth_proxy;
    // Get port from config (default is 3000)
    let port = state.config.lock().gateway.port;
    let callback_url = format!("http://127.0.0.1:{}/auth/github/callback", port);
    let auth_url = format!(
        "{}/start?redirect_uri={}",
        proxy_url,
        urlencoding::encode(&callback_url)
    );

    (StatusCode::FOUND, [(header::LOCATION, auth_url)]).into_response()
}

/// GET /auth/github/callback — handle OAuth callback.
///
/// Two possible flows:
/// 1. Direct GitHub OAuth: receives `code`, exchange for token using client_secret
/// 2. OAuth Proxy: receives `token` directly (proxy already exchanged the code)
pub async fn handle_github_callback(
    State(state): State<AppState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> impl IntoResponse {
    // Handle error from either flow
    if let Some(err) = params.error {
        let desc = params.error_description.as_deref().unwrap_or("unknown error");
        return (
            StatusCode::BAD_REQUEST,
            format!("GitHub OAuth error: {err} — {desc}"),
        )
            .into_response();
    }

    // Check if this is a callback from the OAuth Proxy (token provided directly)
    if let Some(token) = params.token {
        let username = params.username;
        return save_token_and_respond(state, token, username).await;
    }

    // Otherwise, this is a direct GitHub OAuth callback (code exchange required)
    let code = match params.code {
        Some(c) if !c.is_empty() => c,
        _ => {
            return (StatusCode::BAD_REQUEST, "Missing OAuth code or token.").into_response();
        }
    };

    let cfg = state.config.lock().zerobuild.clone();

    // For direct flow, client_id and client_secret are required
    if cfg.github_client_id.is_empty() || cfg.github_client_secret.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "GitHub OAuth is not configured. Please set github_client_id and github_client_secret, \
             or use the default OAuth Proxy by leaving these empty.".to_string(),
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

    save_token_and_respond(state, access_token, username).await
}

/// Save token to database and return success response.
async fn save_token_and_respond(
    state: AppState,
    token: String,
    username: Option<String>,
) -> axum::response::Response {
    let cfg = state.config.lock().zerobuild.clone();
    let db_path = std::path::PathBuf::from(&cfg.db_path);

    match crate::store::init_db(&db_path) {
        Ok(conn) => {
            if let Err(e) = crate::store::tokens::save_github_token(
                &conn,
                &token,
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
    tracing::info!("GitHub OAuth: connected as {}", display_name);

    // Return a simple success page
    let html = format!(
        "<!DOCTYPE html><html><body style='font-family:sans-serif;text-align:center;padding:40px'>
        <h2>✅ GitHub Connected!</h2>
        <p>Connected as <strong>{}</strong></p>
        <p>You can close this window and return to your chat.</p>
        </body></html>",
        display_name
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
