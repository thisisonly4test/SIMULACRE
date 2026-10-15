use chrono::{Datelike, Days, NaiveDate};
use rand::Rng;

use super::{Ctx, pick_gender};
use crate::data::{City, L10n, Locale, gendered};
use crate::model::{Gender, Identity, LangSkill, Place};
use crate::rng::RngExt;

pub(crate) fn place(c: &City) -> Place {
    Place { city: c.name.clone(), region: c.region.clone(), postcode: c.postcode.clone(), lat: c.lat, lon: c.lon, tz: c.tz.clone() }
}

/// Cities weighted by √population: big cities more likely, small ones still
/// show up.
pub(crate) fn pick_city<'a>(rng: &mut impl Rng, loc: &'a Locale) -> &'a City {
    let w: Vec<(&City, f64)> = loc.cities.iter().map(|c| (c, (c.pop as f64).sqrt())).collect();
    rng.pick_weighted::<&City>(&w)
}

pub(crate) fn find_city<'a>(loc: &'a Locale, name: &str) -> &'a City {
    loc.cities.iter().find(|c| c.name == name).unwrap_or(&loc.cities[0])
}

pub fn age_on(birth: NaiveDate, on: NaiveDate) -> u8 {
    let mut age = on.year() - birth.year();
    if (on.month(), on.day()) < (birth.month(), birth.day()) {
        age -= 1;
    }
    age.clamp(0, 255) as u8
}

/// First name whose popularity peak is near the birth year, the way real
/// cohorts look ("Kevin" is a 1991 name, "Monique" a 1945 one).
pub(crate) fn first_name(rng: &mut impl Rng, loc: &Locale, gender: Gender, birth_year: i32) -> String {
    let list = if gender.is_female() { &loc.first_names.female } else { &loc.first_names.male };
    let w: Vec<(&str, f64)> = list
        .iter()
        .map(|(n, peak)| {
            let z = (birth_year - peak) as f64 / 9.0;
            (n.as_str(), (-z * z).exp() + 0.01)
        })
        .collect();
    rng.pick_weighted(&w).to_string()
}

fn zodiac(d: NaiveDate) -> &'static str {
    let (m, day) = (d.month(), d.day());
    match (m, day) {
        (1, 1..=19) | (12, 22..=31) => "capricorn",
        (1, _) | (2, 1..=18) => "aquarius",
        (2, _) | (3, 1..=20) => "pisces",
        (3, _) | (4, 1..=19) => "aries",
        (4, _) | (5, 1..=20) => "taurus",
        (5, _) | (6, 1..=20) => "gemini",
        (6, _) | (7, 1..=22) => "cancer",
        (7, _) | (8, 1..=22) => "leo",
        (8, _) | (9, 1..=22) => "virgo",
        (9, _) | (10, 1..=22) => "libra",
        (10, _) | (11, 1..=21) => "scorpio",
        _ => "sagittarius",
    }
}

const NICKNAMES: &[(&str, &str)] = &[
    ("Alexandre", "Alex"),
    ("Nicolas", "Nico"),
    ("Sébastien", "Seb"),
    ("Stéphanie", "Steph"),
    ("Stéphane", "Steph"),
    ("Maxime", "Max"),
    ("Thomas", "Tom"),
    ("Guillaume", "Guigui"),
    ("Christophe", "Chris"),
    ("Frédéric", "Fred"),
    ("Benoît", "Ben"),
    ("Mathis", "Math"),
    ("Raphaël", "Raph"),
    ("Gabriel", "Gab"),
    ("Émilie", "Mimi"),
    ("Aurélie", "Aurel"),
    ("Valérie", "Val"),
    ("Catherine", "Cathy"),
    ("Isabelle", "Isa"),
    ("Pauline", "Popo"),
    ("Christopher", "Chris"),
    ("Matthew", "Matt"),
    ("Michael", "Mike"),
    ("Robert", "Rob"),
    ("William", "Will"),
    ("Jennifer", "Jen"),
    ("Samantha", "Sam"),
    ("Daniel", "Dan"),
    ("Joshua", "Josh"),
    ("Andrew", "Andy"),
    ("Jessica", "Jess"),
    ("Stephanie", "Steph"),
    ("Rebecca", "Becky"),
    ("Nicola", "Nicky"),
    ("Elizabeth", "Liz"),
    ("Patricia", "Pat"),
    ("Abigail", "Abby"),
    ("Benjamin", "Ben"),
    ("Jacob", "Jake"),
    ("Charlotte", "Lottie"),
    ("Andreas", "Andi"),
    ("Sebastian", "Basti"),
    ("Katrin", "Kati"),
    ("Christina", "Tina"),
    ("Stefanie", "Steffi"),
    ("Maximilian", "Max"),
    ("Tobias", "Tobi"),
    ("Florian", "Flo"),
    ("Susanne", "Susi"),
    ("Alexander", "Alex"),
    ("Brandon", "Bran"),
    ("Callum", "Cal"),
    ("Connor", "Con"),
    ("Margaret", "Maggie"),
    ("Lukas", "Luki"),
];

