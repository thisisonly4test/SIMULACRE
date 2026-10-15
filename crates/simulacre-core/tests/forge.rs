//! End-to-end properties of the forge, checked over thousands of seeds.

use std::collections::HashSet;

use chrono::{Datelike, NaiveDate};
use simulacre_core::coherence::Severity;
use simulacre_core::data::data;
use simulacre_core::labels;
use simulacre_core::model::{Gender, Recipe, SECTIONS};
use simulacre_core::{GenError, UiLang, contamination, export, generate, quiz, refresh, rng};

const LOCALES: [&str; 4] = ["fr_FR", "en_US", "en_GB", "de_DE"];

fn as_of() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
}

fn recipe(seed: &str, locale: &str) -> Recipe {
    let mut r = Recipe::new(seed, as_of());
    r.locale = locale.into();
    r
}

fn corpus() -> impl Iterator<Item = Recipe> {
    let archetypes: Vec<Option<String>> =
        std::iter::once(None).chain(data().common.archetypes.iter().map(|a| Some(a.key.clone()))).collect();
    (0..600u64).map(move |i| {
        let mut r = recipe(&rng::seed_from_u64(i * 7919 + 13), LOCALES[i as usize % LOCALES.len()]);
        r.archetype = archetypes[i as usize % archetypes.len()].clone();
        r
    })
}

