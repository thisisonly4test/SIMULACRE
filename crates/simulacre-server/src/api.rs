//! HTTP handlers. JSON in, JSON out; every error is `{ "error": "…" }`.

use std::collections::BTreeMap;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use simulacre_core::backup::{self, Backup, BackupEntry, BackupFace, BackupRecord, ImportReport};
use simulacre_core::model::FaceRef;
use simulacre_core::service::{self, PoolQuery, RecipeIn};
use simulacre_core::{Persona, Summary, UiLang, contamination, export, generate, quiz, refresh, rng};

use crate::AppState;
use crate::faces::{self, Face};
use crate::store::{Record, STATUSES, StoreError};

pub struct ApiError(StatusCode, String);

impl ApiError {
    pub fn bad(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound => Self(StatusCode::NOT_FOUND, "not found".into()),
            StoreError::BadStatus(_) => Self(StatusCode::BAD_REQUEST, e.to_string()),
            other => {
                tracing::error!("store: {other}");
                Self(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into())
            }
        }
    }
}

impl From<simulacre_core::GenError> for ApiError {
    fn from(e: simulacre_core::GenError) -> Self {
        Self::bad(e.to_string())
    }
}

impl From<faces::FaceError> for ApiError {
    fn from(e: faces::FaceError) -> Self {
        match e {
            faces::FaceError::Fetch(m) => Self(StatusCode::BAD_GATEWAY, format!("face source: {m}")),
            faces::FaceError::Decode(m) => Self::bad(format!("image: {m}")),
            faces::FaceError::Io(e) => {
                tracing::error!("faces io: {e}");
                Self(StatusCode::INTERNAL_SERVER_ERROR, "storage error".into())
            }
        }
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

// ── Meta ─────────────────────────────────────────────────────────────────────

pub async fn meta() -> Json<Value> {
    Json(service::meta())
}

pub async fn new_seed() -> Json<Value> {
    Json(json!({ "seed": rng::random_seed() }))
}

// ── Forge ────────────────────────────────────────────────────────────────────

pub async fn forge(Json(body): Json<RecipeIn>) -> ApiResult<Persona> {
    Ok(Json(generate(&body.into_recipe(today()).map_err(ApiError::bad)?)?))
}

/// The "vivier": an endless, stable stream of candidates.
pub async fn pool(Query(q): Query<PoolQuery>) -> ApiResult<Value> {
    Ok(Json(service::pool(q, today())?))
}

#[derive(Deserialize)]
pub struct LangQuery {
    lang: Option<String>,
    round: Option<u32>,
}

fn lang(q: &Option<String>) -> UiLang {
    service::lang(q.as_deref())
}

/// Re-runs the coherence engine on an edited persona.
pub async fn check(Json(mut p): Json<Persona>) -> ApiResult<Persona> {
    refresh(&mut p);
    Ok(Json(p))
}

#[derive(Deserialize)]
pub struct QuizIn {
    persona: Persona,
    #[serde(default)]
    round: u32,
}

pub async fn quiz_unsaved(Json(b): Json<QuizIn>) -> ApiResult<Vec<quiz::Question>> {
    Ok(Json(quiz::quiz(&b.persona, b.round)))
}

#[derive(Deserialize)]
pub struct ExportIn {
    persona: Persona,
    lang: Option<String>,
}

fn markdown_response(md: String, name: &str) -> Response {
    let safe: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    (
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"simulacre-{safe}.md\"")),
        ],
        md,
    )
        .into_response()
}

pub async fn export_unsaved(Json(b): Json<ExportIn>) -> Response {
    let md = export::markdown(&b.persona, lang(&b.lang));
    markdown_response(md, &b.persona.recipe.seed)
}

// ── Vault ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct VaultItem {
    id: String,
    status: String,
    created_at: String,
    updated_at: String,
    face: Option<FaceRef>,
    #[serde(flatten)]
    summary: Summary,
}

impl From<&Record> for VaultItem {
    fn from(r: &Record) -> Self {
        Self {
            id: r.id.clone(),
            status: r.status.clone(),
            created_at: r.created_at.clone(),
            updated_at: r.updated_at.clone(),
            face: r.persona.face.clone(),
            summary: Summary::from(&r.persona),
        }
    }
}

pub async fn vault_list(State(s): State<AppState>) -> ApiResult<Vec<VaultItem>> {
    Ok(Json(s.store.list()?.iter().map(VaultItem::from).collect()))
}

fn validate_face(s: &AppState, p: &Persona) -> Result<(), ApiError> {
    if let Some(f) = &p.face
        && (!faces::is_valid_id(&f.id) || !s.store.face_exists(&f.id)?)
    {
        return Err(ApiError::bad("unknown face id"));
    }
    Ok(())
}

pub async fn vault_create(State(s): State<AppState>, Json(mut p): Json<Persona>) -> Result<(StatusCode, Json<Record>), ApiError> {
    validate_face(&s, &p)?;
    refresh(&mut p);
    Ok((StatusCode::CREATED, Json(s.store.insert(&p)?)))
}