/// Heritage languages suggested by a name. Makes "speaks Arabic at home" or
/// "Turkish with grandparents" consistent with the name instead of random.
const HERITAGE: &[(&str, &str)] = &[
    ("Mehdi", "ar"),
    ("Karim", "ar"),
    ("Yanis", "ar"),
    ("Nadia", "ar"),
    ("Yasmine", "ar"),
    ("Inès", "ar"),
    ("Lina", "ar"),
    ("Benali", "ar"),
    ("Haddad", "ar"),
    ("Mohammed", "ar"),
    ("Aisha", "ar"),
    ("Khan", "ur"),
    ("Mehmet", "tr"),
    ("Ayşe", "tr"),
    ("Yılmaz", "tr"),
    ("Kaya", "tr"),
    ("Nguyen", "vi"),
    ("Garcia", "es"),
    ("Martinez", "es"),
    ("Rodriguez", "es"),
    ("Hernandez", "es"),
    ("Lopez", "es"),
    ("Carlos", "es"),
    ("Luis", "es"),
    ("Maria", "es"),
    ("Kowalski", "pl"),
    ("Nowak", "pl"),
    ("Patel", "gu"),
];

fn languages(ctx: &Ctx, rng: &mut impl Rng, first: &str, last: &str, age: u8) -> Vec<LangSkill> {
    let native = ctx.locale.language.clone();
    let mut out = vec![LangSkill { code: native.clone(), level: "native".into() }];
    for (name, code) in HERITAGE {
        if (first == *name || last == *name) && !out.iter().any(|l| l.code == *code) && rng.chance(0.65) {
            let level = *rng.pick(&["fluent", "intermediate", "basic"]);
            out.push(LangSkill { code: (*code).into(), level: level.into() });
        }
    }
    // English as a second language: near-universal for the young outside
    // anglophone countries, rarer for older cohorts.
    if native != "en" {
        let p = if age < 35 {
            0.85
        } else if age < 55 {
            0.6
        } else {
            0.35
        };
        if rng.chance(p) {
            let level = if age < 35 && rng.chance(0.5) { "fluent" } else { *rng.pick(&["intermediate", "basic"]) };
            out.push(LangSkill { code: "en".into(), level: level.into() });
        }
    }
    let extra: &[&str] = match native.as_str() {
        "fr" => &["es", "de", "it"],
        "de" => &["fr", "es", "it"],
        _ => &["es", "fr", "de"],
    };
    if rng.chance(if native == "en" { 0.3 } else { 0.25 }) {
        let code = *rng.pick(extra);
        if !out.iter().any(|l| l.code == code) {
            out.push(LangSkill { code: code.into(), level: (*rng.pick(&["intermediate", "basic", "basic"])).into() });
        }
    }
    out
}

pub(crate) fn generate(ctx: &Ctx, (lo, hi): (u8, u8)) -> Identity {
    let mut rng = ctx.rng("identity");
    let gender = ctx.recipe.gender.unwrap_or_else(|| pick_gender(&mut rng));
    let age = rng.gen_range(lo..=hi);
    let as_of = ctx.recipe.as_of;
    let anchor = as_of
        .with_year(as_of.year() - age as i32)
        .or_else(|| NaiveDate::from_ymd_opt(as_of.year() - age as i32, as_of.month(), 28))
        .expect("valid anchor date");
    // Birthday somewhere in the 364 days before the anchor ⇒ exactly `age` today.
    let birth_date = anchor - Days::new(rng.gen_range(1..=364));
    let first_name = first_name(&mut rng, ctx.locale, gender, birth_date.year());
    let last_name = rng.pick(&ctx.locale.last_names).clone();
    let birthplace = place(pick_city(&mut rng, ctx.locale));
    let nickname = NICKNAMES.iter().find(|(n, _)| *n == first_name).map(|(_, nick)| nick.to_string()).filter(|_| rng.chance(0.7));
    let languages = languages(ctx, &mut rng, &first_name, &last_name, age);
    Identity {
        first_name,
        last_name,
        nickname,
        gender,
        birth_date,
        age: age_on(birth_date, as_of),
        birthplace,
        zodiac: zodiac(birth_date).into(),
        languages,
    }
}

/// Operator-facing codename (`GREY HERON` / `HÉRON GRIS`). Lets you talk about
/// a puppet without saying its cover name out loud.
pub(crate) fn codename(ctx: &Ctx, _gender: Gender) -> L10n {
    let mut rng = ctx.rng("codename");
    let words = &ctx.data.common.codename;
    let animal = rng.pick(&words.animals);
    let adj = rng.pick(&words.adjectives);
    let fr_adj = gendered(&adj.fr, animal.g == "f");
    L10n::new(format!("{} {}", animal.fr, fr_adj).to_uppercase(), format!("{} {}", adj.en, animal.en).to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_on_birthday_edges() {
        let b = NaiveDate::from_ymd_opt(1990, 6, 15).unwrap();
        assert_eq!(age_on(b, NaiveDate::from_ymd_opt(2026, 6, 14).unwrap()), 35);
        assert_eq!(age_on(b, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()), 36);
    }

    #[test]
    fn zodiac_boundaries() {
        let z = |m, d| zodiac(NaiveDate::from_ymd_opt(2000, m, d).unwrap());
        assert_eq!(z(1, 19), "capricorn");
        assert_eq!(z(1, 20), "aquarius");
        assert_eq!(z(12, 22), "capricorn");
        assert_eq!(z(12, 21), "sagittarius");
        assert_eq!(z(7, 23), "leo");
    }
}
