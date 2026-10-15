//! # simulacre-core
//!
//! Deterministic generator of fictional identities ("legends") for OSINT
//! sock puppets. A persona is a pure function of its [`Recipe`]; every fact is
//! cross-checked by the [`coherence`] engine; the [`contamination`] module
//! catches overlaps between legends run by the same operator.
//!
//! Guard-rails baked in: adults only, no street addresses, no national ID /
//! bank / card numbers, phone numbers only from ranges reserved for fiction.

// Nested `if let` reads closer to the checks they encode than let-chains.
#![allow(clippy::collapsible_if)]

pub mod coherence;
pub mod contamination;
pub mod data;
pub mod export;
pub mod generate;
pub mod labels;
pub mod model;
pub mod quiz;
pub mod rng;
pub mod story;
pub mod text;

use serde::{Deserialize, Serialize};

pub use generate::{GenError, generate, refresh};
pub use model::{Persona, Recipe};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UiLang {
    #[default]
    Fr,
    En,
}

impl std::str::FromStr for UiLang {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        match s {
            "fr" => Ok(Self::Fr),
            "en" => Ok(Self::En),
            _ => Err(()),
        }
    }
}

/// Lightweight card for galleries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub seed: String,
    pub codename: data::L10n,
    pub name: String,
    pub gender: model::Gender,
    pub age: u8,
    pub city: String,
    pub country_code: String,
    pub archetype: String,
    /// Occupation status (`student`, `working`…), not the vault lifecycle.
    pub occupation: String,
    pub title: Option<String>,
    pub interests: Vec<data::L10n>,
    pub platforms: Vec<String>,
    pub score: u8,
}

impl From<&Persona> for Summary {
    fn from(p: &Persona) -> Self {
        Self {
            seed: p.recipe.seed.clone(),
            codename: p.codename.clone(),
            name: p.identity.full_name(),
            gender: p.identity.gender,
            age: p.identity.age,
            city: p.residence.place.city.clone(),
            country_code: p.locale.country_code.clone(),
            archetype: p.archetype.clone(),
            occupation: p.occupation.status.clone(),
            title: p.occupation.title.clone(),
            interests: p.interests.iter().map(|i| i.label.clone()).collect(),
            platforms: p.accounts.iter().map(|a| a.platform.clone()).collect(),
            score: p.coherence.score,
        }
    }
}
