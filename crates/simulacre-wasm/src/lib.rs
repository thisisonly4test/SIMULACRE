//! The SIMULACRE engine compiled to WebAssembly, for the static build of the
//! web UI (no server). Every function takes and returns JSON strings: the
//! JavaScript side (`web/js/local.js`) owns storage, this side owns logic.
//!
//! The answers are the same as the HTTP API's, because both call
//! `simulacre_core::service`.

use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;
use simulacre_core::service::{self, JournalLine, PoolQuery, RecipeIn};
use simulacre_core::{Persona, Summary, contamination, generate, quiz, refresh, rng};
use wasm_bindgen::prelude::*;

type Res = Result<String, JsError>;

fn parse<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, JsError> {
    serde_json::from_str(s).map_err(|e| JsError::new(&format!("bad request: {e}")))
}

fn out(v: &impl serde::Serialize) -> Res {
    serde_json::to_string(v).map_err(|e| JsError::new(&e.to_string()))
}

fn day(today: &str) -> Result<NaiveDate, JsError> {
    today.parse().map_err(|_| JsError::new("bad date"))
}

#[wasm_bindgen]
pub fn meta() -> Res {
    out(&service::meta())
}

#[wasm_bindgen]
pub fn seed() -> String {
    rng::random_seed()
}

#[wasm_bindgen]
pub fn forge(recipe: &str, today: &str) -> Res {
    let r = parse::<RecipeIn>(recipe)?.into_recipe(day(today)?).map_err(|e| JsError::new(&e))?;
    out(&generate(&r).map_err(|e| JsError::new(&e.to_string()))?)
}

#[wasm_bindgen]
pub fn pool(query: &str, today: &str) -> Res {
    out(&service::pool(parse::<PoolQuery>(query)?, day(today)?).map_err(|e| JsError::new(&e.to_string()))?)
}

/// Re-runs story and coherence on an edited persona.
#[wasm_bindgen]
pub fn check(persona: &str) -> Res {
    let mut p: Persona = parse(persona)?;
    refresh(&mut p);
    out(&p)
}

#[wasm_bindgen]
pub fn quiz(persona: &str, round: u32) -> Res {
    out(&quiz::quiz(&parse(persona)?, round))
}

#[wasm_bindgen]
pub fn summary(persona: &str) -> Res {
    out(&Summary::from(&parse::<Persona>(persona)?))
}

#[derive(Deserialize)]
struct Entry {
    at: String,
    kind: String,
    text: String,
}

/// Legend file in Markdown; `journal` is the vault's journal, newest first.
#[wasm_bindgen]
pub fn export_md(persona: &str, lang: &str, journal: &str) -> Res {
    let p: Persona = parse(persona)?;
    let entries: Vec<Entry> = parse(journal)?;
    let lines: Vec<_> = entries.iter().map(|e| JournalLine { at: &e.at, kind: &e.kind, text: &e.text }).collect();
    Ok(service::markdown_with_journal(&p, service::lang(Some(lang)), &lines))
}

/// Pairwise contamination between saved legends: `[[id, persona], …]`.
#[wasm_bindgen]
pub fn contamination(pairs: &str) -> Res {
    let pairs: Vec<(String, Persona)> = parse(pairs)?;
    out(&contamination::matrix(&pairs))
}

/// Validates an operator's journal entry; returns `{kind, text}`.
#[wasm_bindgen]
pub fn journal_entry(kind: Option<String>, text: &str) -> Res {
    let (kind, text) = service::journal_entry(kind.as_deref(), text).map_err(|e| JsError::new(&e))?;
    out(&json!({ "kind": kind, "text": text }))
}

#[wasm_bindgen]
pub fn edit_note(before: u8, after: u8) -> String {
    service::edit_note(before, after)
}

#[wasm_bindgen]
pub fn is_valid_face_id(id: &str) -> bool {
    service::is_valid_face_id(id)
}

/// Statuses, journal kinds and limits the vault enforces.
#[wasm_bindgen]
pub fn rules() -> Res {
    out(&json!({ "statuses": service::STATUSES, "journal_max": service::JOURNAL_MAX }))
}

/// Checks a vault backup (or a single-legend export) before it is restored;
/// returns it normalised (personas refreshed, faces verified).
#[wasm_bindgen]
pub fn backup_parse(json: &str) -> Res {
    out(&simulacre_core::backup::parse(json).map_err(|e| JsError::new(&e))?)
}

/// `true` when a legend already in the vault should be replaced by the copy
/// from a backup (strictly newer).
#[wasm_bindgen]
pub fn backup_should_replace(existing_updated_at: &str, incoming_updated_at: &str) -> bool {
    simulacre_core::backup::should_replace(existing_updated_at, incoming_updated_at)
}

/// Format tag and version written into new backups.
#[wasm_bindgen]
pub fn backup_header() -> Res {
    out(&json!({ "format": simulacre_core::backup::FORMAT, "version": simulacre_core::backup::VERSION }))
}
