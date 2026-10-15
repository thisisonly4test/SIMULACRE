//! HTTP-level tests. The face source is a local mock server, so the suite
//! runs offline.

use std::io::Cursor;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use simulacre_server::{AppState, app, store::Store};
use tower::ServiceExt;

fn png(seed: u8) -> Vec<u8> {
    let img = image::RgbImage::from_fn(200, 200, |x, y| image::Rgb([seed, (x % 255) as u8, (y % 255) as u8]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// A stand-in for thispersondoesnotexist.com: a different image per request.
async fn mock_face_source() -> String {
    use std::sync::atomic::{AtomicU8, Ordering};
    static N: AtomicU8 = AtomicU8::new(0);
    let r = Router::new().route(
        "/face.jpg",
        axum::routing::get(|| async { ([(header::CONTENT_TYPE, "image/png")], png(N.fetch_add(1, Ordering::SeqCst))) }),
    );
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
    format!("http://{addr}/face.jpg")
}

struct T {
    app: Router,
    _dir: tempfile::TempDir,
}

async fn setup(token: Option<&str>, face_url: Option<String>) -> T {
    let dir = tempfile::tempdir().unwrap();
    let url = match face_url {
        Some(u) => u,
        None => mock_face_source().await,
    };
    let mut state = AppState::new(Store::in_memory().unwrap(), dir.path().to_path_buf(), url, token.map(String::from));
    state.face_stagger_ms = 5;
    T { app: app(state), _dir: dir }
}

impl T {
    async fn raw(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        auth: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut b = Request::builder().method(method).uri(uri);
        if let Some(a) = auth {
            b = b.header(header::AUTHORIZATION, format!("Bearer {a}"));
        }
        let req = match body {
            Some(v) => b.header(header::CONTENT_TYPE, "application/json").body(Body::from(v.to_string())).unwrap(),
            None => b.body(Body::empty()).unwrap(),
        };
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        (status, headers, bytes)
    }
    async fn json(&self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (s, _, b) = self.raw(method, uri, body, None).await;
        (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn meta_lists_catalog() {
    let t = setup(None, None).await;
    let (s, v) = t.json(Method::GET, "/api/meta", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["locales"].as_array().unwrap().len(), 4);
    assert!(v["archetypes"].as_array().unwrap().len() >= 10);
    assert!(v["labels"]["status"]["working"]["fr"].is_string());
    assert_eq!(v["limits"]["min_age"], 18);
}

#[tokio::test]
async fn forge_is_deterministic_and_validated() {
    let t = setup(None, None).await;
    let body = json!({ "seed": "SIM-API-1", "locale": "en_GB", "as_of": "2026-05-01" });
    let (s, a) = t.json(Method::POST, "/api/forge", Some(body.clone())).await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = t.json(Method::POST, "/api/forge", Some(body)).await;
    assert_eq!(a, b);
    assert_eq!(a["locale"]["code"], "en_GB");

    let (s, _) = t.json(Method::POST, "/api/forge", Some(json!({}))).await;
    assert_eq!(s, StatusCode::OK, "everything optional");

    let (s, e) = t.json(Method::POST, "/api/forge", Some(json!({ "locale": "zz_ZZ" }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(e["error"].as_str().unwrap().contains("locale"));
    let (s, _) = t.json(Method::POST, "/api/forge", Some(json!({ "age_min": 16 }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "minors refused");
    let (s, _) = t.json(Method::POST, "/api/forge", Some(json!({ "rerolls": { "bogus": 1 } }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pool_is_endless_and_stable() {
    let t = setup(None, None).await;
    let (s, p1) = t.json(Method::GET, "/api/pool?pool=x&offset=0&limit=5", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(p1["items"].as_array().unwrap().len(), 5);
    assert_eq!(p1["next"], 5);
    let (_, again) = t.json(Method::GET, "/api/pool?pool=x&offset=0&limit=5", None).await;
    assert_eq!(p1["items"][0]["seed"], again["items"][0]["seed"]);
    let (_, p2) = t.json(Method::GET, "/api/pool?pool=x&offset=5&limit=5", None).await;
    assert_ne!(p1["items"][0]["seed"], p2["items"][0]["seed"]);
    let (_, fr) = t.json(Method::GET, "/api/pool?pool=x&limit=4&locale=fr_FR&archetype=retiree", None).await;
    for it in fr["items"].as_array().unwrap() {
        assert_eq!(it["locale"], "fr_FR");
        assert_eq!(it["archetype"], "retiree");
    }
}

#[tokio::test]
async fn vault_lifecycle() {
    let t = setup(None, None).await;
    let (_, p) = t.json(Method::POST, "/api/forge", Some(json!({ "seed": "SIM-VAULT", "as_of": "2026-05-01" }))).await;
    let (s, rec) = t.json(Method::POST, "/api/personas", Some(p.clone())).await;
    assert_eq!(s, StatusCode::CREATED);
    let id = rec["id"].as_str().unwrap().to_string();
    assert_eq!(rec["status"], "draft");

    let (_, list) = t.json(Method::GET, "/api/personas", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["status"], "draft", "vault lifecycle status is not shadowed by the occupation");
    assert_eq!(list[0]["occupation"], p["occupation"]["status"]);
    assert_eq!(
        list[0]["name"],
        p["identity"]["first_name"].as_str().unwrap().to_string() + " " + p["identity"]["last_name"].as_str().unwrap()
    );

    let (s, r) = t.json(Method::PUT, &format!("/api/personas/{id}/status"), Some(json!({ "status": "active" }))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["status"], "active");
    let (s, _) = t.json(Method::PUT, &format!("/api/personas/{id}/status"), Some(json!({ "status": "wat" }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // Edit: make the legend contradict itself; the server re-checks.
    let mut edited = r["persona"].clone();
    edited["identity"]["birth_date"] = json!("2015-01-01");
    let (s, r) = t.json(Method::PUT, &format!("/api/personas/{id}"), Some(edited)).await;
    assert_eq!(s, StatusCode::OK);
    assert!(r["persona"]["coherence"]["score"].as_u64().unwrap() < 80);

    let (s, j) = t
        .json(
            Method::POST,
            &format!("/api/personas/{id}/journal"),
            Some(json!({ "kind": "action", "text": "Created the Instagram account" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(j["kind"], "action");
    let (s, _) = t.json(Method::POST, &format!("/api/personas/{id}/journal"), Some(json!({ "kind": "rm -rf", "text": "x" }))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (_, js) = t.json(Method::GET, &format!("/api/personas/{id}/journal"), None).await;
    assert!(js.as_array().unwrap().len() >= 4, "created, status, edit, action");

    let (s, q) = t.json(Method::GET, &format!("/api/personas/{id}/quiz?round=2"), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(q.as_array().unwrap().len() >= 5);

    let (s, h, md) = t.raw(Method::GET, &format!("/api/personas/{id}/export.md?lang=en"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/markdown"));
    let md = String::from_utf8(md).unwrap();
    assert!(md.contains("## Journal") && md.contains("Created the Instagram account"));
    let (s, _, js) = t.raw(Method::GET, &format!("/api/personas/{id}/export.json"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    let v: Value = serde_json::from_slice(&js).unwrap();
    assert_eq!(v["record"]["id"], id);

    let (_, st) = t.json(Method::GET, "/api/stats", None).await;
    assert_eq!(st["total"], 1);
    assert_eq!(st["by_status"]["active"], 1);

    let (s, _, _) = t.raw(Method::DELETE, &format!("/api/personas/{id}"), None, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = t.json(Method::GET, &format!("/api/personas/{id}"), None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn contamination_flags_clones() {
    let t = setup(None, None).await;
    let (_, p) = t.json(Method::POST, "/api/forge", Some(json!({ "seed": "SIM-TWIN" }))).await;
    t.json(Method::POST, "/api/personas", Some(p.clone())).await;
    t.json(Method::POST, "/api/personas", Some(p)).await;
    let (_, other) = t.json(Method::POST, "/api/forge", Some(json!({ "seed": "SIM-OTHER", "locale": "de_DE" }))).await;
    t.json(Method::POST, "/api/personas", Some(other)).await;
    let (s, c) = t.json(Method::GET, "/api/contamination", None).await;
    assert_eq!(s, StatusCode::OK);
    let pairs = c["pairs"].as_array().unwrap();
    assert_eq!(pairs.len(), 3);
    assert_eq!(pairs[0]["risk"], 100);
    assert_eq!(c["names"].as_object().unwrap().len(), 3);
}

#[tokio::test]
async fn check_quiz_export_for_unsaved() {
    let t = setup(None, None).await;
    let (_, mut p) = t.json(Method::POST, "/api/forge", Some(json!({ "seed": "SIM-UNSAVED" }))).await;
    p["identity"]["first_name"] = json!("");
    let (s, r) = t.json(Method::POST, "/api/check", Some(p.clone())).await;
    assert_eq!(s, StatusCode::OK);
    assert!(r["coherence"]["checks"].as_array().unwrap().iter().any(|c| c["id"] == "name"));
    let (s, q) = t.json(Method::POST, "/api/quiz", Some(json!({ "persona": p, "round": 1 }))).await;
    assert_eq!(s, StatusCode::OK);
    assert!(!q.as_array().unwrap().is_empty());
    let (s, _, md) = t.raw(Method::POST, "/api/export", Some(json!({ "persona": r, "lang": "fr" })), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8(md).unwrap().contains("## Chronologie"));
}

#[tokio::test]
async fn faces_contact_sheet_and_assignment() {
    let t = setup(None, None).await;
    let (s, v) = t.json(Method::POST, "/api/faces/fetch?count=3", None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let faces = v["faces"].as_array().unwrap();
    assert_eq!(faces.len(), 3);
    let face = faces[0].clone();
    let (s, h, img) = t.raw(Method::GET, face["url"].as_str().unwrap(), None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h[header::CONTENT_TYPE], "image/jpeg");
    let decoded = image::load_from_memory(&img).unwrap();
    assert_eq!(decoded.width(), 512);

    let (_, mut p) = t.json(Method::POST, "/api/forge", Some(json!({ "seed": "SIM-FACE" }))).await;
    p["face"] = json!({ "id": face["id"], "source": "thispersondoesnotexist" });
    let (s, rec) = t.json(Method::POST, "/api/personas", Some(p.clone())).await;
    assert_eq!(s, StatusCode::CREATED);
    assert!(!rec["persona"]["coherence"]["checks"].as_array().unwrap().iter().any(|c| c["id"] == "face"));
    let (_, users) = t.json(Method::GET, &format!("/api/faces/{}/users", face["id"].as_str().unwrap()), None).await;
    assert_eq!(users.as_array().unwrap().len(), 1);

    p["face"] = json!({ "id": "0123456789abcdef0123456789abcdef", "source": "x" });
    let (s, _) = t.json(Method::POST, "/api/personas", Some(p)).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "unknown face id rejected");

    let (s, _, _) = t.raw(Method::GET, "/faces/..%2F..%2Fetc%2Fpasswd", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn duplicate_faces_are_retried_then_bounded() {
    // A source that always returns the same picture: the endpoint must not
    // loop forever and must not return duplicates.
    let r = Router::new().route("/same.jpg", axum::routing::get(|| async { ([(header::CONTENT_TYPE, "image/png")], png(42)) }));
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
    let t = setup(None, Some(format!("http://{addr}/same.jpg"))).await;
    let (s, v) = t.json(Method::POST, "/api/faces/fetch?count=4", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["faces"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn face_upload_validates_and_strips() {
    let t = setup(None, None).await;
    let req = Request::post("/api/faces/upload").body(Body::from(png(9))).unwrap();
    let res = t.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let req = Request::post("/api/faces/upload").body(Body::from("<?php echo 1; ?>")).unwrap();
    let res = t.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn faces_offline_reports_gateway_error() {
    let t = setup(None, Some("http://127.0.0.1:9/never".into())).await;
    let (s, v) = t.json(Method::POST, "/api/faces/fetch?count=2", None).await;
    assert_eq!(s, StatusCode::BAD_GATEWAY);
    assert!(v["error"].as_str().unwrap().contains("upload"));
}

#[tokio::test]
async fn token_protects_api_but_not_ui() {
    let t = setup(Some("s3cret"), None).await;
    let (s, _, _) = t.raw(Method::GET, "/api/personas", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, _) = t.raw(Method::GET, "/api/personas", None, Some("wrong")).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, _) = t.raw(Method::GET, "/api/personas", None, Some("s3cret")).await;
    assert_eq!(s, StatusCode::OK);
    let req = Request::get("/api/personas").header(header::COOKIE, "a=b; simulacre_token=s3cret").body(Body::empty()).unwrap();
    assert_eq!(t.app.clone().oneshot(req).await.unwrap().status(), StatusCode::OK);
    let (s, _, _) = t.raw(Method::GET, "/api/health", None, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _, _) = t.raw(Method::GET, "/", None, None).await;
    assert_eq!(s, StatusCode::OK, "UI shell loads, then asks for the token");
}

#[tokio::test]
async fn security_headers_and_spa_fallback() {
    let t = setup(None, None).await;
    let (s, h, body) = t.raw(Method::GET, "/", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("default-src 'self'"));
    assert_eq!(h[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(h[header::REFERRER_POLICY], "no-referrer");
    assert!(String::from_utf8(body).unwrap().contains("<title>"));
    let (s, _, _) = t.raw(Method::GET, "/vault", None, None).await;
    assert_eq!(s, StatusCode::OK, "client-side route falls back to index");
    let (s, _, _) = t.raw(Method::GET, "/missing.js", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}