#[test]
fn deterministic() {
    let r = recipe("SIM-TEST-0001", "fr_FR");
    let a = serde_json::to_string(&generate(&r).unwrap()).unwrap();
    let b = serde_json::to_string(&generate(&r).unwrap()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn different_seeds_differ() {
    let a = generate(&recipe("SIM-A", "fr_FR")).unwrap();
    let b = generate(&recipe("SIM-B", "fr_FR")).unwrap();
    assert_ne!(a.identity, b.identity);
}

#[test]
fn reroll_touches_only_its_section() {
    let base = recipe("SIM-REROLL", "en_GB");
    let p0 = generate(&base).unwrap();
    let mut r = base.clone();
    r.rerolls.insert("voice".into(), 1);
    let p1 = generate(&r).unwrap();
    assert_eq!(p0.identity, p1.identity);
    assert_eq!(p0.accounts, p1.accounts);
    assert_eq!(p0.career, p1.career);
    assert_ne!(p0.voice, p1.voice);

    let mut r = base.clone();
    r.rerolls.insert("appearance".into(), 3);
    let p2 = generate(&r).unwrap();
    assert_eq!(p0.identity, p2.identity);
    assert_eq!(p0.personality, p2.personality);
    assert_ne!(p0.appearance, p2.appearance);
}

#[test]
fn every_section_reroll_is_valid() {
    for s in SECTIONS {
        let mut r = recipe("SIM-SECT", "de_DE");
        r.rerolls.insert((*s).into(), 5);
        let p = generate(&r).unwrap();
        assert!(p.coherence.checks.iter().all(|c| c.severity != Severity::Error), "{s}: {:?}", p.coherence.checks);
    }
}

#[test]
fn corpus_has_no_coherence_errors() {
    let mut failures = Vec::new();
    for r in corpus() {
        let p = generate(&r).unwrap();
        for c in &p.coherence.checks {
            if c.severity == Severity::Error {
                failures.push(format!("{} {:?}: {}", r.seed, r.archetype, c.message.en));
            }
        }
    }
    assert!(failures.is_empty(), "{} errors, first: {:#?}", failures.len(), &failures[..failures.len().min(10)]);
}

#[test]
fn career_changes_employer_and_sleep_is_humane() {
    for r in corpus() {
        let p = generate(&r).unwrap();
        let main: Vec<_> = p.career.iter().filter(|j| !j.side_job).collect();
        for w in main.windows(2) {
            assert_ne!(w[0].employer, w[1].employer, "{}: same employer twice in a row", r.seed);
        }
        let mins = |s: &str| s[..2].parse::<i32>().unwrap() * 60 + s[3..].parse::<i32>().unwrap();
        let (wake, sleep) = (mins(&p.behavior.wake), mins(&p.behavior.sleep));
        let night = (wake - sleep).rem_euclid(24 * 60);
        assert!(night >= 6 * 60 + 30, "{}: only {night} min of sleep ({} → {})", r.seed, p.behavior.sleep, p.behavior.wake);
    }
}

#[test]
fn corpus_scores_are_high() {
    let scores: Vec<u32> = corpus().map(|r| generate(&r).unwrap().coherence.score as u32).collect();
    let mean = scores.iter().sum::<u32>() as f64 / scores.len() as f64;
    assert!(mean >= 80.0, "mean coherence {mean}");
}

#[test]
fn adults_only_and_age_range_respected() {
    for (i, r) in corpus().enumerate() {
        let mut r = r;
        r.archetype = None;
        r.age_min = Some(40);
        r.age_max = Some(45 + (i % 10) as u8);
        let p = generate(&r).unwrap();
        assert!((40..=r.age_max.unwrap()).contains(&p.identity.age), "{} age {}", r.seed, p.identity.age);
    }
    let mut r = recipe("SIM-MINOR", "fr_FR");
    r.age_min = Some(15);
    assert_eq!(generate(&r).unwrap_err(), GenError::InvalidAgeRange(15, 85));
}

#[test]
fn explicit_gender_is_kept() {
    for i in 0..50 {
        let mut r = recipe(&format!("SIM-G{i}"), LOCALES[i % 4]);
        r.gender = Some(Gender::Female);
        assert_eq!(generate(&r).unwrap().identity.gender, Gender::Female);
    }
}

#[test]
fn bad_inputs_are_rejected() {
    assert_eq!(generate(&recipe("x", "xx_XX")).unwrap_err(), GenError::UnknownLocale("xx_XX".into()));
    let mut r = recipe("x", "fr_FR");
    r.archetype = Some("nope".into());
    assert_eq!(generate(&r).unwrap_err(), GenError::UnknownArchetype("nope".into()));
    assert_eq!(generate(&recipe("  ", "fr_FR")).unwrap_err(), GenError::EmptySeed);
}

#[test]
fn no_unfilled_template_slots() {
    for r in corpus() {
        let p = generate(&r).unwrap();
        let mut texts: Vec<&str> = p.accounts.iter().map(|a| a.bio.as_str()).collect();
        texts.extend(p.voice.iter().map(|v| v.text.as_str()));
        texts.extend(p.career.iter().map(|j| j.employer.as_str()));
        texts.extend(p.education.iter().flat_map(|e| [e.degree.as_str(), e.institution.as_str()]));
        texts.push(&p.narrative.fr);
        texts.push(&p.narrative.en);
        for t in texts {
            assert!(!t.contains('{') && !t.contains('}'), "{}: unfilled slot in {t:?}", r.seed);
        }
    }
}

#[test]
fn handles_satisfy_platform_rules() {
    for r in corpus() {
        let p = generate(&r).unwrap();
        for a in &p.accounts {
            let plat = data().platform(&a.platform).unwrap();
            if let (Some(rule), Some(h)) = (&plat.handle, &a.handle) {
                rule.validate(h).unwrap_or_else(|e| panic!("{} @{h} on {}: {e}", r.seed, a.platform));
            }
            assert!(a.claimed_since <= as_of().year());
        }
    }
}

#[test]
fn phones_only_from_fiction_ranges() {
    for r in corpus() {
        let p = generate(&r).unwrap();
        if let Some(ph) = &p.contact.phone {
            let ok = ph.starts_with("+33 6 39 98 ") || ph.contains(") 555-01") || ph.starts_with("+44 7700 900");
            assert!(ok, "{} unexpected phone {ph}", r.seed);
            assert!(p.contact.phone_is_fiction_range);
        }
    }
}

#[test]
fn labels_cover_every_key() {
    let groups = labels::all();
    for r in corpus().take(200) {
        let p = generate(&r).unwrap();
        let mut keys = vec![
            ("status", p.occupation.status.clone()),
            ("schedule", p.occupation.schedule.clone()),
            ("relationship", p.family.relationship.clone()),
            ("build", p.appearance.build.clone()),
            ("eyes", p.appearance.eyes.clone()),
            ("hair_color", p.appearance.hair_color.clone()),
            ("hair_style", p.appearance.hair_style.clone()),
            ("tenure", p.residence.tenure.clone()),
            ("chronotype", p.behavior.chronotype.clone()),
            ("register", p.behavior.writing.register.clone()),
            ("casing", p.behavior.writing.casing.clone()),
            ("zodiac", p.identity.zodiac.clone()),
            ("ptype", p.personality.type_key.clone()),
        ];
        keys.extend(p.appearance.marks.iter().map(|m| ("marks", m.clone())));
        keys.extend(p.behavior.writing.habits.iter().map(|m| ("habit", m.clone())));
        keys.extend(p.identity.languages.iter().map(|l| ("language", l.code.clone())));
        keys.extend(p.identity.languages.iter().map(|l| ("lang_level", l.level.clone())));
        keys.extend(p.accounts.iter().map(|a| ("activity", a.activity.clone())));
        keys.extend(p.interests.iter().map(|i| ("depth", i.depth.clone())));
        keys.extend(p.voice.iter().map(|v| ("topic", v.topic.clone())));
        for (g, k) in keys {
            assert!(groups.get(g).is_some_and(|m| m.contains_key(k.as_str())), "missing label {g}/{k}");
        }
    }
}

#[test]
fn data_references_resolve() {
    let d = data();
    for a in &d.common.archetypes {
        for h in a.hobbies.keys() {
            assert!(d.hobby(h).is_some(), "archetype {} → hobby {h}", a.key);
        }
        for s in a.sectors.keys() {
            assert!(d.sector(s).is_some(), "archetype {} → sector {s}", a.key);
        }
        for p in a.platforms.keys() {
            assert!(d.platform(p).is_some(), "archetype {} → platform {p}", a.key);
        }
        assert!(a.age[0] >= 18 && a.age[0] <= a.age[1]);
    }
    for h in &d.common.hobbies {
        for p in &h.platforms {
            assert!(d.platform(p).is_some(), "hobby {} → platform {p}", h.key);
        }
        for lang in ["fr", "en", "de"] {
            assert!(h.things.get(lang).is_some_and(|t| !t.is_empty()), "hobby {} missing things.{lang}", h.key);
            assert!(h.label.contains_key(lang), "hobby {} missing label.{lang}", h.key);
        }
    }
    for l in &d.locales {
        for s in &d.common.sectors {
            assert!(l.fields.contains_key(&s.key), "{} missing field for {}", l.code, s.key);
        }
        assert!(l.employers.contains_key("default"));
        assert!(l.cities.len() >= 10);
        assert!(l.cities.iter().all(|c| c.districts.len() >= 3));
        assert_eq!(l.education.len(), 5, "{} education levels", l.code);
        assert!(d.langs.contains_key(&l.language));
    }
}

#[test]
fn refresh_after_edit_flags_contradictions() {
    let mut p = generate(&recipe("SIM-EDIT", "fr_FR")).unwrap();
    assert!(p.coherence.checks.iter().all(|c| c.severity != Severity::Error));
    // Operator edits the birth date to make the persona 15.
    p.identity.birth_date = NaiveDate::from_ymd_opt(2011, 1, 1).unwrap();
    refresh(&mut p);
    assert_eq!(p.identity.age, 15);
    assert!(p.coherence.checks.iter().any(|c| c.id == "age" && c.severity == Severity::Error));
    assert!(p.coherence.score < 80);
}

#[test]
fn bad_handle_edit_is_caught_with_suggestion() {
    let mut p = generate(&recipe("SIM-HANDLE", "en_US")).unwrap();
    let Some(a) = p.accounts.iter_mut().find(|a| a.handle.is_some() && a.platform != "linkedin") else { return };
    a.handle = Some("Not Valid!!".into());
    refresh(&mut p);
    let c = p.coherence.checks.iter().find(|c| c.id == "handle_rule").expect("flagged");
    assert!(c.message.en.contains("Suggested"));
}

#[test]
fn contamination_detects_clone_and_ignores_strangers() {
    let a = generate(&recipe("SIM-C1", "fr_FR")).unwrap();
    let b = generate(&recipe("SIM-C2", "en_US")).unwrap();
    let same = contamination::compare("a", &a, "a2", &a);
    assert_eq!(same.risk, 100);
    assert!(same.links.iter().any(|l| l.kind == "rhythm"));
    let diff = contamination::compare("a", &a, "b", &b);
    assert!(diff.risk < 50, "{:?}", diff.links);
    let m = contamination::matrix(&[("a".into(), a.clone()), ("b".into(), b), ("c".into(), a)]);
    assert_eq!(m.len(), 3);
    assert!(m[0].risk >= m[1].risk);
}

#[test]
fn quiz_is_well_formed() {
    for r in corpus().take(100) {
        let p = generate(&r).unwrap();
        let qs = quiz::quiz(&p, 0);
        assert!(qs.len() >= 6, "{} only {} questions", r.seed, qs.len());
        for q in &qs {
            assert!(q.answer < q.choices.len());
            let uniq: HashSet<_> = q.choices.iter().collect();
            assert_eq!(uniq.len(), q.choices.len(), "duplicate choices {:?}", q.choices);
            assert!(q.choices.len() >= 3);
        }
        assert_ne!(quiz::quiz(&p, 0), quiz::quiz(&p, 1));
    }
}

#[test]
fn markdown_export_contains_core_facts() {
    let p = generate(&recipe("SIM-MD", "en_GB")).unwrap();
    let md = export::markdown(&p, UiLang::En);
    assert!(md.contains(&p.identity.full_name()));
    assert!(md.contains(&p.recipe.seed));
    assert!(md.contains("## Timeline"));
    let fr = export::markdown(&p, UiLang::Fr);
    assert!(fr.contains("## Chronologie"));
}

#[test]
fn json_roundtrip() {
    let p = generate(&recipe("SIM-JSON", "de_DE")).unwrap();
    let s = serde_json::to_string(&p).unwrap();
    let back: simulacre_core::Persona = serde_json::from_str(&s).unwrap();
    assert_eq!(p, back);
}

#[test]
fn locale_flavour_shows() {
    let fr = generate(&recipe("SIM-FLAV", "fr_FR")).unwrap();
    let de = generate(&recipe("SIM-FLAV", "de_DE")).unwrap();
    assert_eq!(fr.locale.language, "fr");
    assert!(data().locale("fr_FR").unwrap().cities.iter().any(|c| c.name == fr.residence.place.city));
    assert!(data().locale("de_DE").unwrap().cities.iter().any(|c| c.name == de.residence.place.city));
    assert_eq!(de.behavior.timezone, "Europe/Berlin");
}
