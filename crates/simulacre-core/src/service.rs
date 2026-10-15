//! Transport-independent request handling, shared by the HTTP server and the
//! in-browser (WebAssembly) build so both answer exactly the same way.
//!
//! Nothing here reads the clock or the environment: callers pass "today".

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{EduLevel, data};
use crate::model::{Gender, Recipe, SECTIONS};
use crate::{GenError, Persona, Summary, UiLang, generate, labels, rng};

/// Vault lifecycle, in order.
pub const STATUSES: &[&str] = &["draft", "incubating", "active", "dormant", "burned"];

/// Journal entry kinds an operator may add by hand.
pub const JOURNAL_KINDS: &[&str] = &["note", "action", "contact", "incident"];

/// Upper bound for a journal entry, in bytes.
pub const JOURNAL_MAX: usize = 4000;

/// The catalogue the UI is built from: countries, archetypes, platforms,
/// labels, limits.
pub fn meta() -> Value {
    let d = data();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "locales": d.locales.iter().map(|l| json!({
            "code": l.code, "language": l.language, "country": l.country, "country_code": l.country_code,
            "fiction_phone": l.phone.fiction_reserved,
        })).collect::<Vec<_>>(),
        "archetypes": d.common.archetypes.iter().map(|a| json!({
            "key": a.key, "label": a.label, "desc": a.desc, "age": a.age,
        })).collect::<Vec<_>>(),
        "platforms": d.common.platforms.iter().map(|p| json!({ "key": p.key, "name": p.name, "kind": p.kind })).collect::<Vec<_>>(),
        "sectors": d.common.sectors.iter().map(|s| json!({ "key": s.key, "label": s.label })).collect::<Vec<_>>(),
        "hobbies": d.common.hobbies.iter().map(|h| json!({ "key": h.key, "label": h.l10n(), "cat": h.cat })).collect::<Vec<_>>(),
        "edu_levels": EduLevel::ALL.iter().map(|e| e.key()).collect::<Vec<_>>(),
        "sections": SECTIONS,
        "statuses": STATUSES,
        "labels": labels::all(),
        "limits": { "min_age": crate::generate::MIN_AGE, "max_age": crate::generate::MAX_AGE },
    })
}

/// A recipe as the UI sends it: everything optional.
#[derive(Debug, Deserialize, Default)]
pub struct RecipeIn {
    pub seed: Option<String>,
    pub locale: Option<String>,
    pub archetype: Option<String>,
    pub gender: Option<Gender>,
    pub age_min: Option<u8>,
    pub age_max: Option<u8>,
    pub as_of: Option<NaiveDate>,
    #[serde(default)]
    pub rerolls: BTreeMap<String, u32>,
}

