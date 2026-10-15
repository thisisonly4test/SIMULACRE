//! Digital footprint: which platforms, which handles, what the bios say, and
//! a "voice" — sample posts written in the persona's own style and timed by
//! their activity heatmap.

use std::collections::HashMap;

use chrono::Datelike;
use rand::Rng;

use super::Ctx;
use super::life::Life;
use crate::data::{HandleRule, Platform, gendered};
use crate::model::{Account, Behavior, Contact, Family, Identity, Interest, Personality, Post, WritingStyle};
use crate::rng::{RngExt, SectionRng};
use crate::text::{capitalize, fill, fold};

pub(crate) struct Digital {
    pub accounts: Vec<Account>,
    pub contact: Contact,
}

/// Year each platform became mainstream in Europe/US: the earliest a legend
/// can plausibly claim to have joined.
pub fn launch_year(platform: &str) -> i32 {
    match platform {
        "tripadvisor" => 2000,
        "linkedin" => 2003,
        "steam" | "flickr" => 2004,
        "youtube" | "reddit" => 2005,
        "facebook" | "x" => 2007,
        "goodreads" | "ravelry" => 2007,
        "github" | "inaturalist" => 2008,
        "strava" => 2009,
        "instagram" | "pinterest" | "vivino" => 2010,
        "twitch" | "snapchat" | "letterboxd" => 2011,
        "vinted" => 2013,
        "discord" => 2015,
        "mastodon" => 2017,
        "tiktok" => 2018,
        _ => 2010,
    }
}

/// Platforms that publicly show when an account was created. A legend that
/// claims "on here since 2014" needs a story for a brand-new account.
pub fn shows_join_date(platform: &str) -> bool {
    matches!(platform, "x" | "reddit" | "github" | "youtube" | "steam" | "instagram" | "flickr" | "mastodon" | "letterboxd" | "pinterest")
}

fn real_name_platform(platform: &str) -> bool {
    matches!(platform, "facebook" | "linkedin" | "strava")
}

fn hobby_words(key: &str) -> &'static [&'static str] {
    match key {
        "running" => &["runner", "run", "miles"],
        "cycling" => &["velo", "cyclist", "pedal"],
        "hiking" => &["trails", "hiker", "summit"],
        "fishing" => &["angler", "fisher", "bait"],
        "gardening" => &["garden", "seeds", "veg"],
        "cooking" => &["cooks", "kitchen", "chef"],
        "baking" => &["bakes", "crumbs", "oven"],
        "photography" => &["photo", "lens", "shots"],
        "video_games" => &["gamer", "plays", "gg"],
        "board_games" => &["meeple", "dice"],
        "reading" => &["reads", "books", "pages"],
        "films" => &["films", "cine", "reels"],
        "music_live" => &["gigs", "vinyl", "tunes"],
        "guitar" => &["guitar", "riffs", "strings"],
        "diy" => &["diy", "workshop", "builds"],
        "cars" => &["garage", "motors", "classic"],
        "tech_diy" => &["dev", "hack", "lab", "bits"],
        "fitness" => &["lifts", "fit", "gains"],
        "yoga" => &["yoga", "flow", "zen"],
        "football" => &["foot", "goal", "fan"],
        "travel" => &["travels", "wander", "roads"],
        "knitting" => &["knits", "yarn", "stitch"],
        "birdwatching" => &["birds", "wings", "birder"],
        "anime" => &["otaku", "manga", "anime"],
        "pets" => &["paws", "woof", "meow"],
        "swimming" => &["swims", "laps"],
        "wine" => &["wine", "vino", "cellar"],
        "history" => &["history", "archives"],
        "climbing" => &["climbs", "crux", "boulder"],
        "podcasts" => &["listens", "audio"],
        "skincare" => &["glow", "skin"],
        "fashion" => &["style", "thrift", "fits"],
        _ => &["fan"],
    }
}

