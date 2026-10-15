//! Embedded reference data: locales (names, cities, institutions), languages
//! (bio and post templates), and the shared catalog (sectors, hobbies,
//! archetypes, platforms). Everything is compiled into the binary and parsed
//! once on first use.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// A string in each UI language. Persona-language text (bios, posts, degrees)
/// is a plain `String`; anything the operator reads in the UI is `L10n`.
#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq, Eq, Default)]
pub struct L10n {
    pub fr: String,
    pub en: String,
}

impl L10n {
    pub fn new(fr: impl Into<String>, en: impl Into<String>) -> Self {
        Self { fr: fr.into(), en: en.into() }
    }
    pub fn get(&self, lang: crate::UiLang) -> &str {
        match lang {
            crate::UiLang::Fr => &self.fr,
            crate::UiLang::En => &self.en,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Common {
    pub sectors: Vec<Sector>,
    pub hobbies: Vec<Hobby>,
    pub archetypes: Vec<Archetype>,
    pub platforms: Vec<Platform>,
    pub codename: CodenameWords,
    pub traits: Traits,
}

#[derive(Debug, Deserialize)]
pub struct Sector {
    pub key: String,
    pub label: BTreeMap<String, String>,
    pub jobs: Vec<JobDef>,
}

#[derive(Debug, Deserialize)]
pub struct JobDef {
    pub fr: String,
    pub en: String,
    pub de: String,
    pub edu: EduLevel,
    pub income: u8,
}

impl JobDef {
    /// Job title in the persona's language, gendered when the language marks it
    /// (`"Infirmier|Infirmière"`).
    pub fn title(&self, lang: &str, female: bool) -> String {
        let raw = match lang {
            "fr" => &self.fr,
            "de" => &self.de,
            _ => &self.en,
        };
        gendered(raw, female)
    }
}

/// Picks the masculine or feminine half of a `"masc|fem"` string.
pub fn gendered(raw: &str, female: bool) -> String {
    match raw.split_once('|') {
        Some((m, f)) => if female { f } else { m }.to_string(),
        None => raw.to_string(),
    }
}

#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EduLevel {
    Secondary,
    Vocational,
    Bachelor,
    Master,
    Doctorate,
}

impl EduLevel {
    pub const ALL: [EduLevel; 5] = [Self::Secondary, Self::Vocational, Self::Bachelor, Self::Master, Self::Doctorate];
    pub fn key(self) -> &'static str {
        match self {
            Self::Secondary => "secondary",
            Self::Vocational => "vocational",
            Self::Bachelor => "bachelor",
            Self::Master => "master",
            Self::Doctorate => "doctorate",
        }
    }
    /// Years of study for this stage, on top of the previous one.
    pub fn duration(self, country: &str) -> i32 {
        match (self, country) {
            (Self::Secondary, _) => 3,
            (Self::Vocational, "DE") => 3,
            (Self::Vocational, _) => 2,
            (Self::Bachelor, "US") => 4,
            (Self::Bachelor, _) => 3,
            (Self::Master, "GB") => 1,
            (Self::Master, _) => 2,
            (Self::Doctorate, _) => 3,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Hobby {
    pub key: String,
    pub cat: String,
    pub label: BTreeMap<String, String>,
    pub platforms: Vec<String>,
    pub things: BTreeMap<String, Vec<String>>,
}

impl Hobby {
    pub fn label_in(&self, lang: &str) -> &str {
        self.label.get(lang).or_else(|| self.label.get("en")).map(String::as_str).unwrap_or(&self.key)
    }
    pub fn l10n(&self) -> L10n {
        L10n::new(self.label_in("fr"), self.label_in("en"))
    }
}

#[derive(Debug, Deserialize)]
pub struct Archetype {
    pub key: String,
    pub label: L10n,
    pub desc: L10n,
    pub age: [u8; 2],
    pub status: String,
    pub hobbies: BTreeMap<String, u32>,
    pub sectors: BTreeMap<String, u32>,
    pub platforms: BTreeMap<String, u32>,
    /// Bias added to the Big Five scores: O, C, E, A, N.
    pub ocean: [i32; 5],
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub chronotype: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Platform {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub min_age: u8,
    pub peak_age: f64,
    pub spread: f64,
    pub handle: Option<HandleRule>,
    pub url: Option<String>,
    #[serde(default)]
    pub niche: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HandleRule {
    pub min: usize,
    pub max: usize,
    /// Character class in regex-like shorthand, e.g. `a-z0-9._`.
    pub chars: String,
    pub lower: bool,
}

impl HandleRule {
    pub fn allows(&self, c: char) -> bool {
        let spec: Vec<char> = self.chars.chars().collect();
        let mut i = 0;
        while i < spec.len() {
            if i + 2 < spec.len() && spec[i + 1] == '-' {
                if (spec[i]..=spec[i + 2]).contains(&c) {
                    return true;
                }
                i += 3;
            } else {
                if spec[i] == c {
                    return true;
                }
                i += 1;
            }
        }
        false
    }

    pub fn validate(&self, handle: &str) -> Result<(), String> {
        let n = handle.chars().count();
        if n < self.min || n > self.max {
            return Err(format!("length {n} outside {}..={}", self.min, self.max));
        }
        if let Some(bad) = handle.chars().find(|&c| !self.allows(c)) {
            return Err(format!("character '{bad}' not allowed"));
        }
        if self.lower && handle.chars().any(|c| c.is_uppercase()) {
            return Err("must be lowercase".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CodenameWords {
    pub animals: Vec<CodenameAnimal>,
    pub adjectives: Vec<CodenameAdj>,
}

#[derive(Debug, Deserialize)]
pub struct CodenameAnimal {
    pub fr: String,
    pub g: String,
    pub en: String,
}

#[derive(Debug, Deserialize)]
pub struct CodenameAdj {
    pub fr: String,
    pub en: String,
}

#[derive(Debug, Deserialize)]
pub struct Traits {
    pub values: TraitList,
    pub quirks: TraitList,
    pub peeves: TraitList,
    pub humor: TraitList,
}

#[derive(Debug, Deserialize)]
pub struct TraitList {
    pub fr: Vec<String>,
    pub en: Vec<String>,
}

impl TraitList {
    pub fn len(&self) -> usize {
        self.fr.len().min(self.en.len())
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn at(&self, i: usize) -> L10n {
        L10n::new(&self.fr[i], &self.en[i])
    }
}

#[derive(Debug, Deserialize)]
pub struct Locale {
    pub code: String,
    pub language: String,
    pub country: L10n,
    pub country_code: String,
    pub phone: PhonePlan,
    pub first_names: FirstNames,
    pub last_names: Vec<String>,
    pub cities: Vec<City>,
    pub education: BTreeMap<EduLevel, EduDef>,
    pub schools: Vec<String>,
    pub fields: BTreeMap<String, String>,
    pub employers: BTreeMap<String, Vec<String>>,
    pub words: Vec<String>,
    pub pet_names: BTreeMap<String, Vec<String>>,
    pub housing: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct PhonePlan {
    pub mobile: Option<String>,
    pub landline: Option<String>,
    pub fiction_reserved: bool,
}

#[derive(Debug, Deserialize)]
pub struct FirstNames {
    pub male: Vec<(String, i32)>,
    pub female: Vec<(String, i32)>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize, PartialEq)]
pub struct City {
    pub name: String,
    pub region: String,
    pub postcode: String,
    #[serde(default)]
    pub area: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub tz: String,
    pub pop: u32,
    pub demonym: String,
    pub team: String,
    pub landmark: String,
    pub specialty: String,
    pub districts: Vec<String>,
    pub university: String,
    pub weather: String,
}

#[derive(Debug, Deserialize)]
pub struct EduDef {
    pub degree: String,
    pub institution: String,
}

#[derive(Debug, Deserialize)]
pub struct Lang {
    pub bios: BTreeMap<String, Vec<String>>,
    pub posts: BTreeMap<String, Vec<String>>,
    pub slang: BTreeMap<String, Vec<String>>,
    pub punctuation_space: bool,
}

pub struct Data {
    pub common: Common,
    pub locales: Vec<Locale>,
    pub langs: BTreeMap<String, Lang>,
}

impl Data {
    pub fn locale(&self, code: &str) -> Option<&Locale> {
        self.locales.iter().find(|l| l.code == code)
    }
    pub fn lang(&self, code: &str) -> &Lang {
        self.langs.get(code).unwrap_or_else(|| &self.langs["en"])
    }
    pub fn hobby(&self, key: &str) -> Option<&Hobby> {
        self.common.hobbies.iter().find(|h| h.key == key)
    }
    pub fn sector(&self, key: &str) -> Option<&Sector> {
        self.common.sectors.iter().find(|s| s.key == key)
    }
    pub fn archetype(&self, key: &str) -> Option<&Archetype> {
        self.common.archetypes.iter().find(|a| a.key == key)
    }
    pub fn platform(&self, key: &str) -> Option<&Platform> {
        self.common.platforms.iter().find(|p| p.key == key)
    }
}

fn parse<T: serde::de::DeserializeOwned>(name: &str, raw: &str) -> T {
    serde_json::from_str(raw).unwrap_or_else(|e| panic!("embedded data {name} is invalid: {e}"))
}

/// The process-wide dataset. Parsed lazily; panics only if the embedded JSON is
/// malformed, which the test-suite guards against.
pub fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let locales = vec![
            parse("fr_FR", include_str!("../data/locales/fr_FR.json")),
            parse("en_US", include_str!("../data/locales/en_US.json")),
            parse("en_GB", include_str!("../data/locales/en_GB.json")),
            parse("de_DE", include_str!("../data/locales/de_DE.json")),
        ];
        let mut langs = BTreeMap::new();
        langs.insert("fr".to_string(), parse("lang/fr", include_str!("../data/lang/fr.json")));
        langs.insert("en".to_string(), parse("lang/en", include_str!("../data/lang/en.json")));
        langs.insert("de".to_string(), parse("lang/de", include_str!("../data/lang/de.json")));
        Data { common: parse("common", include_str!("../data/common.json")), locales, langs }
    })
}
