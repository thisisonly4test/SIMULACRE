//! Education, career, residence and current occupation, built as one
//! consistent path: degrees end before jobs start, the last job is in the city
//! the persona lives in, retirees stopped working, students are mid-course.

use std::collections::HashMap;

use chrono::Datelike;
use rand::Rng;

use super::Ctx;
use super::identity::{find_city, pick_city, place};
use crate::data::{City, EduLevel, JobDef, Sector};
use crate::model::{Education, Identity, Job, LocalKnowledge, Occupation, Residence};
use crate::rng::RngExt;
use crate::text::fill;

pub(crate) struct Life {
    pub occupation: Occupation,
    pub education: Vec<Education>,
    pub career: Vec<Job>,
    pub residence: Residence,
}

fn status(ctx: &Ctx, rng: &mut impl Rng, age: u8) -> &'static str {
    let s = match ctx.archetype.status.as_str() {
        "student" => "student",
        "retired" => "retired",
        "working" => "working",
        _ if age < 23 && rng.chance(0.5) => "student",
        _ if age >= 63 => "retired",
        _ => "working",
    };
    match s {
        "student" if age > 30 => "working",
        "retired" if age < 58 => "working",
        "working" if age >= 66 => "retired",
        "working" if rng.chance(0.05) => "between_jobs",
        other => other,
    }
}

fn pick_sector<'a>(ctx: &'a Ctx, rng: &mut impl Rng) -> &'a Sector {
    let w: Vec<(&Sector, f64)> =
        ctx.data.common.sectors.iter().map(|s| (s, ctx.archetype.sectors.get(&s.key).map(|w| *w as f64 * 3.0).unwrap_or(0.4))).collect();
    rng.pick_weighted::<&Sector>(&w)
}

fn employer(ctx: &Ctx, rng: &mut impl Rng, sector: &str, city: &City) -> String {
    let loc = ctx.locale;
    let patterns = loc.employers.get(sector).filter(|_| rng.chance(0.7)).unwrap_or(&loc.employers["default"]);
    let mut slots = HashMap::new();
    slots.insert("surname", rng.pick(&loc.last_names).clone());
    slots.insert("word", rng.pick(&loc.words).clone());
    slots.insert("city", city.name.clone());
    slots.insert("school", rng.pick(&loc.schools).clone());
    fill(rng.pick(patterns), &slots)
}

fn institution(ctx: &Ctx, rng: &mut impl Rng, level: EduLevel, city: &City) -> String {
    let def = &ctx.locale.education[&level];
    let mut slots = HashMap::new();
    slots.insert("school", rng.pick(&ctx.locale.schools).clone());
    slots.insert("city", city.name.clone());
    slots.insert("university", crate::text::capitalize(crate::text::strip_article(&city.university)));
    fill(&def.institution, &slots)
}

fn degree(ctx: &Ctx, level: EduLevel, sector: &str) -> String {
    let mut slots = HashMap::new();
    slots.insert("field", ctx.locale.fields.get(sector).cloned().unwrap_or_default());
    fill(&ctx.locale.education[&level].degree, &slots)
}

/// Stages walked to reach `target` (vocational is an alternative track, not a
/// step towards a bachelor).
fn path_to(target: EduLevel) -> Vec<EduLevel> {
    use EduLevel::*;
    match target {
        Secondary => vec![Secondary],
        Vocational => vec![Secondary, Vocational],
        Bachelor => vec![Secondary, Bachelor],
        Master => vec![Secondary, Bachelor, Master],
        Doctorate => vec![Secondary, Bachelor, Master, Doctorate],
    }
}

fn schedule(sector: &str, rng: &mut impl Rng) -> &'static str {
    match sector {
        "health" | "hospitality" | "logistics" | "industry" if rng.chance(0.65) => "shifts",
        "trades" | "agri" => "early",
        "tech" | "creative" => "flexible",
        _ => "office",
    }
}

