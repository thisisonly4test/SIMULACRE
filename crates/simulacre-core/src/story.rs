//! Turns the structured persona into prose: a dated timeline, a backstory in
//! both UI languages, and the operator's "legend card" (elevator pitch,
//! local talking points, weak spots, opsec reminders).

use chrono::Datelike;

use crate::UiLang;
use crate::data::L10n;
use crate::labels::{l10n, label};
use crate::text::prep_in;

thread_local! {
    static GERMAN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Job titles are common nouns mid-sentence, except in German where every
/// noun keeps its capital.
fn lc_first(s: &str) -> String {
    if GERMAN.with(|g| g.get()) { s.to_string() } else { crate::text::lc_first(s) }
}
use crate::model::{Event, Legend, Persona};

const MONTHS_FR: [&str; 12] =
    ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
const MONTHS_EN: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

pub fn write(p: &mut Persona) {
    GERMAN.with(|g| g.set(p.locale.language == "de"));
    p.timeline = timeline(p);
    p.narrative = L10n::new(narrative(p, UiLang::Fr), narrative(p, UiLang::En));
    p.legend = legend(p);
}

fn ev(year: i32, kind: &str, fr: String, en: String) -> Event {
    Event { year, kind: kind.into(), text: L10n::new(fr, en) }
}

fn timeline(p: &Persona) -> Vec<Event> {
    let id = &p.identity;
    let mut t =
        vec![ev(id.birth_date.year(), "birth", format!("Naissance à {}", id.birthplace.city), format!("Born in {}", id.birthplace.city))];
    for e in &p.education {
        if e.completed {
            t.push(ev(
                e.end_year,
                "education",
                format!("Obtient : {} — {}", e.degree, e.institution),
                format!("Completes {} — {}", e.degree, e.institution),
            ));
        } else {
            t.push(ev(
                e.start_year,
                "education",
                format!("Commence : {} — {}", e.degree, e.institution),
                format!("Starts {} — {}", e.degree, e.institution),
            ));
        }
    }
    for j in &p.career {
        let (fr, en) = if j.side_job {
            (format!("Job étudiant : {} chez {}", j.title, j.employer), format!("Student job: {} at {}", j.title, j.employer))
        } else {
            (format!("{} chez {} ({})", j.title, j.employer, j.city), format!("{} at {} ({})", j.title, j.employer, j.city))
        };
        t.push(ev(j.start_year, "job", fr, en));
    }
    if p.occupation.status == "retired" {
        if let Some(end) = p.career.iter().filter_map(|j| j.end_year).max() {
            t.push(ev(end, "retirement", "Départ à la retraite".into(), "Retires".into()));
        }
    }
    if p.residence.place.city != id.birthplace.city || p.residence.since_year > id.birth_date.year() {
        t.push(ev(
            p.residence.since_year,
            "move",
            format!("S'installe à {} ({})", p.residence.place.city, p.residence.district),
            format!("Moves to {} ({})", p.residence.place.city, p.residence.district),
        ));
    }
    for c in &p.family.children {
        t.push(ev(c.birth_year, "child", format!("Naissance de {}", c.first_name), format!("{} is born", c.first_name)));
    }
    for pet in &p.family.pets {
        let sp = l10n("species", &pet.species);
        t.push(ev(pet.since_year, "pet", format!("Adopte {} ({})", pet.name, sp.fr), format!("Adopts {} the {}", pet.name, sp.en)));
    }
    for a in &p.accounts {
        t.push(ev(a.claimed_since, "account", format!("Ouvre un compte {}", a.platform_name), format!("Joins {}", a.platform_name)));
    }
    t.sort_by(|a, b| a.year.cmp(&b.year).then_with(|| kind_rank(&a.kind).cmp(&kind_rank(&b.kind))));
    t
}

fn kind_rank(k: &str) -> u8 {
    match k {
        "birth" => 0,
        "education" => 1,
        "job" => 2,
        "move" => 3,
        "child" => 4,
        "pet" => 5,
        "retirement" => 6,
        _ => 7,
    }
}

fn list(items: &[String], lang: UiLang) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => {
            let and = if lang == UiLang::Fr { "et" } else { "and" };
            format!("{} {and} {}", items[..n - 1].join(", "), items[n - 1])
        }
    }
}

