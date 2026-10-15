//! The coherence engine. A legend survives scrutiny only if its facts agree
//! with each other. Every check here mirrors a question an observant person
//! (or a platform's trust team) could ask. It runs on generated personas and,
//! more importantly, again after every manual edit.

use std::collections::HashMap;

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::data::{L10n, data};
use crate::generate::{MIN_AGE, normalize_handle};
use crate::model::Persona;
use crate::text::stem;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Ok,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Check {
    pub id: String,
    pub severity: Severity,
    pub message: L10n,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CoherenceReport {
    pub score: u8,
    pub checks: Vec<Check>,
    /// 0–100: how easily the accounts can be linked to each other (shared
    /// handle stems, birth year in handles, real name exposure).
    pub linkability: u8,
}

struct R(Vec<Check>);

impl R {
    fn push(&mut self, id: &str, sev: Severity, fr: impl Into<String>, en: impl Into<String>) {
        self.0.push(Check { id: id.into(), severity: sev, message: L10n::new(fr, en) });
    }
    fn ok(&mut self, id: &str, fr: &str, en: &str) {
        self.push(id, Severity::Ok, fr, en);
    }
}

pub fn check(p: &Persona) -> CoherenceReport {
    let mut r = R(Vec::new());
    let d = data();
    let as_of = p.recipe.as_of;
    let year = as_of.year();
    let by = p.identity.birth_date.year();
    let age = p.identity.age as i32;

    // ── Identity ───────────────────────────────────────────────────────────
    let computed = crate::generate::refresh_age(p.identity.birth_date, as_of) as i32;
    if computed != age {
        r.push(
            "age",
            Severity::Error,
            format!("Âge déclaré {age} ≠ âge réel {computed} d'après la date de naissance."),
            format!("Stated age {age} ≠ {computed} from the birth date."),
        );
    } else if age < MIN_AGE as i32 {
        r.push("age", Severity::Error, "La légende est mineure : interdit.", "The legend is a minor: not allowed.");
    } else {
        r.ok("age", "Âge cohérent avec la date de naissance.", "Age matches the birth date.");
    }
    if p.identity.first_name.trim().is_empty() || p.identity.last_name.trim().is_empty() {
        r.push("name", Severity::Error, "Nom ou prénom vide.", "Empty first or last name.");
    }

    // ── Education ──────────────────────────────────────────────────────────
    let mut edu_ok = true;
    let mut prev_end = i32::MIN;
    for e in &p.education {
        if e.start_year < by + 10 || e.end_year < e.start_year {
            edu_ok = false;
            r.push(
                "education_dates",
                Severity::Error,
                format!("{} : dates impossibles ({}–{}).", e.degree, e.start_year, e.end_year),
                format!("{}: impossible dates ({}–{}).", e.degree, e.start_year, e.end_year),
            );
        }
        if e.start_year < prev_end.saturating_sub(1) {
            edu_ok = false;
            r.push(
                "education_order",
                Severity::Warn,
                format!("{} commence avant la fin du cursus précédent.", e.degree),
                format!("{} starts before the previous stage ended.", e.degree),
            );
        }
        if e.completed && e.end_year > year {
            edu_ok = false;
            r.push(
                "education_future",
                Severity::Error,
                format!("{} marqué obtenu mais finit en {}.", e.degree, e.end_year),
                format!("{} marked completed but ends in {}.", e.degree, e.end_year),
            );
        }
        prev_end = e.end_year;
    }
    if edu_ok {
        r.ok("education", "Parcours scolaire chronologique.", "Education timeline is chronological.");
    }

    // ── Career & status ────────────────────────────────────────────────────
    let main: Vec<_> = p.career.iter().filter(|j| !j.side_job).collect();
    let current_main = main.iter().filter(|j| j.end_year.is_none()).count();
    let edu_done = p.education.iter().filter(|e| e.completed && e.level != crate::data::EduLevel::Secondary).map(|e| e.end_year).max();
    let mut career_ok = true;
    for j in &p.career {
        if j.start_year < by + 16 {
            career_ok = false;
            r.push(
                "career_young",
                Severity::Error,
                format!("{} commencé à {} ans.", j.title, j.start_year - by),
                format!("{} started at age {}.", j.title, j.start_year - by),
            );
        }
        if let Some(end) = j.end_year {
            if end < j.start_year {
                career_ok = false;
                r.push(
                    "career_dates",
                    Severity::Error,
                    format!("{} se termine avant d'avoir commencé.", j.title),
                    format!("{} ends before it starts.", j.title),
                );
            }
        }
        if j.start_year > year {
            career_ok = false;
            r.push(
                "career_future",
                Severity::Error,
                format!("{} commence dans le futur.", j.title),
                format!("{} starts in the future.", j.title),
            );
        }
        if let Some(done) = edu_done {
            if !j.side_job && j.start_year < done - 1 && j.end_year.is_some_and(|e| e > j.start_year + 1) {
                r.push(
                    "career_during_studies",
                    Severity::Info,
                    format!("{} commencé pendant les études ({}).", j.title, j.start_year),
                    format!("{} began during studies ({}).", j.title, j.start_year),
                );
            }
        }
    }
    for w in main.windows(2) {
        if let (Some(end), next) = (w[0].end_year, w[1].start_year) {
            if next < end {
                career_ok = false;
                r.push(
                    "career_overlap",
                    Severity::Warn,
                    format!("{} et {} se chevauchent.", w[0].employer, w[1].employer),
                    format!("{} and {} overlap.", w[0].employer, w[1].employer),
                );
            }
        }
    }
    match p.occupation.status.as_str() {
        "working" if current_main != 1 => {
            career_ok = false;
            r.push("status", Severity::Error, "En activité mais aucun poste actuel (ou plusieurs).", "Employed but no single current job.");
        }
        "retired" if current_main > 0 => {
            career_ok = false;
            r.push("status", Severity::Error, "Retraité·e avec un poste en cours.", "Retired but still holding a job.");
        }
        "retired" if age < 55 => {
            r.push(
                "status",
                Severity::Warn,
                format!("Retraite à {age} ans : inhabituel, prépare une explication."),
                format!("Retired at {age}: unusual, have an explanation."),
            );
        }
        "student" if !p.education.iter().any(|e| !e.completed) => {
            career_ok = false;
            r.push("status", Severity::Error, "Étudiant·e sans formation en cours.", "Student with no ongoing course.");
        }
        _ => {}
    }
    if career_ok {
        r.ok("career", "Carrière et statut cohérents.", "Career and status are consistent.");
    }

    // ── Residence ──────────────────────────────────────────────────────────
    if p.residence.since_year < by || p.residence.since_year > year {
        r.push(
            "residence",
            Severity::Error,
            format!("Installé·e en {} : impossible.", p.residence.since_year),
            format!("Moved in {}: impossible.", p.residence.since_year),
        );
    } else {
        r.ok("residence", "Date d'emménagement plausible.", "Move-in date is plausible.");
    }
    if p.residence.tenure == "owner" && age < 23 {
        r.push("tenure", Severity::Warn, "Propriétaire avant 23 ans : rare.", "Home owner before 23: rare.");
    }

    // ── Family ─────────────────────────────────────────────────────────────
    let mut fam_ok = true;
    for c in &p.family.children {
        let parent_age = c.birth_year - by;
        if parent_age < 16 {
            fam_ok = false;
            r.push(
                "child_age",
                Severity::Error,
                format!("{} né·e quand la légende avait {parent_age} ans.", c.first_name),
                format!("{} born when the legend was {parent_age}.", c.first_name),
            );
        } else if parent_age < 19 {
            r.push(
                "child_age",
                Severity::Warn,
                format!("Parent à {parent_age} ans : prépare l'histoire."),
                format!("Parent at {parent_age}: have the story ready."),
            );
        }
        if c.birth_year > year {
            fam_ok = false;
            r.push(
                "child_future",
                Severity::Error,
                format!("{} naît dans le futur.", c.first_name),
                format!("{} is born in the future.", c.first_name),
            );
        }
        if c.first_name == p.identity.first_name {
            r.push(
                "child_name",
                Severity::Info,
                format!("{} porte le même prénom que la légende.", c.first_name),
                format!("{} shares the legend's first name.", c.first_name),
            );
        }
    }
    if let Some(pt) = &p.family.partner {
        let gap = (pt.birth_year - by).abs();
        if gap > 15 {
            r.push(
                "partner_gap",
                Severity::Warn,
                format!("{gap} ans d'écart avec le/la partenaire : ça attire l'attention."),
                format!("{gap}-year gap with the partner draws attention."),
            );
        }
        if year - pt.birth_year < MIN_AGE as i32 {
            fam_ok = false;
            r.push("partner_minor", Severity::Error, "Partenaire mineur·e.", "Partner is a minor.");
        }
    }
    for parent in &p.family.parents {
        if by - parent.birth_year < 15 {
            fam_ok = false;
            r.push(
                "parent_age",
                Severity::Error,
                format!("{} serait parent à {} ans.", parent.first_name, by - parent.birth_year),
                format!("{} would have been a parent at {}.", parent.first_name, by - parent.birth_year),
            );
        }
    }
    for pet in &p.family.pets {
        if pet.since_year > year || pet.since_year < by {
            fam_ok = false;
            r.push(
                "pet",
                Severity::Error,
                format!("{} adopté en {} : impossible.", pet.name, pet.since_year),
                format!("{} adopted in {}: impossible.", pet.name, pet.since_year),
            );
        }
    }
    if fam_ok {
        r.ok("family", "Âges de la famille cohérents.", "Family ages are consistent.");
    }

    // ── Accounts ───────────────────────────────────────────────────────────
    let mut acc_ok = true;
    for a in &p.accounts {
        let Some(plat) = d.platform(&a.platform) else {
            r.push(
                "platform",
                Severity::Warn,
                format!("Plateforme inconnue : {}.", a.platform),
                format!("Unknown platform: {}.", a.platform),
            );
            continue;
        };
        if age < plat.min_age as i32 {
            acc_ok = false;
            r.push(
                "platform_age",
                Severity::Error,
                format!("{} exige {} ans minimum.", plat.name, plat.min_age),
                format!("{} requires age {}+.", plat.name, plat.min_age),
            );
        }
        if let (Some(rule), Some(h)) = (&plat.handle, &a.handle) {
            if let Err(e) = rule.validate(h) {
                acc_ok = false;
                r.push(
                    "handle_rule",
                    Severity::Error,
                    format!("@{h} invalide sur {} ({e}). Suggestion : @{}.", plat.name, normalize_handle(h, rule)),
                    format!("@{h} is invalid on {} ({e}). Suggested: @{}.", plat.name, normalize_handle(h, rule)),
                );
            }
        }
        let launch = crate::generate::launch_year(&a.platform);
        if a.claimed_since < launch {
            acc_ok = false;
            r.push(
                "claimed_since",
                Severity::Error,
                format!("Compte {} « depuis {} » : la plateforme n'était pas encore répandue ({launch}).", plat.name, a.claimed_since),
                format!("{} account 'since {}': the platform wasn't mainstream yet ({launch}).", plat.name, a.claimed_since),
            );
        } else if a.claimed_since < by + plat.min_age as i32 {
            acc_ok = false;
            r.push(
                "claimed_since",
                Severity::Error,
                format!("Compte {} ouvert à {} ans.", plat.name, a.claimed_since - by),
                format!("{} account opened at age {}.", plat.name, a.claimed_since - by),
            );
        } else if a.claimed_since > year {
            acc_ok = false;
            r.push(
                "claimed_since",
                Severity::Error,
                format!("Compte {} ouvert dans le futur.", plat.name),
                format!("{} account opened in the future.", plat.name),
            );
        }
        let z = (age as f64 - plat.peak_age) / plat.spread;
        if z.abs() > 2.2 && a.activity == "active" {
            r.push(
                "platform_fit",
                Severity::Info,
                format!("Très actif·ve sur {} à {age} ans : atypique pour la tranche d'âge.", plat.name),
                format!("Very active on {} at {age}: atypical for the age group.", plat.name),
            );
        }
    }
    if acc_ok {
        r.ok("accounts", "Comptes valides (règles de pseudo, âges, dates).", "Accounts valid (handle rules, ages, dates).");
    }

    // ── Linkability ────────────────────────────────────────────────────────
    let linkability = linkability(p, &mut r);

    // ── Rhythm vs schedule ─────────────────────────────────────────────────
    if p.behavior.heatmap.len() == 7 && p.occupation.schedule == "office" {
        let busy: u32 = (0..5).map(|d| (9..12).chain(14..17).map(|h| p.behavior.heatmap[d][h] as u32).sum::<u32>()).sum();
        let mean = busy as f64 / 30.0;
        if mean > 45.0 {
            r.push(
                "rhythm",
                Severity::Warn,
                "Très actif·ve en ligne pendant les heures de bureau.",
                "Very active online during office hours.",
            );
        } else {
            r.ok("rhythm", "Rythme en ligne compatible avec l'emploi du temps.", "Online rhythm fits the schedule.");
        }
    }
    let tz_ok = data().locale(&p.locale.code).is_some_and(|l| l.cities.iter().any(|c| c.tz == p.behavior.timezone));
    if !tz_ok {
        r.push(
            "timezone",
            Severity::Warn,
            format!("Fuseau {} incohérent avec le pays.", p.behavior.timezone),
            format!("Timezone {} doesn't match the country.", p.behavior.timezone),
        );
    }

    // ── Face ───────────────────────────────────────────────────────────────
    if p.face.is_none() {
        r.push("face", Severity::Info, "Pas encore de visage attribué.", "No face assigned yet.");
    }

    let mut checks = r.0;
    checks.sort_by_key(|c| std::cmp::Reverse(c.severity));
    let penalty: i32 = checks
        .iter()
        .map(|c| match c.severity {
            Severity::Error => 25,
            Severity::Warn => 8,
            Severity::Info => 2,
            Severity::Ok => 0,
        })
        .sum();
    CoherenceReport { score: (100 - penalty).clamp(0, 100) as u8, checks, linkability }
}

/// How easily someone can pivot from one account to the rest. Real people
/// reuse handles; that realism has a cost, and the operator should see it.
fn linkability(p: &Persona, r: &mut R) -> u8 {
    let handles: Vec<(String, String)> =
        p.accounts.iter().filter_map(|a| a.handle.as_ref().map(|h| (a.platform_name.clone(), stem(h)))).collect();
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for (i, (pi, hi)) in handles.iter().enumerate() {
        for (pj, hj) in handles.iter().skip(i + 1) {
            if crate::text::longest_common_substring(hi, hj) >= 5 {
                groups.entry(hi.clone()).or_default().extend([pi.clone(), pj.clone()]);
            }
        }
    }
    let linked: std::collections::BTreeSet<String> = groups.into_values().flatten().collect();
    let yy = format!("{:02}", p.identity.birth_date.year() % 100);
    let yyyy = p.identity.birth_date.year().to_string();
    let with_year: Vec<String> = p
        .accounts
        .iter()
        .filter(|a| a.handle.as_ref().is_some_and(|h| h.contains(&yyyy) || h.ends_with(&yy)))
        .map(|a| a.platform_name.clone())
        .collect();
    let real = p.accounts.iter().filter(|a| a.real_name).count();
    let n = p.accounts.len().max(1) as f64;
    let score = ((linked.len() as f64 / n) * 60.0 + with_year.len() as f64 * 8.0 + real as f64 * 6.0).clamp(0.0, 100.0) as u8;
    if linked.len() >= 2 {
        let l: Vec<String> = linked.into_iter().collect();
        r.push(
            "handle_reuse",
            Severity::Info,
            format!("Pseudos proches sur {} : réaliste, mais un seul pseudo suffit à retrouver les autres comptes.", l.join(", ")),
            format!("Similar handles on {}: realistic, but one handle leads to the others.", l.join(", ")),
        );
    }
    if !with_year.is_empty() {
        r.push(
            "handle_year",
            Severity::Info,
            format!("L'année de naissance apparaît dans le pseudo ({}) : cohérent, mais ça date la légende.", with_year.join(", ")),
            format!("Birth year shows in the handle ({}): consistent, but it dates the legend.", with_year.join(", ")),
        );
    }
    score
}
