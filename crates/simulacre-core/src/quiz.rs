//! "Interrogation" drill. An operator who hesitates over their own birthday
//! blows the cover. The quiz asks the questions a curious contact would, with
//! plausible wrong answers drawn from the same locale.

use std::collections::BTreeSet;

use chrono::Datelike;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::data::{L10n, data};
use crate::model::Persona;
use crate::rng::{RngExt, section_rng};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Question {
    pub prompt: L10n,
    pub choices: Vec<String>,
    pub answer: usize,
}

fn q(rng: &mut impl Rng, fr: &str, en: &str, right: String, wrong: Vec<String>) -> Option<Question> {
    let mut set: BTreeSet<String> = wrong.into_iter().filter(|w| *w != right && !w.is_empty()).collect();
    if set.len() < 2 || right.is_empty() {
        return None;
    }
    let mut choices: Vec<String> = Vec::new();
    while choices.len() < 3 && !set.is_empty() {
        let v: Vec<String> = set.iter().cloned().collect();
        let pick = rng.pick(&v).clone();
        set.remove(&pick);
        choices.push(pick);
    }
    let answer = rng.gen_range(0..=choices.len());
    choices.insert(answer, right);
    Some(Question { prompt: L10n::new(fr, en), choices, answer })
}

pub fn quiz(p: &Persona, round: u32) -> Vec<Question> {
    let mut rng = section_rng(&p.recipe.seed, "quiz", round);
    let Some(loc) = data().locale(&p.locale.code) else { return vec![] };
    let id = &p.identity;
    let mut out = Vec::new();

    let bd = id.birth_date;
    let fmt = |d: chrono::NaiveDate| d.format("%d/%m/%Y").to_string();
    let wrong_dates: Vec<String> = [(0, 1), (1, 0), (0, -1), (-1, 0), (0, 2)]
        .iter()
        .filter_map(|(dy, dm)| {
            let m = (bd.month() as i32 - 1 + dm).rem_euclid(12) as u32 + 1;
            chrono::NaiveDate::from_ymd_opt(bd.year() + dy, m, bd.day().min(28)).map(fmt)
        })
        .collect();
    out.extend(q(&mut rng, "Ta date de naissance ?", "Your date of birth?", fmt(bd), wrong_dates));

    let cities: Vec<String> = loc.cities.iter().map(|c| c.name.clone()).collect();
    out.extend(q(&mut rng, "Tu es né·e où ?", "Where were you born?", id.birthplace.city.clone(), cities.clone()));
    out.extend(q(
        &mut rng,
        "Tu habites dans quel quartier ?",
        "Which neighbourhood do you live in?",
        p.residence.district.clone(),
        p.residence.local.other_districts.clone(),
    ));

    if let Some(e) = &p.occupation.employer {
        let mut w = Vec::new();
        for _ in 0..6 {
            w.push(format!("{} {}", rng.pick(&loc.words), rng.pick(&["Group", "Services", "SA", "& Co"])));
        }
        w.extend(p.career.iter().filter(|j| &j.employer != e).map(|j| j.employer.clone()));
        out.extend(q(&mut rng, "Tu bosses où en ce moment ?", "Where do you work right now?", e.clone(), w));
    }
    if let Some(j) = p.career.iter().rev().find(|j| !j.side_job) {
        let w = (-3..=3).filter(|d| *d != 0).map(|d| (j.start_year + d).to_string()).collect();
        out.extend(q(
            &mut rng,
            &format!("Depuis quand chez {} ?", j.employer),
            &format!("Since when at {}?", j.employer),
            j.start_year.to_string(),
            w,
        ));
    }
    if let Some(e) = p.education.iter().find(|e| e.level == crate::data::EduLevel::Secondary) {
        let w = loc.schools.iter().map(|s| e.institution.replace(e.institution.split_whitespace().last().unwrap_or(""), s)).collect();
        out.extend(q(&mut rng, "Ton lycée / ton école secondaire ?", "Your secondary school?", e.institution.clone(), w));
    }
    if let Some(s) = p.family.siblings.first() {
        let names = if s.gender.is_female() { &loc.first_names.female } else { &loc.first_names.male };
        let w = names.iter().map(|(n, _)| n.clone()).collect();
        out.extend(q(
            &mut rng,
            "Comment s'appelle ton frère / ta sœur aîné·e ou cadet·te ?",
            "Your sibling's name?",
            s.first_name.clone(),
            w,
        ));
    }
    if let Some(pet) = p.family.pets.first() {
        let w = loc.pet_names.values().flatten().cloned().collect();
        out.extend(q(&mut rng, "Le nom de ton animal ?", "Your pet's name?", pet.name.clone(), w));
    }
    if let Some(pt) = &p.family.partner {
        if pt.alive {
            let names = if pt.gender.is_female() { &loc.first_names.female } else { &loc.first_names.male };
            let w = names.iter().map(|(n, _)| n.clone()).collect();
            out.extend(q(&mut rng, "Le prénom de ton/ta partenaire ?", "Your partner's first name?", pt.first_name.clone(), w));
        }
    }
    for a in p.accounts.iter().filter(|a| a.handle.is_some()).take(2) {
        let h = a.handle.clone().unwrap_or_default();
        let w = vec![
            format!("{h}_"),
            format!("{h}1"),
            h.replace(['.', '_'], ""),
            format!("the{h}"),
            format!("{}{}", h, id.birth_date.year() % 100),
        ];
        out.extend(q(
            &mut rng,
            &format!("Ton pseudo {} ?", a.platform_name),
            &format!("Your {} handle?", a.platform_name),
            format!("@{h}"),
            w.into_iter().filter(|x| *x != h).map(|x| format!("@{x}")).collect(),
        ));
    }
    out.extend(q(
        &mut rng,
        "L'équipe du coin ?",
        "The local team?",
        p.residence.local.team.clone(),
        loc.cities.iter().map(|c| c.team.clone()).collect(),
    ));
    out.extend(q(
        &mut rng,
        "Ta ville est connue pour…",
        "Your city is known for…",
        p.residence.local.landmark.clone(),
        loc.cities.iter().map(|c| c.landmark.clone()).collect(),
    ));

    // Shuffle question order.
    for i in (1..out.len()).rev() {
        let j = rng.gen_range(0..=i);
        out.swap(i, j);
    }
    out.truncate(10);
    out
}
