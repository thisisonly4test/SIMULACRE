//! Cross-contamination: two legends run by the same operator must not share
//! anything that lets an observer connect them. This compares every pair of
//! saved personas and explains each overlap.

use serde::{Deserialize, Serialize};

use crate::coherence::Severity;
use crate::data::L10n;
use crate::model::Persona;
use crate::text::{longest_common_substring, stem};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Link {
    pub kind: String,
    pub severity: Severity,
    pub message: L10n,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PairReport {
    pub a: String,
    pub b: String,
    /// 0 = nothing in common, 100 = trivially linkable.
    pub risk: u8,
    pub links: Vec<Link>,
}

/// Pearson correlation of two activity heatmaps. Two "different people" who
/// are online at exactly the same hours are probably one operator.
pub fn rhythm_correlation(a: &[Vec<u8>], b: &[Vec<u8>]) -> f64 {
    let xa: Vec<f64> = a.iter().flatten().map(|v| *v as f64).collect();
    let xb: Vec<f64> = b.iter().flatten().map(|v| *v as f64).collect();
    if xa.len() != xb.len() || xa.is_empty() {
        return 0.0;
    }
    let n = xa.len() as f64;
    let (ma, mb) = (xa.iter().sum::<f64>() / n, xb.iter().sum::<f64>() / n);
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for (x, y) in xa.iter().zip(&xb) {
        cov += (x - ma) * (y - mb);
        va += (x - ma).powi(2);
        vb += (y - mb).powi(2);
    }
    if va == 0.0 || vb == 0.0 { 0.0 } else { cov / (va.sqrt() * vb.sqrt()) }
}

pub fn compare(id_a: &str, a: &Persona, id_b: &str, b: &Persona) -> PairReport {
    let mut links = Vec::new();
    let mut add = |kind: &str, sev: Severity, fr: String, en: String| {
        links.push(Link { kind: kind.into(), severity: sev, message: L10n::new(fr, en) })
    };

    if let (Some(fa), Some(fb)) = (&a.face, &b.face) {
        if fa.id == fb.id {
            add("face", Severity::Error, "Même photo de profil.".into(), "Same profile photo.".into());
        }
    }
    if a.identity.full_name() == b.identity.full_name() {
        add(
            "name",
            Severity::Error,
            format!("Même nom complet : {}.", a.identity.full_name()),
            format!("Same full name: {}.", a.identity.full_name()),
        );
    } else if a.identity.last_name == b.identity.last_name {
        add(
            "surname",
            Severity::Info,
            format!("Même nom de famille ({}).", a.identity.last_name),
            format!("Same surname ({}).", a.identity.last_name),
        );
    }
    if a.identity.birth_date == b.identity.birth_date {
        add("birthday", Severity::Warn, "Même date de naissance.".into(), "Same date of birth.".into());
    }
    if a.residence.place.city == b.residence.place.city && a.residence.district == b.residence.district {
        add(
            "district",
            Severity::Warn,
            format!("Même quartier : {}, {}.", a.residence.district, a.residence.place.city),
            format!("Same neighbourhood: {}, {}.", a.residence.district, a.residence.place.city),
        );
    }
    let emp_a: Vec<&str> = a.career.iter().map(|j| j.employer.as_str()).collect();
    for j in &b.career {
        if emp_a.contains(&j.employer.as_str()) {
            add("employer", Severity::Warn, format!("Employeur commun : {}.", j.employer), format!("Shared employer: {}.", j.employer));
        }
    }
    if a.contact.phone.is_some() && a.contact.phone == b.contact.phone {
        add("phone", Severity::Error, "Même numéro de téléphone.".into(), "Same phone number.".into());
    }
    for ea in &a.contact.email_localparts {
        if b.contact.email_localparts.contains(ea) {
            add("email", Severity::Error, format!("Même identifiant e-mail : {ea}."), format!("Same email local-part: {ea}."));
        }
    }
    for pa in &a.family.pets {
        if b.family.pets.iter().any(|pb| pb.name == pa.name && pb.species == pa.species) {
            add("pet", Severity::Info, format!("Même animal : {}.", pa.name), format!("Same pet: {}.", pa.name));
        }
    }
    for x in &a.accounts {
        let Some(hx) = &x.handle else { continue };
        for y in &b.accounts {
            let Some(hy) = &y.handle else { continue };
            let (sx, sy) = (stem(hx), stem(hy));
            if sx == sy {
                add(
                    "handle",
                    Severity::Error,
                    format!("Pseudo identique : @{hx} ({}) / @{hy} ({}).", x.platform_name, y.platform_name),
                    format!("Identical handle: @{hx} ({}) / @{hy} ({}).", x.platform_name, y.platform_name),
                );
            } else if longest_common_substring(&sx, &sy) >= 6 {
                add("handle", Severity::Warn, format!("Pseudos proches : @{hx} / @{hy}."), format!("Similar handles: @{hx} / @{hy}."));
            }
        }
        for y in &b.accounts {
            if !x.bio.is_empty() && x.bio == y.bio {
                add(
                    "bio",
                    Severity::Warn,
                    format!("Bio identique sur {}.", x.platform_name),
                    format!("Identical bio on {}.", x.platform_name),
                );
            }
        }
    }
    let corr = rhythm_correlation(&a.behavior.heatmap, &b.behavior.heatmap);
    if corr > 0.93 {
        add(
            "rhythm",
            Severity::Warn,
            format!("Rythmes en ligne quasi identiques (corrélation {corr:.2}) : mêmes heures, même opérateur ?"),
            format!("Near-identical online rhythm (correlation {corr:.2}): same hours, same operator?"),
        );
    }
    let shared_hobbies = a.interests.iter().filter(|i| b.interests.iter().any(|j| j.key == i.key)).count();
    if shared_hobbies >= 3 && a.residence.place.city == b.residence.place.city {
        add("profile", Severity::Info, "Même ville et trois loisirs en commun.".into(), "Same city and three shared hobbies.".into());
    }

    let risk: u32 = links
        .iter()
        .map(|l| match l.severity {
            Severity::Error => 45,
            Severity::Warn => 18,
            Severity::Info => 6,
            Severity::Ok => 0,
        })
        .sum();
    PairReport { a: id_a.into(), b: id_b.into(), risk: risk.min(100) as u8, links }
}

/// All pairs, riskiest first. O(n²) — fine for a vault of a few hundred.
pub fn matrix(personas: &[(String, Persona)]) -> Vec<PairReport> {
    let mut out = Vec::new();
    for i in 0..personas.len() {
        for j in i + 1..personas.len() {
            out.push(compare(&personas[i].0, &personas[i].1, &personas[j].0, &personas[j].1));
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p.risk));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correlation_bounds() {
        let a = vec![vec![0, 10, 20, 30]; 7];
        assert!((rhythm_correlation(&a, &a) - 1.0).abs() < 1e-9);
        let flat = vec![vec![5; 4]; 7];
        assert_eq!(rhythm_correlation(&a, &flat), 0.0);
    }
}
