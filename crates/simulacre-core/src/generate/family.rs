use chrono::Datelike;
use rand::Rng;

use super::Ctx;
use super::identity::first_name;
use crate::model::{Family, Gender, Identity, Interest, Pet, Relative};
use crate::rng::RngExt;

fn other(g: Gender) -> Gender {
    match g {
        Gender::Female => Gender::Male,
        Gender::Male => Gender::Female,
    }
}

fn alive_at(rng: &mut impl Rng, age: i32) -> bool {
    let p = match age {
        ..=69 => 0.96,
        70..=79 => 0.82,
        80..=89 => 0.55,
        _ => 0.2,
    };
    rng.chance(p)
}

fn relationship(rng: &mut impl Rng, age: u8) -> &'static str {
    let table: &[(&str, f64)] = match age {
        0..=24 => &[("single", 55.0), ("dating", 30.0), ("partnered", 15.0)],
        25..=34 => &[("single", 28.0), ("dating", 10.0), ("partnered", 35.0), ("married", 27.0)],
        35..=54 => &[("single", 12.0), ("partnered", 23.0), ("married", 50.0), ("divorced", 15.0)],
        55..=69 => &[("single", 8.0), ("partnered", 10.0), ("married", 57.0), ("divorced", 17.0), ("widowed", 8.0)],
        _ => &[("single", 6.0), ("partnered", 6.0), ("married", 50.0), ("divorced", 13.0), ("widowed", 25.0)],
    };
    rng.pick_weighted(table)
}

pub(crate) fn generate(ctx: &Ctx, id: &Identity, interests: &[Interest]) -> Family {
    let mut rng = ctx.rng("family");
    let loc = ctx.locale;
    let by = id.birth_date.year();
    let year = ctx.year;
    let age = id.age as i32;

    let relationship = relationship(&mut rng, id.age);
    let partner = match relationship {
        "dating" | "partnered" | "married" | "widowed" => {
            let g = if rng.chance(0.06) { id.gender } else { other(id.gender) };
            // Male partners skew ~2 years older, the usual pattern.
            let shift = match (id.gender, g) {
                (Gender::Female, Gender::Male) => 2.0,
                (Gender::Male, Gender::Female) => -2.0,
                _ => 0.0,
            };
            let pby = (by as f64 - rng.normal(shift, 3.0)).round() as i32;
            let pby = pby.clamp(by - 12, (by + 12).min(year - 18));
            Some(Relative {
                first_name: first_name(&mut rng, loc, g, pby),
                gender: g,
                birth_year: pby,
                alive: relationship != "widowed",
                note: None,
            })
        }
        _ => None,
    };

    // Children: anchored to the persona's age at each birth (24–36 for the
    // first, 2–4 years apart), never in the future.
    let wants_kids = matches!(ctx.archetype.family.as_deref(), Some("kids" | "grandkids"));
    let p_kids = match age {
        ..=27 => 0.08,
        28..=39 => 0.5,
        _ => 0.72,
    };
    let mut children = Vec::new();
    if (wants_kids && age >= 26) || (!wants_kids && rng.chance(p_kids)) {
        let n = *rng.pick_weighted(&[(1u8, 35.0), (2, 45.0), (3, 20.0)]);
        let mut cy = by + rng.gen_range(24..=34).min(age - 1);
        for _ in 0..n {
            if cy > year {
                break;
            }
            let g = if rng.chance(0.5) { Gender::Female } else { Gender::Male };
            let mut name = first_name(&mut rng, loc, g, cy);
            if name == id.first_name || children.iter().any(|c: &Relative| c.first_name == name) {
                name = first_name(&mut rng, loc, g, cy + 4);
            }
            children.push(Relative { first_name: name, gender: g, birth_year: cy, alive: true, note: None });
            cy += rng.gen_range(2..=4);
        }
    }
    let grandchildren = {
        let adult_kids = children.iter().filter(|c| year - c.birth_year >= 25).count() as u8;
        if adult_kids > 0 { rng.gen_range(0..=adult_kids * 2).min(6) } else { 0 }
    };

    // Siblings.
    let n_sib = *rng.pick_weighted(&[(0u8, 15.0), (1, 40.0), (2, 30.0), (3, 15.0)]);
    let mut siblings = Vec::new();
    for _ in 0..n_sib {
        let mut sy = by + rng.gen_range(-9..=9);
        if sy == by {
            sy += 2;
        }
        sy = sy.min(year - 1);
        let g = if rng.chance(0.5) { Gender::Female } else { Gender::Male };
        let mut name = first_name(&mut rng, loc, g, sy);
        if name == id.first_name || siblings.iter().any(|s: &Relative| s.first_name == name) {
            name = first_name(&mut rng, loc, g, sy - 6);
        }
        siblings.push(Relative { first_name: name, gender: g, birth_year: sy, alive: alive_at(&mut rng, year - sy), note: None });
    }
    siblings.sort_by_key(|s| s.birth_year);

    // Parents.
    let mby = by - rng.gen_range(22..=36);
    let fby = mby - rng.normal(2.0, 3.0).round() as i32;
    let parents = vec![
        Relative {
            first_name: first_name(&mut rng, loc, Gender::Female, mby),
            gender: Gender::Female,
            birth_year: mby,
            alive: alive_at(&mut rng, year - mby),
            note: None,
        },
        Relative {
            first_name: first_name(&mut rng, loc, Gender::Male, fby),
            gender: Gender::Male,
            birth_year: fby,
            alive: alive_at(&mut rng, year - fby),
            note: None,
        },
    ];

    // Pets.
    let pet_lover = interests.iter().any(|i| i.key == "pets");
    let n_pets = if pet_lover {
        rng.gen_range(1..=2)
    } else if rng.chance(0.35) {
        1
    } else {
        0
    };
    let mut pets: Vec<Pet> = Vec::new();
    for _ in 0..n_pets {
        let species = if rng.chance(0.5) { "dog" } else { "cat" };
        let names = &loc.pet_names[species];
        let name = rng.pick(names).clone();
        if pets.iter().any(|p| p.name == name) {
            continue;
        }
        let max_back = (age - 18).clamp(0, 12);
        pets.push(Pet { name, species: species.into(), since_year: year - rng.gen_range(0..=max_back) });
    }

    Family { relationship: relationship.into(), partner, children, grandchildren, siblings, parents, pets }
}
