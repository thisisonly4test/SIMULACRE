//! The forge. [`generate`] turns a [`Recipe`] into a complete, cross-checked
//! [`Persona`]. Sections run in dependency order; each has its own RNG
//! stream (see [`crate::rng`]).

mod digital;
mod family;
mod identity;
mod life;
mod mind;

use chrono::Datelike;

use crate::data::{Archetype, Data, Locale, data};
use crate::model::{Gender, LocaleInfo, Persona, Recipe};
use crate::rng::{RngExt, SectionRng, section_rng};
use crate::{coherence, story};

pub use digital::{launch_year, normalize_handle, shows_join_date, stylize};
pub use identity::age_on as refresh_age;

/// Hard floor: the forge never produces minors.
pub const MIN_AGE: u8 = 18;
pub const MAX_AGE: u8 = 85;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum GenError {
    #[error("unknown locale '{0}'")]
    UnknownLocale(String),
    #[error("unknown archetype '{0}'")]
    UnknownArchetype(String),
    #[error("invalid age range {0}..={1} (allowed {MIN_AGE}..={MAX_AGE})")]
    InvalidAgeRange(u8, u8),
    #[error("seed must not be empty")]
    EmptySeed,
}

/// Shared read-only context for every section.
pub(crate) struct Ctx<'a> {
    pub data: &'a Data,
    pub locale: &'a Locale,
    pub archetype: &'a Archetype,
    pub recipe: &'a Recipe,
    pub year: i32,
    /// Persona language (`fr`, `en`, `de`): bios, posts, job titles.
    pub lang: &'a str,
}

impl Ctx<'_> {
    pub fn rng(&self, section: &str) -> SectionRng {
        section_rng(&self.recipe.seed, section, self.recipe.reroll_count(section))
    }
}

fn resolve_archetype<'a>(d: &'a Data, recipe: &Recipe) -> Result<&'a Archetype, GenError> {
    match recipe.archetype.as_deref() {
        Some(key) if !key.is_empty() => d.archetype(key).ok_or_else(|| GenError::UnknownArchetype(key.to_string())),
        _ => {
            // Respect an explicit age range when the archetype is left open:
            // only archetypes that overlap it are eligible.
            let (lo, hi) = (recipe.age_min.unwrap_or(MIN_AGE), recipe.age_max.unwrap_or(MAX_AGE));
            let mut rng = section_rng(&recipe.seed, "archetype", recipe.reroll_count("archetype"));
            let eligible: Vec<(&Archetype, f64)> =
                d.common.archetypes.iter().filter(|a| a.age[0] <= hi && a.age[1] >= lo).map(|a| (a, 1.0)).collect();
            if eligible.is_empty() { Ok(rng.pick(&d.common.archetypes)) } else { Ok(*rng.pick_weighted(&eligible)) }
        }
    }
}

/// Final age bounds: an explicit range wins over the archetype's typical one.
pub(crate) fn age_bounds(recipe: &Recipe, arch: &Archetype) -> Result<(u8, u8), GenError> {
    let (lo, hi) = (recipe.age_min.unwrap_or(MIN_AGE), recipe.age_max.unwrap_or(MAX_AGE));
    if lo < MIN_AGE || hi > MAX_AGE || lo > hi {
        return Err(GenError::InvalidAgeRange(lo, hi));
    }
    // Narrow to the archetype's typical range when it overlaps the request.
    let (a_lo, a_hi) = (arch.age[0].max(lo), arch.age[1].min(hi));
    Ok(if a_lo <= a_hi { (a_lo, a_hi) } else { (lo, hi) })
}

pub fn generate(recipe: &Recipe) -> Result<Persona, GenError> {
    if recipe.seed.trim().is_empty() {
        return Err(GenError::EmptySeed);
    }
    let d = data();
    let locale = d.locale(&recipe.locale).ok_or_else(|| GenError::UnknownLocale(recipe.locale.clone()))?;
    let archetype = resolve_archetype(d, recipe)?;
    let bounds = age_bounds(recipe, archetype)?;
    let ctx = Ctx { data: d, locale, archetype, recipe, year: recipe.as_of.year(), lang: &locale.language };

    let identity = identity::generate(&ctx, bounds);
    let interests = mind::interests(&ctx, &identity);
    let life = life::generate(&ctx, &identity);
    let family = family::generate(&ctx, &identity, &interests);
    let personality = mind::personality(&ctx, &identity);
    let appearance = mind::appearance(&ctx, &identity, &interests);
    let behavior = mind::behavior(&ctx, &identity, &life.occupation, &personality, &interests, &life.residence.place.tz);
    let dig = digital::accounts(&ctx, &identity, &life, &interests, &personality, &behavior);
    let voice = digital::voice(&ctx, &identity, &life, &family, &interests, &behavior, &dig.accounts);

    let mut persona = Persona {
        recipe: recipe.clone(),
        codename: identity::codename(&ctx, identity.gender),
        archetype: archetype.key.clone(),
        locale: LocaleInfo {
            code: locale.code.clone(),
            language: locale.language.clone(),
            country: locale.country.clone(),
            country_code: locale.country_code.clone(),
        },
        identity,
        appearance,
        residence: life.residence,
        contact: dig.contact,
        occupation: life.occupation,
        education: life.education,
        career: life.career,
        family,
        interests,
        personality,
        behavior,
        accounts: dig.accounts,
        voice,
        timeline: Vec::new(),
        narrative: Default::default(),
        legend: crate::model::Legend { pitch: Default::default(), talking_points: vec![], weak_spots: vec![], opsec: vec![] },
        face: None,
        coherence: Default::default(),
    };
    story::write(&mut persona);
    persona.coherence = coherence::check(&persona);
    Ok(persona)
}

/// Re-derives the parts of a persona that depend on other parts (timeline,
/// narrative, legend, coherence) after a manual edit.
pub fn refresh(persona: &mut Persona) {
    let as_of = persona.recipe.as_of;
    persona.identity.age = identity::age_on(persona.identity.birth_date, as_of);
    story::write(persona);
    persona.coherence = coherence::check(persona);
}

pub(crate) fn pick_gender(rng: &mut SectionRng) -> Gender {
    if rng.chance(0.5) { Gender::Female } else { Gender::Male }
}
