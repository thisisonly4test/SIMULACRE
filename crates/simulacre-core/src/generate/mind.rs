//! Interests, personality (Big Five), appearance and behaviour. Behaviour is
//! derived, not rolled: the online-activity heatmap follows from chronotype,
//! work schedule and extraversion; the writing style from age, register and
//! conscientiousness.

use chrono::Datelike;
use rand::Rng;

use super::Ctx;
use crate::data::Hobby;
use crate::model::{Appearance, Behavior, Identity, Interest, Occupation, Personality, WritingStyle};
use crate::rng::RngExt;

/// Age drift on hobby popularity: positive = more common with age.
fn age_drift(key: &str) -> f64 {
    match key {
        "gardening" | "history" | "birdwatching" | "knitting" | "wine" | "fishing" => 1.0,
        "reading" | "hiking" | "cooking" => 0.4,
        "video_games" | "anime" | "skincare" | "fashion" | "fitness" | "tiktok" => -1.0,
        "music_live" | "climbing" => -0.5,
        _ => 0.0,
    }
}

pub(crate) fn interests(ctx: &Ctx, id: &Identity) -> Vec<Interest> {
    let mut rng = ctx.rng("interests");
    let age_z = (id.age as f64 - 45.0) / 20.0;
    let pool: Vec<(&Hobby, f64)> = ctx
        .data
        .common
        .hobbies
        .iter()
        .map(|h| {
            let base = ctx.archetype.hobbies.get(&h.key).map(|w| *w as f64 * 3.0).unwrap_or(0.35);
            (h, base * (age_drift(&h.key) * age_z).exp())
        })
        .collect();
    let n = rng.gen_range(3..=5);
    let picked = rng.sample_weighted(&pool, n);
    let by = id.birth_date.year();
    picked
        .into_iter()
        .enumerate()
        .map(|(i, h)| {
            let depth = match i {
                0 if rng.chance(0.55) => "devoted",
                0 | 1 => "regular",
                _ if rng.chance(0.5) => "regular",
                _ => "casual",
            };
            let start_age = rng.gen_range(12..=id.age.max(13) as i32);
            Interest {
                key: h.key.clone(),
                label: h.l10n(),
                native_label: h.label_in(ctx.lang).to_string(),
                category: h.cat.clone(),
                depth: depth.into(),
                since_year: (by + start_age).min(ctx.year),
            }
        })
        .collect()
}

fn type_key(o: [f64; 5]) -> &'static str {
    let [op, c, e, a, n] = o.map(|v| v - 50.0);
    let scores = [
        ("connector", e + a),
        ("organizer", c * 1.6),
        ("explorer", op + e),
        ("observer", op - e),
        ("anchor", c - n),
        ("worrier", n * 1.6),
        ("maverick", op - a),
        ("helper", a * 1.6),
    ];
    scores.iter().max_by(|x, y| x.1.total_cmp(&y.1)).map(|s| s.0).unwrap_or("anchor")
}

pub(crate) fn personality(ctx: &Ctx, id: &Identity) -> Personality {
    let mut rng = ctx.rng("personality");
    let age = id.age as f64;
    // Maturity effect: conscientiousness and agreeableness rise, neuroticism
    // falls with age.
    let maturity = [0.0, (age - 35.0) * 0.25, 0.0, (age - 35.0) * 0.15, -(age - 35.0) * 0.2];
    let mut raw = [0f64; 5];
    for (i, r) in raw.iter_mut().enumerate() {
        *r = rng.normal(50.0, 15.0) + ctx.archetype.ocean[i] as f64 * 1.2 + maturity[i];
        *r = r.clamp(3.0, 97.0);
    }
    let t = &ctx.data.common.traits;
    let pick_n = |rng: &mut crate::rng::SectionRng, list: &crate::data::TraitList, n: usize| {
        let idx: Vec<(usize, f64)> = (0..list.len()).map(|i| (i, 1.0)).collect();
        rng.sample_weighted(&idx, n).into_iter().map(|i| list.at(i)).collect::<Vec<_>>()
    };
    let values = pick_n(&mut rng, &t.values, 2);
    let quirks = pick_n(&mut rng, &t.quirks, 2);
    let peeves = pick_n(&mut rng, &t.peeves, 1);
    // Humour follows extraversion a little: introverts rarely joke online.
    let humor_idx = if raw[2] < 30.0 && rng.chance(0.5) { t.humor.len() - 1 } else { rng.gen_range(0..t.humor.len() - 1) };
    Personality {
        ocean: raw.map(|v| v.round() as u8),
        type_key: type_key(raw).into(),
        values,
        quirks,
        peeves,
        humor: t.humor.at(humor_idx),
    }
}

