//! The persona ("legend") data model. Everything here serialises to the JSON
//! the web UI consumes. Small enumerations are stored as stable keys
//! (`"owl"`, `"renter"`…) and translated through [`crate::labels`].

use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::coherence::CoherenceReport;
use crate::data::{EduLevel, L10n};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    Female,
    Male,
}

impl Gender {
    pub fn is_female(self) -> bool {
        self == Gender::Female
    }
}

/// Everything needed to rebuild a persona bit-for-bit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    pub seed: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default)]
    pub archetype: Option<String>,
    #[serde(default)]
    pub gender: Option<Gender>,
    #[serde(default)]
    pub age_min: Option<u8>,
    #[serde(default)]
    pub age_max: Option<u8>,
    /// The "today" the legend is written against. Pinning it keeps ages and
    /// timelines stable when the persona is reopened a year later.
    pub as_of: NaiveDate,
    /// Section name → how many times it was rerolled.
    #[serde(default)]
    pub rerolls: BTreeMap<String, u32>,
}

fn default_locale() -> String {
    "fr_FR".into()
}

impl Recipe {
    pub fn new(seed: impl Into<String>, as_of: NaiveDate) -> Self {
        Self {
            seed: seed.into(),
            locale: default_locale(),
            archetype: None,
            gender: None,
            age_min: None,
            age_max: None,
            as_of,
            rerolls: BTreeMap::new(),
        }
    }
    pub fn reroll_count(&self, section: &str) -> u32 {
        self.rerolls.get(section).copied().unwrap_or(0)
    }
}

/// Sections that can be rerolled independently.
pub const SECTIONS: &[&str] = &["identity", "appearance", "life", "family", "interests", "personality", "behavior", "digital", "voice"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Persona {
    pub recipe: Recipe,
    pub codename: L10n,
    pub archetype: String,
    pub locale: LocaleInfo,
    pub identity: Identity,
    pub appearance: Appearance,
    pub residence: Residence,
    pub contact: Contact,
    pub occupation: Occupation,
    pub education: Vec<Education>,
    pub career: Vec<Job>,
    pub family: Family,
    pub interests: Vec<Interest>,
    pub personality: Personality,
    pub behavior: Behavior,
    pub accounts: Vec<Account>,
    pub voice: Vec<Post>,
    pub timeline: Vec<Event>,
    pub narrative: L10n,
    pub legend: Legend,
    #[serde(default)]
    pub face: Option<FaceRef>,
    pub coherence: CoherenceReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocaleInfo {
    pub code: String,
    pub language: String,
    pub country: L10n,
    pub country_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Place {
    pub city: String,
    pub region: String,
    pub postcode: String,
    pub lat: f64,
    pub lon: f64,
    pub tz: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    pub first_name: String,
    pub last_name: String,
    pub nickname: Option<String>,
    pub gender: Gender,
    pub birth_date: NaiveDate,
    pub age: u8,
    pub birthplace: Place,
    pub zodiac: String,
    pub languages: Vec<LangSkill>,
}

impl Identity {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LangSkill {
    pub code: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Appearance {
    pub height_cm: u16,
    pub build: String,
    pub eyes: String,
    pub hair_color: String,
    pub hair_style: String,
    pub glasses: bool,
    pub marks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Residence {
    pub place: Place,
    pub district: String,
    pub housing: String,
    pub housing_label: String,
    pub tenure: String,
    pub since_year: i32,
    pub local: LocalKnowledge,
}

/// What a real local would know. The backbone of the "talking points".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalKnowledge {
    pub demonym: String,
    pub team: String,
    pub landmark: String,
    pub specialty: String,
    pub weather: String,
    pub university: String,
    pub other_districts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contact {
    pub phone: Option<String>,
    pub phone_is_fiction_range: bool,
    pub email_localparts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Occupation {
    /// `student` | `working` | `retired` | `between_jobs`
    pub status: String,
    pub title: Option<String>,
    pub employer: Option<String>,
    pub sector: Option<String>,
    pub income_band: u8,
    pub schedule: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Education {
    pub level: EduLevel,
    pub degree: String,
    pub institution: String,
    pub city: String,
    pub start_year: i32,
    pub end_year: i32,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    pub title: String,
    pub employer: String,
    pub sector: String,
    pub city: String,
    pub start_year: i32,
    pub end_year: Option<i32>,
    pub side_job: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Family {
    /// `single` | `dating` | `partnered` | `married` | `divorced` | `widowed`
    pub relationship: String,
    pub partner: Option<Relative>,
    pub children: Vec<Relative>,
    pub grandchildren: u8,
    pub siblings: Vec<Relative>,
    pub parents: Vec<Relative>,
    pub pets: Vec<Pet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relative {
    pub first_name: String,
    pub gender: Gender,
    pub birth_year: i32,
    pub alive: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pet {
    pub name: String,
    pub species: String,
    pub since_year: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interest {
    pub key: String,
    pub label: L10n,
    pub native_label: String,
    pub category: String,
    /// `casual` | `regular` | `devoted`
    pub depth: String,
    pub since_year: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Personality {
    /// Openness, Conscientiousness, Extraversion, Agreeableness, Neuroticism (0–100).
    pub ocean: [u8; 5],
    pub type_key: String,
    pub values: Vec<L10n>,
    pub quirks: Vec<L10n>,
    pub peeves: Vec<L10n>,
    pub humor: L10n,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Behavior {
    /// `lark` | `intermediate` | `owl`
    pub chronotype: String,
    pub wake: String,
    pub sleep: String,
    pub timezone: String,
    /// 7 days (Mon..Sun) × 24 hours, local time, 0–100 likelihood of being online.
    pub heatmap: Vec<Vec<u8>>,
    pub posts_per_week: f32,
    pub phone: String,
    pub computer: Option<String>,
    pub writing: WritingStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WritingStyle {
    /// `formal` | `neutral` | `casual`
    pub register: String,
    /// `standard` | `lowercase` | `expressive`
    pub casing: String,
    pub emoji_rate: f32,
    pub emojis: Vec<String>,
    pub typo_rate: f32,
    pub habits: Vec<String>,
    pub slang: Vec<String>,
    pub avg_words: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub platform: String,
    pub platform_name: String,
    pub handle: Option<String>,
    pub display_name: String,
    pub url: Option<String>,
    pub bio: String,
    /// `lurker` | `casual` | `active`
    pub activity: String,
    pub posts_per_week: f32,
    /// Year the legend claims the account was opened.
    pub claimed_since: i32,
    /// Platform shows join date publicly: the cover story must explain a fresh account.
    pub join_date_public: bool,
    pub real_name: bool,
    pub content: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Post {
    pub platform: String,
    pub topic: String,
    pub text: String,
    /// 0 = Monday.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub year: i32,
    pub kind: String,
    pub text: L10n,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Legend {
    pub pitch: L10n,
    pub talking_points: Vec<L10n>,
    pub weak_spots: Vec<L10n>,
    pub opsec: Vec<L10n>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FaceRef {
    pub id: String,
    pub source: String,
}
