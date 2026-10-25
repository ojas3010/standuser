//! Prints the five best name candidates (S, Cool, F, A, source track) for ten seeded synthetic users on the real film table and
//! name candidates (each user's ability template is the one the analysis gives, so the pick is the card's), with the name the first-track rule would pick next to the S winner.  cargo run -p standuser-core --example synthetic_users
#[path = "../tests/common/synth_users.rs"]
mod synth_users;

use standuser_core::{analysis, dates::parse_date, lookup::Lookup, pool};

fn main() {
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    for (i, (mut ex, _)) in synth_users::users(10, &lk).into_iter().enumerate() {
        // the same inputs the CLI sees for the zips written by synthetic_zips: join day and export date as in those files,
        // and the ability template the analysis really gives this history (not an assigned one)
        ex.joined = parse_date("2022-01-01");
        ex.as_of = parse_date("2026-10-19");
        let t = analysis::analyze(&ex, &lk).ability.template.unwrap_or(17);
        let name = ex.username.clone().unwrap();
        let tpl = standuser_core::ability::template_by_id(t).name;
        match pool::pick(&ex, &lk, t, &name) {
            None => println!("user {i} ({name}, template {t} {tpl}): no pool"),
            Some(p) => {
                println!("user {i} ({name}, {} films, template {t} {tpl}): pool {} films, {} candidates, median Cool {:.2}", ex.watched.len(), p.pool_films, p.pool_cands, p.median_cool);
                for (k, s) in p.top.iter().enumerate() {
                    println!("  {}{} {:<26} S {:.4}  Cool {:.2}  F {:.1}  A {:.3}  track {} of {} ({})", k + 1, if s.picked { "*" } else { " " }, s.name, s.s, s.cool, s.f, s.a, s.track, s.film, s.year);
                }
                if let Some(f) = &p.first_track {
                    println!("  first-track: {} (track {} of {});  S winner: {}", f.name, f.track, f.film, p.name);
                }
            }
        }
    }
}