pub(crate) fn appearance(ctx: &Ctx, id: &Identity, interests: &[Interest]) -> Appearance {
    let mut rng = ctx.rng("appearance");
    let female = id.gender.is_female();
    let (mean, sd) = match (ctx.locale.country_code.as_str(), female) {
        ("DE", false) => (180.0, 7.0),
        ("DE", true) => (166.0, 6.5),
        (_, false) => (176.0, 7.0),
        (_, true) => (163.0, 6.5),
    };
    let height_cm = rng.normal(mean, sd).clamp(148.0, 205.0).round() as u16;
    let sporty = interests.iter().filter(|i| i.category == "sport").count();
    let build = if sporty > 0 && rng.chance(0.55) {
        "athletic"
    } else {
        *rng.pick_weighted(&[("slim", 22.0), ("average", 45.0), ("stocky", 15.0), ("heavy", 13.0 + id.age as f64 / 10.0)])
    };
    let eyes = rng.pick_weighted(match ctx.locale.country_code.as_str() {
        "DE" | "GB" => &[("blue", 35.0), ("brown", 30.0), ("hazel", 12.0), ("green", 13.0), ("grey", 10.0)],
        _ => &[("brown", 48.0), ("blue", 22.0), ("hazel", 14.0), ("green", 11.0), ("grey", 5.0)],
    });
    let p_grey = ((id.age as f64 - 35.0) / 30.0).clamp(0.0, 0.95);
    let hair_color = if rng.chance(p_grey) {
        if id.age > 68 && rng.chance(0.4) { "white" } else { "grey" }
    } else {
        rng.pick_weighted(&[("dark_brown", 35.0), ("light_brown", 28.0), ("black", 17.0), ("blond", 15.0), ("red", 5.0)])
    };
    let hair_style = if !female && id.age > 35 && rng.chance(((id.age as f64 - 30.0) / 50.0).min(0.6)) {
        "bald"
    } else if female {
        rng.pick_weighted(&[("long", 35.0), ("medium", 30.0), ("short", 12.0), ("curly", 13.0), ("bun", 10.0)])
    } else {
        rng.pick_weighted(&[("short", 55.0), ("buzz", 18.0), ("medium", 15.0), ("curly", 10.0), ("long", 2.0)])
    };
    let glasses = rng.chance(0.25 + (id.age as f64 / 100.0) * 0.5);
    let mut marks = Vec::new();
    let pool: &[(&str, f64)] = if female {
        &[("freckles", 1.0), ("tattoo_small", 1.5), ("piercing", 1.2), ("scar_eyebrow", 0.4), ("tattoo_forearm", 0.6)]
    } else {
        &[("beard", 3.0), ("freckles", 0.6), ("tattoo_forearm", 1.2), ("tattoo_small", 0.6), ("scar_eyebrow", 0.5), ("moustache", 0.4)]
    };
    let n_marks = rng.pick_weighted(&[(0usize, 35.0), (1, 45.0), (2, 20.0)]);
    marks.extend(rng.sample_weighted(pool, *n_marks).into_iter().map(String::from));
    Appearance {
        height_cm,
        build: build.into(),
        eyes: eyes.to_string(),
        hair_color: hair_color.into(),
        hair_style: hair_style.into(),
        glasses,
        marks,
    }
}

