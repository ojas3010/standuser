//! The best candidates of a real export with their lexicon matches, as JSON on stdout (read by tools/naming/report_top.py).
//!   cargo run -p standuser-core --example name_report -- <export.zip> [n]
use standuser_core::{analysis, lookup::Lookup, parse, pool};

fn main() {
    let mut a = std::env::args().skip(1);
    let zip = a.next().expect("export zip");
    let n: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(10);
    let bytes = std::fs::read(&zip).unwrap();
    let ex = parse::parse_zip(&bytes, Some(&zip)).unwrap();
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    let an = analysis::analyze(&ex, &lk);
    let t = an.ability.template.unwrap_or(17);
    let p = pool::pick_top(&ex, &lk, t, an.username.as_str(), n).expect("no pool");
    println!("{}", serde_json::json!({ "template": t, "favorites_read": ex.favorites.len(), "pool_films": p.pool_films, "median_cool": p.median_cool, "top": p.top, "first_track": p.first_track }));
}