fn date(p: &Persona, lang: UiLang) -> String {
    let d = p.identity.birth_date;
    match lang {
        UiLang::Fr => format!("{} {} {}", d.day(), MONTHS_FR[d.month0() as usize], d.year()),
        UiLang::En => format!("{} {}, {}", MONTHS_EN[d.month0() as usize], d.day(), d.year()),
    }
}

fn narrative(p: &Persona, lang: UiLang) -> String {
    let id = &p.identity;
    let f = id.gender.is_female();
    let r = &p.residence;
    let fr = lang == UiLang::Fr;
    let (il, lui) = if fr {
        if f { ("Elle", "elle") } else { ("Il", "lui") }
    } else if f {
        ("She", "her")
    } else {
        ("He", "him")
    };
    let mut paras = Vec::new();

    // 1 — who and where.
    let tenure = label("tenure", &r.tenure, lang);
    let mut s = if fr {
        format!(
            "{} {}, {} ans, est {} le {} à {}. ",
            id.first_name,
            id.last_name,
            id.age,
            if f { "née" } else { "né" },
            date(p, lang),
            id.birthplace.city
        )
    } else {
        format!("{} {}, {}, was born on {} in {}. ", id.first_name, id.last_name, id.age, date(p, lang), id.birthplace.city)
    };
    let stayed = r.place.city == id.birthplace.city
        && p.career.iter().all(|j| j.city == r.place.city)
        && p.education.iter().all(|e| e.city == r.place.city);
    let district = prep_in(if fr { "fr" } else { "en" }, &r.district);
    if stayed {
        s += &if fr {
            format!(
                "{il} n'a jamais vraiment quitté {} et vit aujourd'hui {district} ({}, {}) depuis {}.",
                r.place.city, r.housing_label, tenure, r.since_year
            )
        } else {
            format!(
                "{il} never really left {} and has lived {district} ({}, {}) since {}.",
                r.place.city, r.housing_label, tenure, r.since_year
            )
        };
    } else {
        s += &if fr {
            format!("{il} vit à {}, {district}, depuis {} ({}, {}).", r.place.city, r.since_year, r.housing_label, tenure)
        } else {
            format!("{il} has lived in {}, {district}, since {} ({}, {}).", r.place.city, r.since_year, r.housing_label, tenure)
        };
    }
    paras.push(s);

    // 2 — path.
    let mut s = String::new();
    if let Some(last) = p.education.iter().rev().find(|e| e.completed) {
        s += &if fr {
            format!("Diplôme le plus élevé : {} ({}, {}). ", last.degree, last.institution, last.end_year)
        } else {
            format!("Highest qualification: {} ({}, {}). ", last.degree, last.institution, last.end_year)
        };
    }
    let o = &p.occupation;
    let main_jobs: Vec<_> = p.career.iter().filter(|j| !j.side_job).collect();
    match o.status.as_str() {
        "student" => {
            if let Some(cur) = p.education.iter().find(|e| !e.completed) {
                s += &if fr {
                    format!("{il} prépare actuellement un {} à {}.", cur.degree, cur.institution)
                } else {
                    format!("{il} is currently studying for a {} at {}.", cur.degree, cur.institution)
                };
            }
            if let Some(j) = p.career.iter().find(|j| j.end_year.is_none()) {
                s += &if fr {
                    format!(" Pour payer le loyer : {} chez {}.", lc_first(&j.title), j.employer)
                } else {
                    format!(" Pays the rent working as {} at {}.", lc_first(&j.title), j.employer)
                };
            }
        }
        "retired" => {
            if let Some(j) = main_jobs.last() {
                s += &if fr {
                    format!(
                        "{} depuis {}, {} a fait l'essentiel de sa carrière comme {} ({} postes au total).",
                        if f { "Retraitée" } else { "Retraité" },
                        j.end_year.unwrap_or(j.start_year),
                        id.first_name,
                        lc_first(&j.title),
                        main_jobs.len()
                    )
                } else {
                    format!(
                        "Retired since {}, {} spent most of {} career as {} ({} jobs in total).",
                        j.end_year.unwrap_or(j.start_year),
                        id.first_name,
                        if f { "her" } else { "his" },
                        lc_first(&j.title),
                        main_jobs.len()
                    )
                };
            }
        }
        _ => {
            if let Some(j) = main_jobs.last() {
                if o.status == "between_jobs" {
                    s += &if fr {
                        format!(
                            "Entre deux emplois depuis {} ; dernier poste : {} chez {}.",
                            j.end_year.unwrap_or(j.start_year),
                            j.title,
                            j.employer
                        )
                    } else {
                        format!("Between jobs since {}; last role: {} at {}.", j.end_year.unwrap_or(j.start_year), j.title, j.employer)
                    };
                } else {
                    s += &if fr {
                        format!(
                            "{il} travaille comme {} chez {} depuis {} ({}).",
                            lc_first(&j.title),
                            j.employer,
                            j.start_year,
                            label("schedule", &o.schedule, lang)
                        )
                    } else {
                        format!(
                            "{il} has worked as {} at {} since {} ({}).",
                            lc_first(&j.title),
                            j.employer,
                            j.start_year,
                            label("schedule", &o.schedule, lang)
                        )
                    };
                    if main_jobs.len() > 1 {
                        let prev: Vec<String> =
                            main_jobs[..main_jobs.len() - 1].iter().map(|j| format!("{} ({})", j.employer, j.start_year)).collect();
                        s += &if fr { format!(" Avant : {}.", list(&prev, lang)) } else { format!(" Before that: {}.", list(&prev, lang)) };
                    }
                }
            }
        }
    }
    paras.push(s);

    // 3 — family.
    let fam = &p.family;
    let mut s = String::new();
    let rel = label("relationship", &fam.relationship, lang);
    match &fam.partner {
        Some(pt) if fam.relationship == "widowed" => {
            s += &if fr {
                format!("{il} a perdu {} ({}). ", pt.first_name, rel)
            } else {
                format!("{il} lost {} ({}). ", pt.first_name, rel)
            };
        }
        Some(pt) => {
            s += &if fr {
                format!("{} : {} ({} ans). ", crate::text::capitalize(&rel), pt.first_name, p.recipe.as_of.year() - pt.birth_year)
            } else {
                format!("{}: {} ({}). ", crate::text::capitalize(&rel), pt.first_name, p.recipe.as_of.year() - pt.birth_year)
            };
        }
        None => s += &format!("{}. ", crate::text::capitalize(&rel)),
    }
    if !fam.children.is_empty() {
        let kids: Vec<String> =
            fam.children.iter().map(|c| format!("{} ({})", c.first_name, p.recipe.as_of.year() - c.birth_year)).collect();
        s += &if fr { format!("Enfants : {}. ", list(&kids, lang)) } else { format!("Children: {}. ", list(&kids, lang)) };
    }
    if fam.grandchildren > 0 {
        s += &if fr { format!("{} petits-enfants. ", fam.grandchildren) } else { format!("{} grandchildren. ", fam.grandchildren) };
    }
    if !fam.siblings.is_empty() {
        let sib: Vec<String> = fam
            .siblings
            .iter()
            .map(|x| format!("{} {}", label("sibling", if x.gender.is_female() { "sister" } else { "brother" }, lang), x.first_name))
            .collect();
        s += &if fr { format!("Fratrie : {}. ", list(&sib, lang)) } else { format!("Siblings: {}. ", list(&sib, lang)) };
    }
    if !fam.pets.is_empty() {
        let pets: Vec<String> = fam.pets.iter().map(|x| format!("{} ({})", x.name, label("species", &x.species, lang))).collect();
        s += &if fr { format!("À la maison : {}.", list(&pets, lang)) } else { format!("At home: {}.", list(&pets, lang)) };
    }
    paras.push(s.trim().to_string());

    // 4 — character.
    let per = &p.personality;
    let b = &p.behavior;
    let values: Vec<String> = per.values.iter().map(|v| v.get(lang).to_string()).collect();
    let quirk = per.quirks.first().map(|q| q.get(lang).to_string()).unwrap_or_default();
    let hobbies: Vec<String> = p.interests.iter().map(|i| i.label.get(lang).to_string()).collect();
    paras.push(if fr {
        format!(
            "Profil « {} ». Plutôt {}, {} se lève vers {} et se couche vers {}. Ce qui compte pour {lui} : {}. Loisirs : {}. Petite manie : {}.",
            label("ptype", &per.type_key, lang),
            label("chronotype", &b.chronotype, lang),
            id.first_name,
            b.wake,
            b.sleep,
            list(&values, lang),
            list(&hobbies, lang),
            quirk
        )
    } else {
        format!(
            "'{}' type. Chronotype: {}. {} gets up around {} and goes to bed around {}. What matters to {lui}: {}. Hobbies: {}. Quirk: {}.",
            label("ptype", &per.type_key, lang),
            label("chronotype", &b.chronotype, lang),
            id.first_name,
            b.wake,
            b.sleep,
            list(&values, lang),
            list(&hobbies, lang),
            quirk
        )
    });

    // 5 — online.
    let plats: Vec<String> = p.accounts.iter().map(|a| a.platform_name.clone()).collect();
    let w = &b.writing;
    let habits: Vec<String> = w.habits.iter().map(|h| label("habit", h, lang)).collect();
    paras.push(if fr {
        format!(
            "En ligne : {}. Environ {:.1} publications par semaine, depuis un {}. Registre {}, {} ; {}.",
            list(&plats, lang),
            b.posts_per_week,
            b.phone,
            label("register", &w.register, lang),
            label("casing", &w.casing, lang),
            list(&habits, lang)
        )
    } else {
        format!(
            "Online: {}. About {:.1} posts a week, from {} {}. {} register, {}; {}.",
            list(&plats, lang),
            b.posts_per_week,
            if f { "her" } else { "his" },
            b.phone,
            crate::text::capitalize(&label("register", &w.register, lang)),
            label("casing", &w.casing, lang),
            list(&habits, lang)
        )
    });
    let _ = il;
    paras.join("\n\n")
}

