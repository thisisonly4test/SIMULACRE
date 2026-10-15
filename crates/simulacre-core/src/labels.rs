//! Display labels for every enumerated key the persona carries. One source of
//! truth: the Rust narrative uses it and `/api/meta` ships it to the UI.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::UiLang;
use crate::data::L10n;

type Group = BTreeMap<&'static str, L10n>;
/// (group, [(key, fr, en)])
type Table = &'static [(&'static str, &'static [(&'static str, &'static str, &'static str)])];

const TABLE: Table = &[
    (
        "status",
        &[
            ("student", "étudiant·e", "student"),
            ("working", "en activité", "employed"),
            ("retired", "retraité·e", "retired"),
            ("between_jobs", "entre deux emplois", "between jobs"),
        ],
    ),
    (
        "schedule",
        &[
            ("office", "horaires de bureau", "office hours"),
            ("shifts", "travail posté / en horaires décalés", "shift work"),
            ("early", "journées qui commencent tôt", "early starts"),
            ("flexible", "horaires flexibles", "flexible hours"),
            ("classes", "rythme des cours", "class timetable"),
            ("free", "emploi du temps libre", "free schedule"),
        ],
    ),
    (
        "relationship",
        &[
            ("single", "célibataire", "single"),
            ("dating", "en couple depuis peu", "dating"),
            ("partnered", "en couple", "in a relationship"),
            ("married", "marié·e", "married"),
            ("divorced", "divorcé·e", "divorced"),
            ("widowed", "veuf·ve", "widowed"),
        ],
    ),
    (
        "build",
        &[
            ("slim", "mince", "slim"),
            ("average", "corpulence moyenne", "average build"),
            ("athletic", "sportive", "athletic"),
            ("stocky", "trapue", "stocky"),
            ("heavy", "forte", "heavy-set"),
        ],
    ),
    (
        "eyes",
        &[
            ("brown", "marron", "brown"),
            ("hazel", "noisette", "hazel"),
            ("blue", "bleus", "blue"),
            ("green", "verts", "green"),
            ("grey", "gris", "grey"),
        ],
    ),
    (
        "hair_color",
        &[
            ("black", "noirs", "black"),
            ("dark_brown", "brun foncé", "dark brown"),
            ("light_brown", "châtains", "light brown"),
            ("blond", "blonds", "blond"),
            ("red", "roux", "red"),
            ("grey", "gris", "grey"),
            ("white", "blancs", "white"),
        ],
    ),
    (
        "hair_style",
        &[
            ("short", "courts", "short"),
            ("medium", "mi-longs", "medium-length"),
            ("long", "longs", "long"),
            ("curly", "bouclés", "curly"),
            ("buzz", "rasés de près", "buzz cut"),
            ("bald", "crâne dégarni", "balding"),
            ("bun", "attachés en chignon", "in a bun"),
        ],
    ),
    (
        "marks",
        &[
            ("freckles", "taches de rousseur", "freckles"),
            ("beard", "barbe", "beard"),
            ("tattoo_forearm", "tatouage sur l'avant-bras", "forearm tattoo"),
            ("tattoo_small", "petit tatouage discret", "small discreet tattoo"),
            ("piercing", "piercing à l'oreille", "ear piercing"),
            ("scar_eyebrow", "petite cicatrice au sourcil", "small eyebrow scar"),
            ("moustache", "moustache", "moustache"),
        ],
    ),
    (
        "tenure",
        &[
            ("renter", "locataire", "renting"),
            ("owner", "propriétaire", "owner"),
            ("family", "chez la famille", "living with family"),
            ("student_housing", "logement étudiant", "student housing"),
            ("flatshare", "colocation", "flatshare"),
        ],
    ),
    (
        "chronotype",
        &[("lark", "du matin", "early bird"), ("intermediate", "intermédiaire", "intermediate"), ("owl", "du soir", "night owl")],
    ),
    ("activity", &[("lurker", "lit sans poster", "lurker"), ("casual", "occasionnel", "casual"), ("active", "actif", "active")]),
    ("depth", &[("casual", "occasionnel", "casual"), ("regular", "régulier", "regular"), ("devoted", "passionné", "devoted")]),
    ("register", &[("formal", "soutenu", "formal"), ("neutral", "neutre", "neutral"), ("casual", "familier", "casual")]),
    (
        "casing",
        &[
            ("standard", "majuscules standard", "standard capitalisation"),
            ("lowercase", "tout en minuscules", "all lowercase"),
            ("expressive", "MAJUSCULES pour insister", "CAPS for emphasis"),
        ],
    ),
    (
        "habit",
        &[
            ("ellipsis", "abuse des points de suspension", "overuses ellipses"),
            ("multi_exclaim", "multiplie les points d'exclamation", "stacks exclamation marks"),
            ("no_final_period", "ne met jamais de point final", "never ends with a full stop"),
            ("haha", "ponctue avec « haha »", "punctuates with 'haha'"),
            ("signoff", "signe ses messages", "signs off messages"),
            ("hashtags", "ajoute des hashtags", "adds hashtags"),
            ("short_lines", "phrases très courtes", "very short sentences"),
        ],
    ),
    (
        "zodiac",
        &[
            ("capricorn", "Capricorne", "Capricorn"),
            ("aquarius", "Verseau", "Aquarius"),
            ("pisces", "Poissons", "Pisces"),
            ("aries", "Bélier", "Aries"),
            ("taurus", "Taureau", "Taurus"),
            ("gemini", "Gémeaux", "Gemini"),
            ("cancer", "Cancer", "Cancer"),
            ("leo", "Lion", "Leo"),
            ("virgo", "Vierge", "Virgo"),
            ("libra", "Balance", "Libra"),
            ("scorpio", "Scorpion", "Scorpio"),
            ("sagittarius", "Sagittaire", "Sagittarius"),
        ],
    ),
    (
        "ptype",
        &[
            ("connector", "Le liant", "The connector"),
            ("organizer", "L'organisateur", "The organiser"),
            ("explorer", "L'explorateur", "The explorer"),
            ("observer", "L'observateur", "The observer"),
            ("anchor", "Le roc", "The anchor"),
            ("worrier", "L'inquiet", "The worrier"),
            ("maverick", "L'électron libre", "The maverick"),
            ("helper", "Le bienveillant", "The helper"),
        ],
    ),
    (
        "ocean",
        &[
            ("o", "Ouverture", "Openness"),
            ("c", "Conscienciosité", "Conscientiousness"),
            ("e", "Extraversion", "Extraversion"),
            ("a", "Agréabilité", "Agreeableness"),
            ("n", "Névrosisme", "Neuroticism"),
        ],
    ),
    (
        "language",
        &[
            ("fr", "français", "French"),
            ("en", "anglais", "English"),
            ("de", "allemand", "German"),
            ("es", "espagnol", "Spanish"),
            ("it", "italien", "Italian"),
            ("pt", "portugais", "Portuguese"),
            ("ar", "arabe", "Arabic"),
            ("tr", "turc", "Turkish"),
            ("vi", "vietnamien", "Vietnamese"),
            ("pl", "polonais", "Polish"),
            ("ur", "ourdou", "Urdu"),
            ("gu", "gujarati", "Gujarati"),
        ],
    ),
    (
        "lang_level",
        &[
            ("native", "langue maternelle", "native"),
            ("fluent", "courant", "fluent"),
            ("intermediate", "intermédiaire", "intermediate"),
            ("basic", "notions", "basic"),
        ],
    ),
    ("species", &[("dog", "chien", "dog"), ("cat", "chat", "cat")]),
    (
        "topic",
        &[
            ("hobby", "loisir", "hobby"),
            ("city", "ville", "city"),
            ("food", "cuisine", "food"),
            ("team", "sport", "sport"),
            ("work", "travail", "work"),
            ("pet", "animal", "pet"),
            ("family", "famille", "family"),
            ("mood", "humeur", "mood"),
        ],
    ),
    (
        "edu",
        &[
            ("secondary", "secondaire", "secondary"),
            ("vocational", "professionnel", "vocational"),
            ("bachelor", "licence / bachelor", "bachelor"),
            ("master", "master", "master"),
            ("doctorate", "doctorat", "doctorate"),
        ],
    ),
    ("sibling", &[("brother", "frère", "brother"), ("sister", "sœur", "sister")]),
    (
        "weekday",
        &[
            ("0", "lundi", "Monday"),
            ("1", "mardi", "Tuesday"),
            ("2", "mercredi", "Wednesday"),
            ("3", "jeudi", "Thursday"),
            ("4", "vendredi", "Friday"),
            ("5", "samedi", "Saturday"),
            ("6", "dimanche", "Sunday"),
        ],
    ),
];

pub fn all() -> &'static BTreeMap<&'static str, Group> {
    static ALL: OnceLock<BTreeMap<&'static str, Group>> = OnceLock::new();
    ALL.get_or_init(|| {
        TABLE.iter().map(|(group, rows)| (*group, rows.iter().map(|(k, fr, en)| (*k, L10n::new(*fr, *en))).collect())).collect()
    })
}

/// Label for `group/key`, falling back to the key itself so a missing entry is
/// visible rather than silent.
pub fn label(group: &str, key: &str, lang: UiLang) -> String {
    all().get(group).and_then(|g| g.get(key)).map(|l| l.get(lang).to_string()).unwrap_or_else(|| key.to_string())
}

pub fn l10n(group: &str, key: &str) -> L10n {
    all().get(group).and_then(|g| g.get(key)).cloned().unwrap_or_else(|| L10n::new(key, key))
}