pub async fn vault_get(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Record> {
    Ok(Json(s.store.get(&id)?))
}

pub async fn vault_update(State(s): State<AppState>, Path(id): Path<String>, Json(mut p): Json<Persona>) -> ApiResult<Record> {
    validate_face(&s, &p)?;
    refresh(&mut p);
    let before = s.store.get(&id)?;
    let rec = s.store.update(&id, &p)?;
    s.store.journal_add(&id, "edit", &service::edit_note(before.persona.coherence.score, rec.persona.coherence.score))?;
    Ok(Json(rec))
}

#[derive(Deserialize)]
pub struct StatusIn {
    status: String,
}

pub async fn vault_status(State(s): State<AppState>, Path(id): Path<String>, Json(b): Json<StatusIn>) -> ApiResult<Record> {
    Ok(Json(s.store.set_status(&id, &b.status)?))
}

pub async fn vault_delete(State(s): State<AppState>, Path(id): Path<String>) -> Result<StatusCode, ApiError> {
    s.store.delete(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct JournalIn {
    kind: Option<String>,
    text: String,
}

pub async fn journal_list(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Vec<crate::store::JournalEntry>> {
    s.store.get(&id)?;
    Ok(Json(s.store.journal(&id)?))
}

pub async fn journal_add(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<JournalIn>,
) -> Result<(StatusCode, Json<crate::store::JournalEntry>), ApiError> {
    s.store.get(&id)?;
    let (kind, text) = service::journal_entry(b.kind.as_deref(), &b.text).map_err(ApiError::bad)?;
    Ok((StatusCode::CREATED, Json(s.store.journal_add(&id, &kind, &text)?)))
}

pub async fn vault_quiz(State(s): State<AppState>, Path(id): Path<String>, Query(q): Query<LangQuery>) -> ApiResult<Vec<quiz::Question>> {
    Ok(Json(quiz::quiz(&s.store.get(&id)?.persona, q.round.unwrap_or(0))))
}

pub async fn vault_export_md(State(s): State<AppState>, Path(id): Path<String>, Query(q): Query<LangQuery>) -> Result<Response, ApiError> {
    let rec = s.store.get(&id)?;
    let journal = s.store.journal(&id)?;
    let lines: Vec<_> = journal.iter().map(|j| service::JournalLine { at: &j.at, kind: &j.kind, text: &j.text }).collect();
    let md = service::markdown_with_journal(&rec.persona, lang(&q.lang), &lines);
    Ok(markdown_response(md, &rec.persona.recipe.seed))
}

pub async fn vault_export_json(State(s): State<AppState>, Path(id): Path<String>) -> Result<Response, ApiError> {
    let rec = s.store.get(&id)?;
    let body = serde_json::to_string_pretty(&json!({ "record": rec, "journal": s.store.journal(&id)? }))
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"simulacre-{}.json\"", rec.id)),
        ],
        body,
    )
        .into_response())
}

/// The whole vault in one file: every legend, its journal, and the faces.
pub async fn vault_backup(State(s): State<AppState>) -> Result<Response, ApiError> {
    let mut records = Vec::new();
    for r in s.store.list()? {
        let mut journal: Vec<BackupEntry> =
            s.store.journal(&r.id)?.into_iter().map(|j| BackupEntry { at: j.at, kind: j.kind, text: j.text }).collect();
        journal.reverse();
        records.push(BackupRecord {
            id: r.id,
            status: r.status,
            created_at: r.created_at,
            updated_at: r.updated_at,
            persona: r.persona,
            journal,
        });
    }
    let mut faces = Vec::new();
    for (id, created_at, source) in s.store.face_list()? {
        match tokio::fs::read(faces::path_for(&s.faces_dir, &id)).await {
            Ok(bytes) => faces.push(BackupFace::new(&id, &source, &created_at, &bytes)),
            Err(e) => tracing::warn!("backup: face {id} unreadable: {e}"),
        }
    }
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let b = Backup { format: backup::FORMAT.into(), version: backup::VERSION, exported_at: now.clone(), records, faces };
    let body = serde_json::to_string(&b).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"simulacre-vault-{}.json\"", &now[..10])),
        ],
        body,
    )
        .into_response())
}

/// Restores a backup (or a single-legend export) into the vault.
pub async fn vault_restore(State(s): State<AppState>, body: Bytes) -> ApiResult<ImportReport> {
    let text = std::str::from_utf8(&body).map_err(|_| ApiError::bad("backup must be UTF-8 JSON"))?;
    let mut b = backup::parse(text).map_err(ApiError::bad)?;
    let mut report = ImportReport::default();
    for f in &b.faces {
        if !s.store.face_exists(&f.id)? {
            faces::save(&s.faces_dir, &f.id, &f.bytes()).await?;
            s.store.face_add(&f.id, &f.source)?;
            report.faces += 1;
        }
    }
    for r in &mut b.records {
        if let Some(f) = &r.persona.face {
            if !s.store.face_exists(&f.id)? {
                r.persona.face = None;
                refresh(&mut r.persona);
                report.faces_missing += 1;
            }
        }
        match s.store.import(r)? {
            Some(true) => report.added += 1,
            Some(false) => report.updated += 1,
            None => report.skipped += 1,
        }
    }
    Ok(Json(report))
}

