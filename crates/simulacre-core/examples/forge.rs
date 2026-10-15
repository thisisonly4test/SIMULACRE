//! Print a legend file from the command line:
//! `cargo run -p simulacre-core --example forge -- [SEED] [LOCALE] [ARCHETYPE] [fr|en]`

use simulacre_core::{Recipe, UiLang, export, generate, rng};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed = args.first().cloned().unwrap_or_else(rng::random_seed);
    let mut recipe = Recipe::new(seed, chrono::Local::now().date_naive());
    if let Some(l) = args.get(1) {
        recipe.locale = l.clone();
    }
    recipe.archetype = args.get(2).filter(|a| *a != "-").cloned();
    let lang = args.get(3).and_then(|l| l.parse().ok()).unwrap_or(UiLang::Fr);
    match generate(&recipe) {
        Ok(p) => print!("{}", export::markdown(&p, lang)),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