fn hm(min: i32) -> String {
    let m = min.rem_euclid(24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Builds the 7×24 "when is this person online" matrix in local time.
pub(crate) fn heatmap(rng: &mut impl Rng, wake: i32, sleep: i32, schedule: &str, extraversion: u8, lark: bool) -> Vec<Vec<u8>> {
    let scale = 0.55 + extraversion as f64 / 100.0 * 0.8;
    let mut days = Vec::with_capacity(7);
    for day in 0..7 {
        let weekend = day >= 5;
        let (w, s) = if weekend && !lark { (wake + 90, sleep + 45) } else { (wake, sleep) };
        let mut hours = Vec::with_capacity(24);
        for h in 0..24 {
            let mid = h * 60 + 30;
            // Sleep window may wrap past midnight (sleep > 1440).
            let awake = if s >= 24 * 60 { mid >= w || mid + 24 * 60 < s } else { mid >= w && mid < s };
            let mut v: f64 = if awake { 14.0 } else { 0.0 };
            if awake {
                let since_wake = mid - w;
                if (0..90).contains(&since_wake) {
                    v = 45.0; // the morning scroll
                }
                let work = !weekend
                    && match schedule {
                        "office" | "flexible" => (9..12).contains(&h) || (14..17).contains(&h),
                        "early" => (7..12).contains(&h) || (13..16).contains(&h),
                        "shifts" => (6..14).contains(&h) && day % 2 == 0 || (14..22).contains(&h) && day % 2 == 1,
                        "classes" => (8..12).contains(&h) || (13..16).contains(&h),
                        _ => false,
                    };
                if work {
                    v = if schedule == "classes" { 26.0 } else { 9.0 };
                } else if (12..14).contains(&h) {
                    v = 50.0;
                } else if (18..20).contains(&h) {
                    v = 55.0;
                } else if (20..23).contains(&h) {
                    v = 78.0;
                } else if !(3..23).contains(&h) {
                    v = 62.0;
                } else if weekend || schedule == "free" {
                    v = 40.0;
                }
                if schedule == "free" && (14..17).contains(&h) {
                    v = 52.0;
                }
            }
            let noisy = (v * scale + rng.normal(0.0, 5.0)).clamp(0.0, 100.0);
            hours.push(if awake { noisy.round() as u8 } else { 0 });
        }
        days.push(hours);
    }
    days
}

const CAT_EMOJI: &[(&str, &[&str])] = &[
    ("sport", &["🏃", "💪", "🔥", "🚴", "🏅"]),
    ("outdoor", &["🌲", "⛰️", "🌅", "🎣", "🐦"]),
    ("home", &["🌱", "🍅", "🔨", "🏡", "🍰"]),
    ("creative", &["📷", "🎸", "🎨", "🧶", "✂️"]),
    ("gaming", &["🎮", "🕹️", "👾", "🎲"]),
    ("culture", &["📚", "🎬", "🎶", "🍷", "🎧"]),
    ("tech", &["🔧", "💻", "🚗", "⚙️"]),
    ("lifestyle", &["✨", "💅", "👗", "🌸"]),
];
const GENERAL_EMOJI: &[&str] = &["😂", "🙂", "😅", "❤️", "👌", "🙏", "😍", "🤷", "😊", "👍", "🥲", "💀"];

pub(crate) fn behavior(ctx: &Ctx, id: &Identity, occ: &Occupation, p: &Personality, interests: &[Interest], tz: &str) -> Behavior {
    let mut rng = ctx.rng("behavior");
    let age = id.age;
    let [o, c, e, a, _n] = p.ocean;
    let chronotype = match ctx.archetype.chronotype.as_deref() {
        Some(t) if rng.chance(0.8) => t,
        _ => *rng.pick_weighted(match age {
            0..=29 => &[("owl", 45.0), ("intermediate", 40.0), ("lark", 15.0)],
            30..=54 => &[("owl", 25.0), ("intermediate", 50.0), ("lark", 25.0)],
            _ => &[("owl", 10.0), ("intermediate", 40.0), ("lark", 50.0)],
        }),
    };
    let (mut wake, sleep) = match chronotype {
        "lark" => (rng.gen_range(5 * 60 + 30..=6 * 60 + 45), rng.gen_range(21 * 60 + 45..=23 * 60)),
        "owl" => (rng.gen_range(8 * 60 + 30..=10 * 60), rng.gen_range(24 * 60 + 30..=26 * 60 + 30)),
        _ => (rng.gen_range(6 * 60 + 45..=7 * 60 + 45), rng.gen_range(23 * 60..=24 * 60 + 15)),
    };
    // Work wins over biology on weekdays.
    let latest = match occ.schedule.as_str() {
        "office" | "classes" => 7 * 60 + 40,
        "early" | "shifts" => 5 * 60 + 50,
        _ => 24 * 60,
    };
    wake = wake.min(latest);
    // Nobody holds a job on four hours of sleep for long: an owl with early
    // starts goes to bed earlier on work nights (at least ~6h30 of sleep).
    let sleep = sleep.min(wake + 24 * 60 - 390);
    let heatmap = heatmap(&mut rng, wake, sleep, &occ.schedule, e, chronotype == "lark");

    let posts_per_week = {
        let base = 0.5 + e as f64 / 100.0 * 6.0;
        let age_f = if age < 30 {
            1.5
        } else if age > 55 {
            0.7
        } else {
            1.0
        };
        ((base * age_f * rng.normal(1.0, 0.2).clamp(0.5, 1.6)) * 10.0).round() / 10.0
    } as f32;

    let cc = ctx.locale.country_code.as_str();
    let income = occ.income_band;
    let tinkerer = interests.iter().any(|i| i.key == "tech_diy");
    let phone = if tinkerer && rng.chance(0.4) {
        *rng.pick(&["Google Pixel", "Fairphone"])
    } else {
        let p_iphone = match cc {
            "US" => 0.58,
            "GB" => 0.5,
            "DE" => 0.32,
            _ => 0.33,
        } + income as f64 * 0.04
            + if age < 30 { 0.1 } else { 0.0 };
        if rng.chance(p_iphone) {
            "iPhone"
        } else {
            *rng.pick_weighted(&[
                ("Samsung Galaxy A", 40.0),
                ("Samsung Galaxy S", 20.0),
                ("Xiaomi Redmi", 22.0),
                ("Google Pixel", 8.0),
                ("Motorola", 10.0),
            ])
        }
    };
    let computer = if tinkerer {
        Some(if rng.chance(0.6) { "Linux laptop" } else { "Desktop PC (Windows)" })
    } else if age > 60 && rng.chance(0.4) {
        Some("Tablet")
    } else if rng.chance(0.15) {
        None
    } else if phone == "iPhone" && rng.chance(0.45) {
        Some("MacBook")
    } else if interests.iter().any(|i| i.key == "video_games") {
        Some("Gaming PC (Windows)")
    } else {
        Some("Windows laptop")
    }
    .map(String::from);

    // ── Writing style ──────────────────────────────────────────────────────
    let register = if age > 55 || c > 75 {
        if rng.chance(0.55) { "formal" } else { "neutral" }
    } else if age < 30 && rng.chance(0.6) {
        "casual"
    } else {
        "neutral"
    };
    let casing = if age < 30 && register == "casual" && rng.chance(0.4) {
        "lowercase"
    } else if e > 70 && rng.chance(0.25) {
        "expressive"
    } else {
        "standard"
    };
    let emoji_rate = (0.15
        + if age < 30 {
            0.25
        } else if age > 55 {
            0.2
        } else {
            0.05
        }
        + (a as f32 + e as f32) / 400.0
        + rng.normal(0.0, 0.08) as f32)
        .clamp(0.0, 0.95);
    let mut emoji_pool: Vec<(&str, f64)> = GENERAL_EMOJI.iter().map(|e| (*e, 1.0)).collect();
    for i in interests {
        if let Some((_, list)) = CAT_EMOJI.iter().find(|(c, _)| *c == i.category) {
            emoji_pool.extend(list.iter().map(|e| (*e, 1.6)));
        }
    }
    emoji_pool.sort_by(|x, y| x.0.cmp(y.0));
    emoji_pool.dedup_by(|x, y| x.0 == y.0);
    let emojis: Vec<String> = rng.sample_weighted(&emoji_pool, 3).into_iter().map(String::from).collect();
    let typo_rate = (((100 - c) as f32 / 100.0) * 0.06 + rng.normal(0.0, 0.01) as f32).clamp(0.0, 0.08);
    let habit_pool: Vec<(&str, f64)> = vec![
        ("ellipsis", if age > 45 { 3.0 } else { 1.0 }),
        ("multi_exclaim", if e > 60 { 2.5 } else { 0.8 }),
        ("no_final_period", if age < 35 { 3.0 } else { 0.7 }),
        ("haha", 1.2),
        ("signoff", if age > 60 { 2.5 } else { 0.1 }),
        ("hashtags", if interests.iter().any(|i| matches!(i.category.as_str(), "lifestyle" | "sport" | "creative")) { 1.8 } else { 0.5 }),
        ("short_lines", if o < 40 { 1.8 } else { 0.8 }),
    ];
    let n_habits = rng.gen_range(1..=3);
    let habits: Vec<String> = rng.sample_weighted(&habit_pool, n_habits).into_iter().map(String::from).collect();
    let band = if age < 30 {
        "young"
    } else if age < 55 {
        "mid"
    } else {
        "old"
    };
    let slang_list = ctx.data.lang(ctx.lang).slang.get(band).cloned().unwrap_or_default();
    let slang_pool: Vec<(String, f64)> = slang_list.into_iter().map(|s| (s, 1.0)).collect();
    let slang = rng.sample_weighted(&slang_pool, if register == "formal" { 1 } else { 3 });
    let avg_words =
        (8.0 + o as f64 / 100.0 * 10.0 + if register == "formal" { 4.0 } else { 0.0 } + rng.normal(0.0, 2.0)).clamp(5.0, 30.0) as u8;

    Behavior {
        chronotype: chronotype.into(),
        wake: hm(wake),
        sleep: hm(sleep),
        timezone: tz.to_string(),
        heatmap,
        posts_per_week,
        phone: phone.into(),
        computer,
        writing: WritingStyle { register: register.into(), casing: casing.into(), emoji_rate, emojis, typo_rate, habits, slang, avg_words },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::section_rng;

    #[test]
    fn heatmap_shape_and_sleep() {
        let mut r = section_rng("x", "y", 0);
        let m = heatmap(&mut r, 7 * 60, 23 * 60, "office", 50, false);
        assert_eq!(m.len(), 7);
        assert!(m.iter().all(|d| d.len() == 24));
        // 03:00 on a weekday is asleep.
        assert_eq!(m[1][3], 0);
        // Evening is busier than working hours.
        assert!(m[2][21] > m[2][10]);
    }

    #[test]
    fn heatmap_wraps_past_midnight() {
        let mut r = section_rng("x", "y", 0);
        let m = heatmap(&mut r, 10 * 60, 26 * 60, "free", 80, false);
        assert!(m[0][0] > 0, "an owl sleeping at 2am is online at midnight");
        assert_eq!(m[0][5], 0);
    }

    #[test]
    fn type_key_picks_dominant() {
        assert_eq!(type_key([50.0, 95.0, 50.0, 50.0, 50.0]), "organizer");
        assert_eq!(type_key([50.0, 50.0, 50.0, 50.0, 95.0]), "worrier");
    }
}