pub(crate) fn generate(ctx: &Ctx, id: &Identity) -> Life {
    let mut rng = ctx.rng("life");
    let loc = ctx.locale;
    let country = loc.country_code.as_str();
    let female = id.gender.is_female();
    let birth_year = id.birth_date.year();
    let status = status(ctx, &mut rng, id.age);
    let sector = pick_sector(ctx, &mut rng);

    // Main job: filtered to what the persona could hold at their age.
    let jobs: Vec<&JobDef> = sector.jobs.iter().collect();
    let job = *rng.pick(&jobs);
    let mut target = job.edu;
    if status == "student" {
        target = if id.age >= 22 && rng.chance(0.5) { EduLevel::Master } else { EduLevel::Bachelor };
    } else if target == EduLevel::Bachelor && rng.chance(0.15) {
        target = EduLevel::Master;
    }

    // ── Education ────────────────────────────────────────────────────────────
    let birth_city = find_city(loc, &id.birthplace.city);
    let study_city = if rng.chance(0.55) { birth_city } else { pick_city(&mut rng, loc) };
    let mut education = Vec::new();
    let mut year = birth_year + 15;
    let gap = rng.chance(0.1);
    for (i, level) in path_to(target).into_iter().enumerate() {
        let dur = level.duration(country);
        let start = year + if i == 1 && gap { 1 } else { 0 };
        let end = start + dur;
        let city = if level == EduLevel::Secondary { birth_city } else { study_city };
        let completed = end <= ctx.year;
        if start > ctx.year {
            break;
        }
        education.push(Education {
            level,
            degree: degree(ctx, level, &sector.key),
            institution: institution(ctx, &mut rng, level, city),
            city: city.name.clone(),
            start_year: start,
            end_year: end,
            completed,
        });
        year = end;
        if !completed {
            break;
        }
    }
    // A student whose path finished early is simply still in the last stage.
    if status == "student" {
        if let Some(last) = education.last_mut() {
            if last.completed && last.level != EduLevel::Secondary {
                last.end_year = ctx.year + 1;
                last.completed = false;
            } else if last.completed {
                let start = last.end_year;
                education.push(Education {
                    level: EduLevel::Bachelor,
                    degree: degree(ctx, EduLevel::Bachelor, &sector.key),
                    institution: institution(ctx, &mut rng, EduLevel::Bachelor, study_city),
                    city: study_city.name.clone(),
                    start_year: start.min(ctx.year),
                    end_year: start.min(ctx.year) + EduLevel::Bachelor.duration(country),
                    completed: false,
                });
            }
        }
    }
    let edu_end = education.iter().filter(|e| e.completed).map(|e| e.end_year).max().unwrap_or(birth_year + 18);

    // ── Residence city ──────────────────────────────────────────────────────
    let home: &City = match status {
        "student" => study_city,
        _ if rng.chance(0.5) => birth_city,
        _ => pick_city(&mut rng, loc),
    };

    // ── Career ──────────────────────────────────────────────────────────────
    let mut career = Vec::new();
    let student_job = |rng: &mut _, from: i32, to: Option<i32>, city: &City| -> Job {
        let key = if RngExt::chance(rng, 0.5) { "hospitality" } else { "retail" };
        let s = ctx.data.sector(key).expect("sector");
        let entry: Vec<&JobDef> = s.jobs.iter().filter(|j| j.edu == EduLevel::Secondary).collect();
        let j = *RngExt::pick(rng, &entry);
        Job {
            title: j.title(ctx.lang, female),
            employer: employer(ctx, rng, key, city),
            sector: key.into(),
            city: city.name.clone(),
            start_year: from,
            end_year: to,
            side_job: true,
        }
    };

    let retire_year = birth_year + rng.gen_range(60..=64);
    let career_end = match status {
        "retired" => Some(retire_year.min(ctx.year)),
        "between_jobs" => Some(ctx.year - rng.gen_range(0..=1)),
        _ => None,
    };

    if status == "student" {
        if rng.chance(0.4) {
            let from = education.last().map(|e| e.start_year).unwrap_or(ctx.year).max(birth_year + 18);
            career.push(student_job(&mut rng, from.min(ctx.year), None, home));
        }
    } else {
        if education.iter().any(|e| e.level >= EduLevel::Bachelor) && rng.chance(0.35) {
            let b = education.iter().find(|e| e.level >= EduLevel::Bachelor).expect("bachelor");
            let from = b.start_year + 1;
            career.push(student_job(&mut rng, from, Some((from + 1).min(b.end_year)), study_city));
        }
        let start = (edu_end + rng.gen_range(0..=1)).max(birth_year + 17).min(ctx.year);
        let stop = career_end.unwrap_or(ctx.year);
        let span = (stop - start).max(0);
        let n = (1 + span / 7).clamp(1, 4) as usize;
        // Cut points: earlier jobs in the same sector at the same or lower level.
        let mut cuts: Vec<i32> = (1..n).map(|_| start + rng.gen_range(1..=span.max(1))).collect();
        cuts.sort_unstable();
        cuts.dedup();
        let mut bounds = vec![start];
        bounds.extend(cuts.into_iter().filter(|c| *c > start && *c < stop));
        let earlier: Vec<&JobDef> = sector.jobs.iter().filter(|j| j.edu <= job.edu).collect();
        for (i, from) in bounds.iter().enumerate() {
            let last = i + 1 == bounds.len();
            let to = if last { career_end } else { Some(bounds[i + 1]) };
            let def = if last { job } else { *rng.pick(&earlier) };
            let city = if last || rng.chance(0.6) { home } else { pick_city(&mut rng, loc) };
            // A job change means a new employer: never the same name twice in a row.
            let mut emp = employer(ctx, &mut rng, &sector.key, city);
            for _ in 0..4 {
                if career.last().is_none_or(|prev: &Job| prev.employer != emp) {
                    break;
                }
                emp = employer(ctx, &mut rng, &sector.key, city);
            }
            career.push(Job {
                title: def.title(ctx.lang, female),
                employer: emp,
                sector: sector.key.clone(),
                city: city.name.clone(),
                start_year: *from,
                end_year: to,
                side_job: false,
            });
        }
    }

    let current = career.iter().rev().find(|j| j.end_year.is_none());
    let occupation = Occupation {
        status: status.into(),
        title: current.map(|j| j.title.clone()).or_else(|| career.iter().rev().find(|j| !j.side_job).map(|j| j.title.clone())),
        employer: current.map(|j| j.employer.clone()),
        sector: (status != "student").then(|| sector.key.clone()),
        income_band: match status {
            "student" => 1,
            "between_jobs" => 1,
            _ => job.income,
        },
        schedule: match status {
            "student" => "classes".into(),
            "retired" | "between_jobs" => "free".into(),
            _ => schedule(&sector.key, &mut rng).into(),
        },
    };

    // ── Housing ─────────────────────────────────────────────────────────────
    let age = id.age;
    let house = match age {
        0..=29 => rng.chance(0.15),
        30..=54 => rng.chance(0.45),
        _ => rng.chance(0.65),
    };
    let housing = if house { "house" } else { "apartment" };
    let tenure = match status {
        "student" => *rng.pick(&["student_housing", "flatshare", "family", "renter"]),
        _ if age < 25 && rng.chance(0.3) => "family",
        _ => {
            let p_own = (age as f64 - 25.0) / 40.0 + occupation.income_band as f64 * 0.08;
            if rng.chance(p_own) { "owner" } else { "renter" }
        }
    };
    let since_year = match status {
        "student" => education.last().map(|e| e.start_year).unwrap_or(ctx.year),
        _ => {
            let earliest = (birth_year + 20).min(ctx.year);
            let last_job = career.iter().rev().find(|j| !j.side_job && j.city == home.name).map(|j| j.start_year);
            let base = last_job.unwrap_or(earliest).clamp(earliest, ctx.year);
            if home.name == birth_city.name && tenure == "family" { birth_year } else { rng.gen_range(base..=ctx.year.max(base)) }
        }
    };
    let district = rng.pick(&home.districts).clone();
    let residence = Residence {
        place: place(home),
        district: district.clone(),
        housing: housing.into(),
        housing_label: loc.housing.get(housing).cloned().unwrap_or_default(),
        tenure: tenure.into(),
        since_year,
        local: LocalKnowledge {
            demonym: crate::data::gendered(&home.demonym, female),
            team: home.team.clone(),
            landmark: home.landmark.clone(),
            specialty: home.specialty.clone(),
            weather: home.weather.clone(),
            university: home.university.clone(),
            other_districts: home.districts.iter().filter(|d| **d != district).cloned().collect(),
        },
    };

    Life { occupation, education, career, residence }
}