const ANON_ADJ: &[&str] =
    &["quiet", "lazy", "salty", "grumpy", "cosmic", "rusty", "sleepy", "lucky", "northern", "little", "old", "wild", "tiny", "late"];
const ANON_NOUN: &[&str] =
    &["fox", "otter", "pigeon", "toast", "badger", "moth", "walrus", "potato", "comet", "kettle", "raven", "pebble", "noodle", "heron"];

/// Rewrites a raw handle so it satisfies a platform's rules. Pure and
/// deterministic: the same raw input always yields the same handle.
pub fn normalize_handle(raw: &str, rule: &HandleRule) -> String {
    let folded = fold(raw);
    let mut out = String::new();
    for c in folded.chars() {
        let c = if rule.lower { c.to_ascii_lowercase() } else { c };
        if c.is_ascii_alphanumeric() && rule.allows(c) {
            out.push(c);
        } else if matches!(c, '.' | '_' | '-' | ' ') {
            let sep = ['.', '_', '-']
                .into_iter()
                .find(|s| *s == c && rule.allows(*s))
                .or_else(|| ['_', '.', '-'].into_iter().find(|s| rule.allows(*s)));
            if let Some(s) = sep {
                if !out.ends_with(['.', '_', '-']) && !out.is_empty() {
                    out.push(s);
                }
            }
        }
    }
    let trim = |s: &mut String| {
        while s.ends_with(['.', '_', '-']) {
            s.pop();
        }
    };
    trim(&mut out);
    if out.chars().count() > rule.max {
        out = out.chars().take(rule.max).collect();
        trim(&mut out);
    }
    // Pad short handles with digits derived from the handle itself.
    let mut seed: u32 = out.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    while out.chars().count() < rule.min {
        out.push(char::from(b'0' + (seed % 10) as u8));
        seed /= 10;
        if seed == 0 {
            seed = 1234;
        }
    }
    out
}

struct Bits {
    first: String,
    last: String,
    nick: Option<String>,
    yy: String,
    hobby: Vec<&'static str>,
    city: String,
}

impl Bits {
    fn name_based(&self, rng: &mut SectionRng) -> String {
        let (f, l) = (&self.first, &self.last);
        let fi = &f[..1];
        let d2 = format!("{:02}", rng.gen_range(1..100));
        match rng.gen_range(0..9) {
            0 => format!("{f}.{l}"),
            1 => format!("{f}{l}"),
            2 => format!("{fi}{l}"),
            3 => format!("{f}_{l}"),
            4 => format!("{f}{l}{}", self.yy),
            5 => format!("{fi}.{l}{d2}"),
            6 => format!("{f}.{}", &l[..1]),
            7 => format!("{l}.{f}"),
            _ => format!("{f}{l}{d2}"),
        }
    }
    fn hobby_based(&self, rng: &mut SectionRng) -> String {
        let w = rng.pick(&self.hobby);
        let base = self.nick.clone().unwrap_or_else(|| self.first.clone());
        match rng.gen_range(0..5) {
            0 => format!("{base}_{w}"),
            1 => format!("{w}{base}"),
            2 => format!("{}{w}", self.city),
            3 => format!("{base}.{w}{}", self.yy),
            _ => format!("{w}_{}", self.last),
        }
    }
    fn anon(&self, rng: &mut SectionRng) -> String {
        format!("{}{}{}", rng.pick(ANON_ADJ), capitalize(rng.pick(ANON_NOUN)), rng.gen_range(2..999))
    }
}

