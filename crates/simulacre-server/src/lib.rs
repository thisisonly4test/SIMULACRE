//! SIMULACRE web platform: router, shared state, embedded UI.

// Nested `if let` reads closer to the checks they encode than let-chains.
#![allow(clippy::collapsible_if)]

pub mod api;
pub mod faces;
pub mod store;

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde_json::json;
use tokio::sync::Semaphore;
use tower_http::set_header::SetResponseHeaderLayer;

#[derive(RustEmbed)]
#[folder = "../../web/"]
#[exclude = "_headers"]
struct Web;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<store::Store>,
    pub faces_dir: PathBuf,
    pub http: reqwest::Client,
    pub face_source: String,
    pub face_permits: Arc<Semaphore>,
    /// Delay between requests inside a wave (the upstream dedupes bursts).
    pub face_stagger_ms: u64,
    /// When set, every `/api` call must carry `Authorization: Bearer <token>`.
    pub token: Option<String>,
}

impl AppState {
    pub fn new(store: store::Store, faces_dir: PathBuf, face_source: String, token: Option<String>) -> Self {
        Self {
            store: Arc::new(store),
            faces_dir,
            http: faces::client(),
            face_source,
            face_permits: Arc::new(Semaphore::new(3)),
            face_stagger_ms: 450,
            token: token.filter(|t| !t.is_empty()),
        }
    }
}

const CSP: &str = "default-src 'self'; img-src 'self' data: blob:; style-src 'self'; script-src 'self'; connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

/// Constant-time comparison so the token can't be guessed byte by byte.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn auth(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let Some(token) = &s.token else { return next.run(req).await };
    let path = req.uri().path();
    let protected = path.starts_with("/api/") || path.starts_with("/faces/");
    if !protected || path == "/api/health" {
        return next.run(req).await;
    }
    let given = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_string)
        .or_else(|| {
            // <img> tags can't send headers: accept the token as a cookie too.
            req.headers()
                .get(header::COOKIE)
                .and_then(|v| v.to_str().ok())
                .and_then(|c| c.split(';').map(str::trim).find_map(|kv| kv.strip_prefix("simulacre_token=")).map(str::to_string))
        });
    match given {
        Some(g) if same(g.as_bytes(), token.as_bytes()) => next.run(req).await,
        _ => (StatusCode::UNAUTHORIZED, Json(json!({ "error": "token required" }))).into_response(),
    }
}

async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Web::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let cache = if path == "index.html" { "no-cache" } else { "public, max-age=300" };
            ([(header::CONTENT_TYPE, mime.as_ref().to_string()), (header::CACHE_CONTROL, cache.to_string())], file.data).into_response()
        }
        None if !path.contains('.') => match Web::get("index.html") {
            Some(f) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string())], f.data).into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn app(state: AppState) -> Router {
    use api::*;
    let api = Router::new()
        .route("/api/health", get(|| async { Json(json!({ "ok": true, "backend": "server" })) }))
        .route("/api/meta", get(meta))
        .route("/api/seed", get(new_seed))
        .route("/api/forge", post(forge))
        .route("/api/pool", get(pool))
        .route("/api/check", post(check))
        .route("/api/quiz", post(quiz_unsaved))
        .route("/api/export", post(export_unsaved))
        .route("/api/stats", get(stats))
        .route("/api/contamination", get(contamination_report))
        .route("/api/personas", get(vault_list).post(vault_create))
        .route("/api/personas/{id}", get(vault_get).put(vault_update).delete(vault_delete))
        .route("/api/personas/{id}/status", put(vault_status))
        .route("/api/personas/{id}/journal", get(journal_list).post(journal_add))
        .route("/api/personas/{id}/quiz", get(vault_quiz))
        .route("/api/personas/{id}/export.md", get(vault_export_md))
        .route("/api/personas/{id}/export.json", get(vault_export_json))
        .route("/api/faces/fetch", post(faces_fetch))
        .route("/api/faces/upload", post(faces_upload))
        .route("/api/faces/{id}/users", get(face_users))
        .route("/faces/{file}", get(face_file));

    api.fallback(static_file)
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .layer(DefaultBodyLimit::max(faces::MAX_UPLOAD + 1024))
        .layer(SetResponseHeaderLayer::overriding(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP)))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")))
        .layer(SetResponseHeaderLayer::overriding(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .with_state(state)
}