impl RecipeIn {
    /// Validates the request; a missing seed gets a fresh random one.
    pub fn into_recipe(self, today: NaiveDate) -> Result<Recipe, String> {
        let seed = self.seed.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(rng::random_seed);
        if seed.len() > 64 {
            return Err("seed too long (max 64)".into());
        }
        if let Some(bad) = self.rerolls.keys().find(|k| !SECTIONS.contains(&k.as_str())) {
            return Err(format!("unknown section '{bad}'"));
        }
        let mut r = Recipe::new(seed, self.as_of.unwrap_or(today));
        if let Some(l) = self.locale.filter(|l| !l.is_empty()) {
            r.locale = l;
        }
        r.archetype = self.archetype.filter(|a| !a.is_empty());
        r.gender = self.gender;
        r.age_min = self.age_min;
        r.age_max = self.age_max;
        r.rerolls = self.rerolls;
        Ok(r)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct PoolQuery {
    pub pool: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub locale: Option<String>,
    pub archetype: Option<String>,
    pub gender: Option<Gender>,
}

#[derive(Serialize)]
struct PoolItem {
    #[serde(flatten)]
    summary: Summary,
    locale: String,
}

/// The "vivier": an endless, stable stream of candidates. Page N of pool P
/// always shows the same people.
pub fn pool(q: PoolQuery, today: NaiveDate) -> Result<Value, GenError> {
    let pool = q.pool.unwrap_or_else(|| "default".into());
    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(24).clamp(1, 60);
    let locales: Vec<String> = match q.locale.filter(|l| !l.is_empty() && l != "any") {
        Some(l) => vec![l],
        None => data().locales.iter().map(|l| l.code.clone()).collect(),
    };
    let mut items = Vec::with_capacity(limit as usize);
    for i in offset..offset + limit {
        let mut r = Recipe::new(rng::pool_seed(&pool, i), today);
        r.locale = locales[(i as usize) % locales.len()].clone();
        r.archetype = q.archetype.clone().filter(|a| !a.is_empty());
        r.gender = q.gender;
        let p = generate(&r)?;
        items.push(PoolItem { summary: Summary::from(&p), locale: r.locale });
    }
    Ok(json!({ "items": items, "next": offset + limit }))
}

/// UI language from a query value; French when missing or unknown.
pub fn lang(q: Option<&str>) -> UiLang {
    q.and_then(|l| l.parse().ok()).unwrap_or_default()
}

/// One line of an operations journal, as exported.
pub struct JournalLine<'a> {
    pub at: &'a str,
    pub kind: &'a str,
    pub text: &'a str,
}

/// The printable legend file, followed by its journal (oldest first).
/// `journal` comes newest first, as the vault lists it.
pub fn markdown_with_journal(p: &Persona, lang: UiLang, journal: &[JournalLine]) -> String {
    let mut md = crate::export::markdown(p, lang);
    if !journal.is_empty() {
        md.push_str("\n## Journal\n");
        for j in journal.iter().rev() {
            md.push_str(&format!("- `{}` **{}** {}\n", j.at, j.kind, j.text));
        }
    }
    md
}

/// Checks an operator's journal entry; returns the cleaned text and kind.
pub fn journal_entry(kind: Option<&str>, text: &str) -> Result<(String, String), String> {
    let text = text.trim();
    if text.is_empty() || text.len() > JOURNAL_MAX {
        return Err("journal text must be 1–4000 characters".into());
    }
    let kind = kind.unwrap_or("note");
    if !JOURNAL_KINDS.contains(&kind) {
        return Err("kind must be note, action, contact or incident".into());
    }
    Ok((kind.to_string(), text.to_string()))
}

/// Journal text recorded when a saved legend is edited.
pub fn edit_note(before: u8, after: u8) -> String {
    if before != after { format!("coherence {before} → {after}") } else { "edited".into() }
}

/// Face ids are the first 128 bits of the SHA-256 of the normalised JPEG, in
/// lowercase hex. Doubles as a safe file name.
pub fn is_valid_face_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()
    }

    #[test]
    fn recipe_validation() {
        let r = RecipeIn { seed: Some("  SIM-X ".into()), ..Default::default() }.into_recipe(day()).unwrap();
        assert_eq!(r.seed, "SIM-X");
        assert_eq!(r.as_of, day());
        assert!(RecipeIn { seed: Some("x".repeat(65)), ..Default::default() }.into_recipe(day()).is_err());
        let mut bad = RecipeIn::default();
        bad.rerolls.insert("nope".into(), 1);
        assert!(bad.into_recipe(day()).is_err());
        assert!(RecipeIn::default().into_recipe(day()).unwrap().seed.starts_with("SIM-"));
    }

    #[test]
    fn pool_is_stable() {
        let q = || PoolQuery { pool: Some("p".into()), limit: Some(3), ..Default::default() };
        assert_eq!(pool(q(), day()).unwrap(), pool(q(), day()).unwrap());
        assert_eq!(pool(q(), day()).unwrap()["items"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn journal_rules() {
        assert_eq!(journal_entry(None, "  hi ").unwrap(), ("note".into(), "hi".into()));
        assert!(journal_entry(Some("system"), "x").is_err());
        assert!(journal_entry(None, "   ").is_err());
        assert_eq!(edit_note(80, 80), "edited");
        assert_eq!(edit_note(80, 90), "coherence 80 → 90");
    }

    #[test]
    fn face_ids() {
        assert!(is_valid_face_id(&"a1".repeat(16)));
        assert!(!is_valid_face_id(&"A1".repeat(16)));
        assert!(!is_valid_face_id("../etc/passwd"));
    }
}