fn persona_word(lang: &str, status: &str, female: bool) -> String {
    let raw = match (lang, status) {
        ("fr", "student") => "Étudiant|Étudiante",
        ("fr", "retired") => "Retraité|Retraitée",
        ("fr", _) => "En recherche d'emploi",
        ("de", "student") => "Student|Studentin",
        ("de", "retired") => "Rentner|Rentnerin",
        ("de", _) => "Auf Jobsuche",
        (_, "student") => "Student",
        (_, "retired") => "Retired",
        _ => "Between jobs",
    };
    gendered(raw, female)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn accounts(ctx: &Ctx, id: &Identity, life: &Life, interests: &[Interest], p: &Personality, b: &Behavior) -> Digital {
    let mut rng = ctx.rng("digital");
    let age = id.age as f64;
    let e = p.ocean[2] as f64;
    let c = p.ocean[1] as f64;

    // ── Which platforms ────────────────────────────────────────────────────
    let mut weights: HashMap<&str, f64> = HashMap::new();
    for (k, w) in &ctx.archetype.platforms {
        *weights.entry(k.as_str()).or_default() += *w as f64 * 2.0;
    }
    for i in interests {
        if let Some(h) = ctx.data.hobby(&i.key) {
            for pk in &h.platforms {
                *weights.entry(pk.as_str()).or_default() += if i.depth == "devoted" { 1.6 } else { 0.9 };
            }
        }
    }
    for base in ["facebook", "instagram", "youtube"] {
        *weights.entry(base).or_default() += 0.6;
    }
    let pro = matches!(life.occupation.status.as_str(), "working" | "between_jobs" | "student")
        && (life.education.iter().any(|e| e.level >= crate::data::EduLevel::Bachelor)
            || matches!(life.occupation.sector.as_deref(), Some("office" | "tech" | "creative")));
    if pro {
        *weights.entry("linkedin").or_default() += 1.2;
    } else {
        weights.remove("linkedin");
    }
    let mut candidates: Vec<(&Platform, f64)> = weights
        .into_iter()
        .filter_map(|(k, w)| ctx.data.platform(k).map(|p| (p, w)))
        .filter(|(p, _)| age >= p.min_age as f64)
        .map(|(p, w)| {
            let z = (age - p.peak_age) / p.spread;
            (p, w * (-0.5 * z * z).exp().max(0.04))
        })
        .collect();
    candidates.sort_by(|a, b| a.0.key.cmp(&b.0.key));
    let n = (2.0 + e / 100.0 * 4.0 + rng.normal(0.0, 0.8)).round().clamp(2.0, 7.0) as usize;
    let chosen = rng.sample_weighted(&candidates, n);

    // ── Handles ────────────────────────────────────────────────────────────
    let bits = Bits {
        first: fold(&id.first_name).to_lowercase().replace([' ', '-', '\''], ""),
        last: fold(&id.last_name).to_lowercase().replace([' ', '-', '\''], ""),
        nick: id.nickname.as_ref().map(|n| fold(n).to_lowercase()),
        yy: format!("{:02}", id.birth_date.year() % 100),
        hobby: interests.iter().flat_map(|i| hobby_words(&i.key).iter().copied()).collect(),
        city: fold(&life.residence.place.city).to_lowercase().split([' ', '-']).next().unwrap_or("").to_string(),
    };
    let reuse = (0.3 + if age < 30.0 { 0.2 } else { 0.0 } + (100.0 - c) / 300.0).clamp(0.1, 0.9);
    let core = if rng.chance(0.5) { bits.name_based(&mut rng) } else { bits.hobby_based(&mut rng) };

    let female = id.gender.is_female();
    let job_word = life
        .occupation
        .title
        .clone()
        .filter(|_| life.occupation.status == "working")
        .unwrap_or_else(|| persona_word(ctx.lang, &life.occupation.status, female));
    let employer = life.occupation.employer.clone().or_else(|| life.education.last().map(|e| e.institution.clone())).unwrap_or_default();
    let years = life.career.iter().filter(|j| !j.side_job).map(|j| j.end_year.unwrap_or(ctx.year) - j.start_year).sum::<i32>().max(1);
    let lang = ctx.data.lang(ctx.lang);
    let hobby_names: Vec<String> = interests.iter().map(|i| i.native_label.clone()).collect();

    let mut accounts = Vec::new();
    for (idx, plat) in chosen.iter().enumerate() {
        let key = plat.key.as_str();
        let real = real_name_platform(key);
        let raw = match key {
            "facebook" => {
                let mut h = format!("{}.{}", bits.first, bits.last);
                if rng.chance(0.5) {
                    h.push_str(&format!(".{}", rng.gen_range(1..999)));
                }
                h
            }
            "linkedin" => {
                let suffix: String = (0..rng.gen_range(6..=9)).map(|_| *rng.pick(&b"0123456789abcdef".map(char::from))).collect();
                format!("{}-{}-{suffix}", bits.first, bits.last)
            }
            "reddit" | "discord" | "steam" | "twitch" | "snapchat" if !rng.chance(reuse) => {
                if rng.chance(0.6) {
                    bits.anon(&mut rng)
                } else {
                    bits.hobby_based(&mut rng)
                }
            }
            _ if rng.chance(reuse) => core.clone(),
            _ => {
                if rng.chance(0.5) {
                    bits.name_based(&mut rng)
                } else {
                    bits.hobby_based(&mut rng)
                }
            }
        };
        let handle = plat.handle.as_ref().map(|rule| normalize_handle(&raw, rule));
        let url = match (&plat.url, &handle) {
            (Some(t), Some(h)) => Some(t.replace("{h}", h)),
            _ => None,
        };
        let display_name = match key {
            "facebook" | "linkedin" | "strava" => id.full_name(),
            "github" if rng.chance(0.6) => id.full_name(),
            "instagram" | "tiktok" => match rng.gen_range(0..3) {
                0 => id.full_name(),
                1 => id.nickname.clone().unwrap_or_else(|| id.first_name.clone()),
                _ => format!("{} {}", id.first_name, b.writing.emojis.first().cloned().unwrap_or_default()).trim().to_string(),
            },
            "x" | "mastodon" | "discord" | "twitch" => id.nickname.clone().unwrap_or_else(|| id.first_name.clone()),
            _ if handle.is_none() => format!(
                "{} {}.",
                id.first_name,
                &id.last_name[..id.last_name.char_indices().nth(1).map(|(i, _)| i).unwrap_or(id.last_name.len())]
            ),
            _ => handle.clone().unwrap_or_default(),
        };

        // Bio.
        let bio_key = if lang.bios.contains_key(key) {
            key
        } else if key == "pinterest" || key == "flickr" {
            "instagram"
        } else {
            "generic"
        };
        let mut slots: HashMap<&str, String> = HashMap::new();
        slots.insert("first", id.first_name.clone());
        slots.insert("city", life.residence.place.city.clone());
        slots.insert("job", job_word.clone());
        slots.insert("hobby", hobby_names.first().cloned().unwrap_or_default());
        slots.insert("hobby2", hobby_names.get(1).cloned().unwrap_or_else(|| hobby_names.first().cloned().unwrap_or_default()));
        slots.insert("emoji", b.writing.emojis.first().cloned().unwrap_or_default());
        slots.insert("emoji2", b.writing.emojis.get(1).cloned().unwrap_or_default());
        slots.insert("age", id.age.to_string());
        slots.insert("team", life.residence.local.team.clone());
        slots.insert("demonym", life.residence.local.demonym.clone());
        slots.insert("specialty", life.residence.local.specialty.clone());
        slots.insert("employer", employer.clone());
        slots.insert("field", life.occupation.sector.as_ref().and_then(|s| ctx.locale.fields.get(s)).cloned().unwrap_or_default());
        slots.insert("years", years.to_string());
        let bio = if (key == "reddit" || key == "discord") && rng.chance(0.5) {
            String::new()
        } else {
            let mut t = fill(rng.pick(&lang.bios[bio_key]), &slots);
            if b.writing.casing == "lowercase" && !real {
                t = t.to_lowercase();
            }
            t.split_whitespace().collect::<Vec<_>>().join(" ")
        };

        let activity = if idx == 0 || rng.chance(e / 130.0) {
            "active"
        } else if rng.chance(0.5) {
            "casual"
        } else {
            "lurker"
        };
        let min_year = launch_year(key).max(id.birth_date.year() + plat.min_age.max(13) as i32);
        let claimed_since = if min_year >= ctx.year { ctx.year } else { rng.gen_range(min_year..=ctx.year.min(min_year + 6)) };
        let mut content: Vec<String> = interests
            .iter()
            .filter(|i| ctx.data.hobby(&i.key).is_some_and(|h| h.platforms.iter().any(|p| p == key)))
            .map(|i| format!("hobby:{}", i.key))
            .collect();
        match key {
            "instagram" | "facebook" | "x" | "mastodon" => content.push("city".into()),
            "linkedin" => content.push("work".into()),
            _ => {}
        }
        if content.is_empty() {
            if let Some(i) = interests.first() {
                content.push(format!("hobby:{}", i.key));
            }
        }
        accounts.push(Account {
            platform: key.into(),
            platform_name: plat.name.clone(),
            handle,
            display_name,
            url,
            bio,
            activity: activity.into(),
            posts_per_week: 0.0,
            claimed_since,
            join_date_public: shows_join_date(key),
            real_name: real,
            content,
        });
    }
    // Spread the weekly post budget over non-lurker accounts.
    let total_w: f32 = accounts
        .iter()
        .map(|a| match a.activity.as_str() {
            "active" => 3.0,
            "casual" => 1.0,
            _ => 0.0,
        })
        .sum();
    if total_w > 0.0 {
        for a in &mut accounts {
            let w = match a.activity.as_str() {
                "active" => 3.0,
                "casual" => 1.0,
                _ => 0.0,
            };
            a.posts_per_week = ((b.posts_per_week * w / total_w) * 10.0).round() / 10.0;
        }
    }

    // ── Contact ────────────────────────────────────────────────────────────
    let phone = ctx.locale.phone.mobile.as_ref().map(|pat| {
        let area = super::identity::find_city(ctx.locale, &life.residence.place.city).area.clone().unwrap_or_default();
        rng.digits(&pat.replace("{area}", &area))
    });
    let mut email_localparts = vec![format!("{}.{}", bits.first, bits.last), format!("{}{}{}", &bits.first[..1], bits.last, bits.yy)];
    email_localparts.push(match &bits.nick {
        Some(n) => format!("{n}.{}{}", bits.last, rng.gen_range(1..99)),
        None => format!("{}.{}{}", bits.first, bits.last, rng.gen_range(1..99)),
    });

    Digital { accounts, contact: Contact { phone, phone_is_fiction_range: ctx.locale.phone.fiction_reserved, email_localparts } }
}

/// Rewrites a neutral sentence in the persona's own voice: casing,
/// punctuation habits, emoji, slang and the occasional typo.
pub fn stylize(text: &str, w: &WritingStyle, rng: &mut impl Rng, hashtag: Option<&str>) -> String {
    let mut s = text.trim().to_string();
    let has = |h: &str| w.habits.iter().any(|x| x == h);
    if has("short_lines") {
        if let Some(cut) = s.find([',', ';']) {
            if cut > 12 {
                s = format!("{}.{}", &s[..cut], &s[cut + 1..]);
            }
        }
    }
    if w.register == "casual" && !w.slang.is_empty() && rng.chance(0.45) {
        let sl = &w.slang[rng.gen_range(0..w.slang.len())];
        s = if rng.chance(0.5) { format!("{sl} {}", crate::text::lc_first(&s)) } else { format!("{s} {sl}") };
    }
    if has("multi_exclaim") {
        s = s.replace('!', "!!!");
        if !s.contains('!') && rng.chance(0.4) {
            s = format!("{}!!", s.trim_end_matches('.'));
        }
    }
    if has("ellipsis") && s.ends_with('.') && rng.chance(0.6) {
        s.pop();
        s.push('…');
    }
    if has("no_final_period") && s.ends_with('.') {
        s.pop();
    }
    if has("haha") && rng.chance(0.4) {
        s.push_str(" haha");
    }
    if !w.emojis.is_empty() && rng.chance(w.emoji_rate as f64) {
        let n = if rng.chance(0.3) { 2 } else { 1 };
        let pick: Vec<&str> = (0..n).map(|_| w.emojis[rng.gen_range(0..w.emojis.len())].as_str()).collect();
        s = format!("{s} {}", pick.join(""));
    }
    if has("hashtags") {
        if let Some(tag) = hashtag {
            s = format!("{s} #{}", fold(tag).replace([' ', '-', '\''], "").to_lowercase());
        }
    }
    // Typos: swap two inner letters of a longer word.
    if w.typo_rate > 0.0 {
        let words: Vec<String> = s
            .split(' ')
            .map(|word| {
                let chars: Vec<char> = word.chars().collect();
                if chars.len() >= 5 && chars.iter().all(|c| c.is_alphabetic()) && rng.chance(w.typo_rate as f64) {
                    let i = rng.gen_range(1..chars.len() - 2);
                    let mut c = chars.clone();
                    c.swap(i, i + 1);
                    c.into_iter().collect()
                } else {
                    word.to_string()
                }
            })
            .collect();
        s = words.join(" ");
    }
    match w.casing.as_str() {
        "lowercase" => s = s.to_lowercase(),
        "expressive" if rng.chance(0.4) => {
            let words: Vec<&str> = s.split(' ').collect();
            if let Some((i, _)) = words
                .iter()
                .enumerate()
                .filter(|(_, x)| x.chars().count() > 3 && x.chars().all(char::is_alphabetic))
                .max_by_key(|(_, x)| x.len())
            {
                let mut owned: Vec<String> = words.iter().map(|x| x.to_string()).collect();
                owned[i] = owned[i].to_uppercase();
                s = owned.join(" ");
            }
        }
        _ => {}
    }
    s
}

pub(crate) fn voice(
    ctx: &Ctx,
    id: &Identity,
    life: &Life,
    family: &Family,
    interests: &[Interest],
    b: &Behavior,
    accounts: &[Account],
) -> Vec<Post> {
    let mut rng = ctx.rng("voice");
    let lang = ctx.data.lang(ctx.lang);
    let mut topics: Vec<(&str, f64)> = vec![("hobby", 4.0), ("city", 2.0), ("food", 1.0), ("mood", 1.0)];
    if interests.iter().any(|i| i.key == "football") || rng.chance(0.25) {
        topics.push(("team", 1.5));
    }
    if life.occupation.status == "working" {
        topics.push(("work", 1.5));
    }
    if !family.pets.is_empty() {
        topics.push(("pet", 1.5));
    }
    if family.children.iter().any(|c| ctx.year - c.birth_year < 16) {
        topics.push(("family", 1.5));
    }
    // Where each kind of post can plausibly appear.
    let fits = |platform: &str, topic: &str, cat: Option<&str>| match platform {
        "instagram" | "x" | "facebook" | "mastodon" | "tiktok" => true,
        "reddit" => matches!(topic, "hobby" | "city" | "mood" | "team"),
        "strava" => topic == "hobby" && matches!(cat, Some("sport" | "outdoor")),
        _ => false,
    };
    let slots_time: Vec<((u8, u8), f64)> = b
        .heatmap
        .iter()
        .enumerate()
        .flat_map(|(d, hours)| hours.iter().enumerate().map(move |(h, v)| ((d as u8, h as u8), (*v as f64).powf(1.5))))
        .collect();

    let mut posts = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut attempts = 0;
    while posts.len() < 8 && attempts < 40 {
        attempts += 1;
        let topic = *rng.pick_weighted(&topics);
        let Some(templates) = lang.posts.get(topic) else { continue };
        let template = rng.pick(templates);
        if !seen.insert(template.as_str()) {
            continue;
        }
        let interest = if topic == "hobby" { Some(rng.pick(interests)) } else { None };
        let mut slots: HashMap<&str, String> = HashMap::new();
        if let Some(i) = interest {
            let h = ctx.data.hobby(&i.key).expect("hobby");
            slots.insert("hobby", i.native_label.clone());
            if let Some(things) = h.things.get(ctx.lang).or_else(|| h.things.get("en")) {
                slots.insert("thing", rng.pick(things).clone());
            }
        }
        let loc = &life.residence.local;
        slots.insert("city", life.residence.place.city.clone());
        slots.insert("district", life.residence.district.clone());
        slots.insert("in_district", crate::text::prep_in(ctx.lang, &life.residence.district));
        slots.insert("de_team", crate::text::prep_de_fr(&loc.team));
        slots.insert("landmark", loc.landmark.clone());
        slots.insert("specialty", loc.specialty.clone());
        slots.insert("team", loc.team.clone());
        slots.insert("pet", family.pets.first().map(|p| p.name.clone()).unwrap_or_default());
        let text = capitalize(&fill(template, &slots));
        let posting: Vec<(&Account, f64)> = accounts
            .iter()
            .filter(|a| fits(&a.platform, topic, interest.map(|i| i.category.as_str())))
            .map(|a| {
                (
                    a,
                    match a.activity.as_str() {
                        "active" => 3.0,
                        "casual" => 1.0,
                        _ => 0.15,
                    },
                )
            })
            .collect();
        let platform = if posting.is_empty() { "generic".to_string() } else { rng.pick_weighted(&posting).platform.clone() };
        let tag = interest.map(|i| i.native_label.as_str()).or(Some(life.residence.place.city.as_str()));
        let styled = stylize(&text, &b.writing, &mut rng, tag.filter(|_| matches!(platform.as_str(), "instagram" | "tiktok" | "x")));
        let (weekday, hour) = *rng.pick_weighted(&slots_time);
        posts.push(Post { platform, topic: topic.into(), text: styled, weekday, hour, minute: rng.gen_range(0..60) });
    }
    let _ = id;
    posts.sort_by_key(|p| (p.weekday, p.hour, p.minute));
    posts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(min: usize, max: usize, chars: &str, lower: bool) -> HandleRule {
        HandleRule { min, max, chars: chars.into(), lower }
    }

    #[test]
    fn normalize_respects_rules() {
        let x = rule(4, 15, "a-zA-Z0-9_", false);
        assert_eq!(normalize_handle("jean.dupont", &x), "jean_dupont");
        let long = normalize_handle("Hélène.Müller-Lüdenscheidt", &x);
        assert!(long.len() <= 15 && !long.ends_with('_'), "{long}");
        let ig = rule(1, 30, "a-z0-9._", true);
        assert_eq!(normalize_handle("Zoé_Lefèvre", &ig), "zoe_lefevre");
        let li = rule(3, 100, "a-z0-9-", true);
        assert_eq!(normalize_handle("anne_marie.roy", &li), "anne-marie-roy");
    }

    #[test]
    fn normalize_pads_and_trims() {
        let x = rule(4, 15, "a-zA-Z0-9_", false);
        let h = normalize_handle("lea", &x);
        assert_eq!(h.len(), 4);
        assert!(h.starts_with("lea"));
        assert_eq!(normalize_handle("_bob_", &x).trim_start_matches('_').len(), normalize_handle("_bob_", &x).len());
        assert!(!normalize_handle("abc..def__", &rule(1, 30, "a-z._", true)).contains(".."));
    }

    #[test]
    fn stylize_lowercase_and_no_period() {
        let w = WritingStyle {
            register: "neutral".into(),
            casing: "lowercase".into(),
            emoji_rate: 0.0,
            emojis: vec![],
            typo_rate: 0.0,
            habits: vec!["no_final_period".into()],
            slang: vec![],
            avg_words: 10,
        };
        let mut r = crate::rng::section_rng("a", "b", 0);
        assert_eq!(stylize("Hello World.", &w, &mut r, None), "hello world");
    }
}
