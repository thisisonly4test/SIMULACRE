//! Whole-vault backups: every saved legend with its journal, plus the face
//! pictures they use, in one JSON file. Shared by the HTTP server and the
//! in-browser build, so a backup taken in one restores into the other.
//!
//! An import is never trusted: the format and every field are checked, each
//! persona goes back through the coherence engine, and each face must be a
//! JPEG whose SHA-256 matches its id.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::service::{JOURNAL_KINDS, JOURNAL_MAX, STATUSES, is_valid_face_id};
use crate::{Persona, refresh};

pub const FORMAT: &str = "simulacre-vault";
pub const VERSION: u32 = 1;
/// Upper bounds, so a hostile file cannot exhaust memory or storage.
pub const MAX_RECORDS: usize = 5000;
pub const MAX_FACE_BYTES: usize = 8 * 1024 * 1024;

/// Journal kinds the vault writes itself, on top of the operator's own.
const SYSTEM_KINDS: &[&str] = &["system", "status", "edit"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub exported_at: String,
    pub records: Vec<BackupRecord>,
    #[serde(default)]
    pub faces: Vec<BackupFace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub persona: Persona,
    /// Oldest first.
    #[serde(default)]
    pub journal: Vec<BackupEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupEntry {
    pub at: String,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFace {
    pub id: String,
    pub source: String,
    pub created_at: String,
    /// The normalised JPEG, base64.
    pub data: String,
}

impl BackupFace {
    pub fn new(id: &str, source: &str, created_at: &str, jpeg: &[u8]) -> Self {
        Self { id: id.into(), source: source.into(), created_at: created_at.into(), data: B64.encode(jpeg) }
    }

    /// The picture's bytes. Only valid after [`parse`] has checked them.
    pub fn bytes(&self) -> Vec<u8> {
        B64.decode(&self.data).unwrap_or_default()
    }
}

/// What an import did, as reported to the operator.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    pub added: usize,
    pub updated: usize,
    /// Already in the vault, and not older there.
    pub skipped: usize,
    pub faces: usize,
    /// Face references dropped because the picture was in neither the file
    /// nor the vault.
    pub faces_missing: usize,
}

/// An existing legend is replaced only by a strictly newer copy.
pub fn should_replace(existing_updated_at: &str, incoming_updated_at: &str) -> bool {
    match (DateTime::parse_from_rfc3339(existing_updated_at), DateTime::parse_from_rfc3339(incoming_updated_at)) {
        (Ok(a), Ok(b)) => b > a,
        _ => false,
    }
}

fn check_time(what: &str, t: &str) -> Result<(), String> {
    DateTime::parse_from_rfc3339(t).map(|_| ()).map_err(|_| format!("{what}: bad timestamp '{t}'"))
}

fn is_valid_record_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A single-legend export (`{ "record": …, "journal": […] }`, journal newest
/// first) is accepted too, as a one-record backup without faces.
fn from_single(json: &str) -> Result<Backup, String> {
    #[derive(Deserialize)]
    struct Single {
        record: SingleRecord,
        #[serde(default)]
        journal: Vec<BackupEntry>,
    }
    #[derive(Deserialize)]
    struct SingleRecord {
        id: String,
        status: String,
        created_at: String,
        updated_at: String,
        persona: Persona,
    }
    let s: Single = serde_json::from_str(json).map_err(|e| format!("not a SIMULACRE export: {e}"))?;
    let r = s.record;
    let mut journal = s.journal;
    journal.reverse();
    Ok(Backup {
        format: FORMAT.into(),
        version: VERSION,
        exported_at: r.updated_at.clone(),
        records: vec![BackupRecord {
            id: r.id,
            status: r.status,
            created_at: r.created_at,
            updated_at: r.updated_at,
            persona: r.persona,
            journal,
        }],
        faces: Vec::new(),
    })
}

/// Reads and checks a backup file. Personas come back refreshed by the
/// coherence engine.
pub fn parse(json: &str) -> Result<Backup, String> {
    // Parsing straight from the text (never through a `serde_json::Value`)
    // keeps one copy of the persona decoder in the WebAssembly build.
    #[derive(Deserialize)]
    struct Probe {
        record: Option<serde::de::IgnoredAny>,
    }
    let probe: Probe = serde_json::from_str(json).map_err(|e| format!("not a SIMULACRE backup: {e}"))?;
    let mut b = if probe.record.is_some() {
        from_single(json)?
    } else {
        let b: Backup = serde_json::from_str(json).map_err(|e| format!("not a SIMULACRE backup: {e}"))?;
        if b.format != FORMAT {
            return Err(format!("not a SIMULACRE backup (format '{}')", b.format));
        }
        if b.version == 0 || b.version > VERSION {
            return Err(format!("backup version {} is not supported (max {VERSION})", b.version));
        }
        b
    };
    if b.records.len() > MAX_RECORDS {
        return Err(format!("too many legends (max {MAX_RECORDS})"));
    }
    let mut ids = std::collections::HashSet::new();
    for r in &mut b.records {
        if !is_valid_record_id(&r.id) {
            return Err(format!("bad legend id '{}'", r.id));
        }
        if !ids.insert(r.id.clone()) {
            return Err(format!("legend '{}' appears twice", r.id));
        }
        if !STATUSES.contains(&r.status.as_str()) {
            return Err(format!("legend '{}': invalid status '{}'", r.id, r.status));
        }
        check_time(&r.id, &r.created_at)?;
        check_time(&r.id, &r.updated_at)?;
        for j in &r.journal {
            check_time(&r.id, &j.at)?;
            if !JOURNAL_KINDS.contains(&j.kind.as_str()) && !SYSTEM_KINDS.contains(&j.kind.as_str()) {
                return Err(format!("legend '{}': unknown journal kind '{}'", r.id, j.kind));
            }
            if j.text.len() > JOURNAL_MAX {
                return Err(format!("legend '{}': journal entry too long", r.id));
            }
        }
        if let Some(f) = &r.persona.face {
            if !is_valid_face_id(&f.id) {
                return Err(format!("legend '{}': bad face id", r.id));
            }
        }
        refresh(&mut r.persona);
    }
    let mut face_ids = std::collections::HashSet::new();
    for f in &b.faces {
        if !is_valid_face_id(&f.id) || !face_ids.insert(f.id.clone()) {
            return Err(format!("bad or repeated face id '{}'", f.id));
        }
        if f.source.len() > 64 {
            return Err(format!("face {}: source too long", f.id));
        }
        check_time(&f.id, &f.created_at)?;
        if f.data.len() > MAX_FACE_BYTES / 3 * 4 + 4 {
            return Err(format!("face {}: too large", f.id));
        }
        let bytes = B64.decode(&f.data).map_err(|_| format!("face {}: bad base64", f.id))?;
        if !bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            return Err(format!("face {}: not a JPEG", f.id));
        }
        if hex(&Sha256::digest(&bytes)[..16]) != f.id {
            return Err(format!("face {}: picture does not match its id", f.id));
        }
    }
    Ok(b)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FaceRef;
    use crate::{Recipe, generate};
    use chrono::NaiveDate;

    fn persona(seed: &str) -> Persona {
        generate(&Recipe::new(seed, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())).unwrap()
    }

    fn jpeg() -> (String, Vec<u8>) {
        let bytes = vec![0xff, 0xd8, 0xff, 0xe0, 1, 2, 3, 4];
        (hex(&Sha256::digest(&bytes)[..16]), bytes)
    }

    fn backup() -> Backup {
        let (fid, bytes) = jpeg();
        let mut p = persona("SIM-BK1");
        p.face = Some(FaceRef { id: fid.clone(), source: "upload".into() });
        Backup {
            format: FORMAT.into(),
            version: VERSION,
            exported_at: "2026-05-01T10:00:00Z".into(),
            records: vec![BackupRecord {
                id: "abc123".into(),
                status: "active".into(),
                created_at: "2026-04-01T10:00:00Z".into(),
                updated_at: "2026-04-02T10:00:00Z".into(),
                persona: p,
                journal: vec![BackupEntry { at: "2026-04-01T10:00:00Z".into(), kind: "system".into(), text: "created".into() }],
            }],
            faces: vec![BackupFace::new(&fid, "upload", "2026-04-01T10:00:00Z", &bytes)],
        }
    }

    #[test]
    fn round_trip() {
        let b = parse(&serde_json::to_string(&backup()).unwrap()).unwrap();
        assert_eq!(b.records.len(), 1);
        assert_eq!(b.faces[0].bytes(), jpeg().1);
    }

    #[test]
    fn rejects_tampering() {
        let bad = |f: fn(&mut Backup)| {
            let mut b = backup();
            f(&mut b);
            parse(&serde_json::to_string(&b).unwrap()).unwrap_err()
        };
        assert!(bad(|b| b.format = "other".into()).contains("not a SIMULACRE"));
        assert!(bad(|b| b.version = 99).contains("not supported"));
        assert!(bad(|b| b.records[0].status = "hacked".into()).contains("invalid status"));
        assert!(bad(|b| b.records[0].id = "../x".into()).contains("bad legend id"));
        assert!(bad(|b| b.records.push(b.records[0].clone())).contains("twice"));
        assert!(bad(|b| b.records[0].journal[0].kind = "root".into()).contains("journal kind"));
        assert!(bad(|b| b.faces[0].data = B64.encode([0xff, 0xd8, 0xff, 9])).contains("does not match"));
        assert!(bad(|b| b.faces[0].data = B64.encode(b"GIF89a")).contains("not a JPEG"));
        assert!(parse("[]").is_err());
    }

    #[test]
    fn single_legend_export_is_accepted() {
        let b = backup();
        let r = &b.records[0];
        let single = serde_json::json!({
            "record": { "id": r.id, "status": r.status, "created_at": r.created_at, "updated_at": r.updated_at, "persona": r.persona },
            "journal": [
                { "id": 2, "persona_id": r.id, "at": "2026-04-02T10:00:00Z", "kind": "note", "text": "second" },
                { "id": 1, "persona_id": r.id, "at": "2026-04-01T10:00:00Z", "kind": "system", "text": "created" },
            ],
        });
        let b = parse(&single.to_string()).unwrap();
        assert_eq!(b.records[0].journal[0].text, "created", "oldest first");
        assert!(b.faces.is_empty());
    }

    #[test]
    fn newer_wins() {
        assert!(should_replace("2026-04-01T10:00:00Z", "2026-04-02T10:00:00Z"));
        assert!(!should_replace("2026-04-02T10:00:00Z", "2026-04-02T10:00:00Z"));
        assert!(!should_replace("2026-04-02T10:00:00Z", "garbage"));
    }
}