fn legend(p: &Persona) -> Legend {
    let id = &p.identity;
    let r = &p.residence;
    let loc = &r.local;
    let o = &p.occupation;
    let h1 = p.interests.first().map(|i| i.label.clone()).unwrap_or_default();
    let h2 = p.interests.get(1).map(|i| i.label.clone()).unwrap_or_else(|| h1.clone());
    let job_fr = match o.status.as_str() {
        "student" => format!(
            "étudiant{} à {}",
            if id.gender.is_female() { "e" } else { "" },
            p.education.last().map(|e| e.institution.as_str()).unwrap_or("")
        ),
        "retired" => format!(
            "à la retraite depuis peu, après une carrière de {}",
            p.career.iter().rev().find(|j| !j.side_job).map(|j| j.title.as_str()).unwrap_or("salarié")
        ),
        "between_jobs" => "entre deux boulots en ce moment".to_string(),
        _ => format!("{} chez {}", lc_first(o.title.as_deref().unwrap_or("")), o.employer.as_deref().unwrap_or("")),
    };
    let job_en = match o.status.as_str() {
        "student" => format!("a student at {}", p.education.last().map(|e| e.institution.as_str()).unwrap_or("")),
        "retired" => format!(
            "recently retired after a career as {}",
            p.career.iter().rev().find(|j| !j.side_job).map(|j| j.title.as_str()).unwrap_or("an employee")
        ),
        "between_jobs" => "between jobs at the moment".to_string(),
        _ => format!("{} at {}", lc_first(o.title.as_deref().unwrap_or("")), o.employer.as_deref().unwrap_or("")),
    };
    let pitch = L10n::new(
        format!(
            "Moi c'est {}, j'ai {} ans. Je suis {} et j'habite à {}, {}. Mon truc, c'est {} — et un peu de {} quand j'ai le temps.",
            id.nickname.as_deref().unwrap_or(&id.first_name),
            id.age,
            job_fr,
            r.place.city,
            prep_in("fr", &r.district),
            h1.fr,
            h2.fr
        ),
        format!(
            "I'm {}, {}. I'm {} and I live in {}, {}. I'm mostly into {} — and a bit of {} when I find the time.",
            id.nickname.as_deref().unwrap_or(&id.first_name),
            id.age,
            job_en,
            r.place.city,
            prep_in("en", &r.district),
            h1.en,
            h2.en
        ),
    );

    let mut talking = vec![
        L10n::new(
            format!("Équipe locale : {} — connais le dernier résultat avant d'en parler.", loc.team),
            format!("Local team: {} — know the latest result before bringing it up.", loc.team),
        ),
        L10n::new(format!("Repère que tout le monde connaît : {}.", loc.landmark), format!("Landmark everyone knows: {}.", loc.landmark)),
        L10n::new(format!("Spécialité locale : {}.", loc.specialty), format!("Local speciality: {}.", loc.specialty)),
        L10n::new(format!("Météo typique : {}.", loc.weather), format!("Typical weather: {}.", loc.weather)),
        L10n::new(
            format!("Habitant·e de {} : « {} ».", r.place.city, loc.demonym),
            format!("People from {} are '{}'.", r.place.city, loc.demonym),
        ),
    ];
    if !loc.other_districts.is_empty() {
        let d = loc.other_districts.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
        talking.push(L10n::new(
            format!("Quartiers voisins à savoir situer : {d}."),
            format!("Nearby areas you should be able to place: {d}."),
        ));
    }
    if let Some(i) = p.interests.first() {
        talking.push(L10n::new(
            format!(
                "{} depuis {} : prépare trois anecdotes concrètes (lieu, date, détail).",
                crate::text::capitalize(&i.label.fr),
                i.since_year
            ),
            format!(
                "{} since {}: prepare three concrete anecdotes (place, date, detail).",
                crate::text::capitalize(&i.label.en),
                i.since_year
            ),
        ));
    }

    let mut weak = Vec::new();
    if id.birthplace.city != r.place.city {
        weak.push(L10n::new(
            format!("Tu as grandi à {} : connais au moins ton lycée, un quartier et un plat local.", id.birthplace.city),
            format!("You grew up in {}: know at least your school, one neighbourhood and one local dish.", id.birthplace.city),
        ));
    }
    if let Some(t) = &o.title {
        if o.status == "working" {
            weak.push(L10n::new(
                format!("Ton métier ({t}) peut être testé par un vrai professionnel : vocabulaire, horaires, salaire typique."),
                format!("Your job ({t}) can be tested by a real professional: jargon, hours, typical pay."),
            ));
        }
    }
    for l in id.languages.iter().filter(|l| l.level == "fluent") {
        let name = l10n("language", &l.code);
        weak.push(L10n::new(
            format!("La légende parle {} couramment : évite les échanges dans cette langue si ce n'est pas ton cas.", name.fr),
            format!("The legend is fluent in {}: avoid conversations in it unless you are too.", name.en),
        ));
    }
    if !p.family.children.is_empty() {
        weak.push(L10n::new(
            format!("{} enfant(s) : retiens prénoms, âges et activités.", p.family.children.len()),
            format!("{} child(ren): memorise names, ages and activities.", p.family.children.len()),
        ));
    }
    for a in p.accounts.iter().filter(|a| a.join_date_public && a.claimed_since < p.recipe.as_of.year() - 1) {
        weak.push(L10n::new(
            format!("{} affiche la date de création du compte alors que la légende dit « depuis {} » : prévois l'explication d'un compte récent.", a.platform_name, a.claimed_since),
            format!("{} shows the account creation date while the legend says 'since {}': have a reason ready for a fresh account.", a.platform_name, a.claimed_since),
        ));
    }
    weak.push(L10n::new(
        format!("Fuseau {} : tes publications doivent tomber aux heures locales de {}.", p.behavior.timezone, r.place.city),
        format!("Timezone {}: your posts should land at {} local hours.", p.behavior.timezone, r.place.city),
    ));

    let opsec = vec![
        L10n::new(
            "Environnement dédié (VM ou profil de navigateur isolé) pour cette légende, jamais ton appareil personnel.",
            "Dedicated environment (VM or isolated browser profile) for this legend, never your personal device.",
        ),
        L10n::new(
            "Ne te connecte jamais à un compte personnel depuis l'environnement de la légende, ni l'inverse.",
            "Never log into a personal account from the legend's environment, or vice versa.",
        ),
        L10n::new(
            "Une adresse e-mail et un numéro dédiés par légende ; rien de partagé entre légendes.",
            "One dedicated email address and phone number per legend; nothing shared between legends.",
        ),
        L10n::new(
            format!("Langue système, clavier et fuseau horaire cohérents avec {}.", r.place.city),
            format!("System language, keyboard layout and timezone consistent with {}.", r.place.city),
        ),
        L10n::new(
            "Aucune interaction avec tes vrais contacts ni avec tes autres légendes (voir l'onglet Contamination).",
            "No interaction with your real contacts or your other legends (see the Contamination tab).",
        ),
        L10n::new(
            "Reste dans le cadre légal de ta mission et les CGU des plateformes ; consigne chaque action dans le journal.",
            "Stay within the legal scope of your engagement and platform terms; log every action in the journal.",
        ),
    ];

    Legend { pitch, talking_points: talking, weak_spots: weak, opsec }
}