pub async fn contamination_report(State(s): State<AppState>) -> ApiResult<Value> {
    let recs = s.store.list()?;
    let pairs: Vec<(String, Persona)> = recs.iter().filter(|r| r.status != "burned").map(|r| (r.id.clone(), r.persona.clone())).collect();
    let names: BTreeMap<String, Value> = recs
        .iter()
        .map(|r| (r.id.clone(), json!({ "codename": r.persona.codename, "name": r.persona.identity.full_name(), "status": r.status, "face": r.persona.face })))
        .collect();
    Ok(Json(json!({ "pairs": contamination::matrix(&pairs), "names": names })))
}

pub async fn stats(State(s): State<AppState>) -> ApiResult<Value> {
    let recs = s.store.list()?;
    let mut by_status: BTreeMap<&str, usize> = STATUSES.iter().map(|s| (*s, 0)).collect();
    for r in &recs {
        if let Some(c) = by_status.get_mut(r.status.as_str()) {
            *c += 1;
        }
    }
    let avg = if recs.is_empty() { 0.0 } else { recs.iter().map(|r| r.persona.coherence.score as f64).sum::<f64>() / recs.len() as f64 };
    Ok(Json(json!({ "total": recs.len(), "by_status": by_status, "avg_coherence": avg.round() })))
}

// ── Faces ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct FetchQuery {
    count: Option<usize>,
}

async fn store_face(s: &AppState, raw: &[u8], source: &str) -> Result<Face, ApiError> {
    let (id, jpg) = tokio::task::spawn_blocking({
        let raw = raw.to_vec();
        move || faces::normalize(&raw)
    })
    .await
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    faces::save(&s.faces_dir, &id, &jpg).await?;
    s.store.face_add(&id, source)?;
    Ok(Face { url: format!("/faces/{id}.jpg"), id, source: source.into() })
}

/// Contact sheet: N distinct faces so the operator can pick one that matches
/// the legend's apparent age and gender. The upstream generator tends to hand
/// the same picture to simultaneous requests, so faces are fetched in small,
/// staggered waves and duplicates are retried (bounded).
pub async fn faces_fetch(State(s): State<AppState>, Query(q): Query<FetchQuery>) -> ApiResult<Value> {
    let n = q.count.unwrap_or(6).clamp(1, 9);
    let mut seen = std::collections::HashSet::new();
    let mut faces: Vec<Face> = Vec::new();
    let mut attempts = 0;
    let mut last_error = String::new();
    while faces.len() < n && attempts < n * 3 {
        let wave = (n - faces.len()).min(3);
        attempts += wave;
        let tasks = (0..wave).map(|i| {
            let s = s.clone();
            async move {
                tokio::time::sleep(std::time::Duration::from_millis(s.face_stagger_ms * i as u64)).await;
                let _permit = s.face_permits.acquire().await.map_err(|e| e.to_string())?;
                let raw = faces::fetch(&s.http, &s.face_source).await.map_err(|e| e.to_string())?;
                store_face(&s, &raw, "thispersondoesnotexist").await.map_err(|e| e.1)
            }
        });
        let mut got_any = false;
        for r in futures::future::join_all(tasks).await {
            match r {
                Ok(f) => {
                    got_any = true;
                    if seen.insert(f.id.clone()) {
                        faces.push(f);
                    }
                }
                Err(e) => {
                    tracing::warn!("face fetch: {e}");
                    last_error = e;
                }
            }
        }
        if !got_any {
            break;
        }
        if faces.len() < n {
            tokio::time::sleep(std::time::Duration::from_millis(s.face_stagger_ms * 2)).await;
        }
    }
    if faces.is_empty() {
        return Err(ApiError(StatusCode::BAD_GATEWAY, format!("face source unreachable ({last_error}) — upload a picture instead")));
    }
    Ok(Json(json!({ "faces": faces, "requested": n })))
}

pub async fn faces_upload(State(s): State<AppState>, body: Bytes) -> Result<(StatusCode, Json<Face>), ApiError> {
    if body.is_empty() {
        return Err(ApiError::bad("empty body"));
    }
    if !faces::sniff_ok(&body) {
        return Err(ApiError::bad("only JPEG, PNG or WebP"));
    }
    Ok((StatusCode::CREATED, Json(store_face(&s, &body, "upload").await?)))
}

pub async fn face_users(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Vec<String>> {
    if !faces::is_valid_id(&id) {
        return Err(ApiError::bad("bad face id"));
    }
    Ok(Json(s.store.face_users(&id)?))
}

pub async fn face_file(State(s): State<AppState>, Path(file): Path<String>) -> Result<Response, ApiError> {
    let id = file.strip_suffix(".jpg").unwrap_or(&file);
    if !faces::is_valid_id(id) {
        return Err(ApiError(StatusCode::NOT_FOUND, "not found".into()));
    }
    let bytes =
        tokio::fs::read(faces::path_for(&s.faces_dir, id)).await.map_err(|_| ApiError(StatusCode::NOT_FOUND, "not found".into()))?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=31536000, immutable")], bytes).into_response())
}
