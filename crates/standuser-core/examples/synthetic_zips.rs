//! Writes export ZIPs for the first N seeded synthetic users (real film table, synthetic histories) so the CLI can render their cards.
//!   cargo run -p standuser-core --example synthetic_zips -- <out_dir> [n]
#[path = "../tests/common/synth_users.rs"]
mod synth_users;

use standuser_core::{dates::iso, lookup::Lookup};
use std::io::Write;

fn main() {
    let mut a = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(a.next().expect("out dir"));
    let n: usize = a.next().and_then(|x| x.parse().ok()).unwrap_or(3);
    std::fs::create_dir_all(&dir).unwrap();
    let mut lk = Lookup::from_json(include_str!("../data/films.json")).unwrap();
    lk.load_names(include_str!("../data/names.json")).unwrap();
    for (i, (ex, _)) in synth_users::users(n, &lk).into_iter().enumerate() {
        let user = ex.username.clone().unwrap();
        let mut watched = String::from("Date,Name,Year,Letterboxd URI\n");
        for w in &ex.watched {
            watched += &format!("{},{},{},{}\n", iso(w.day), w.name, w.year, w.uri);
        }
        let mut ratings = String::from("Date,Name,Year,Letterboxd URI,Rating\n");
        for r in &ex.ratings {
            ratings += &format!("2026-10-01,{},{},{},{}\n", r.name, r.year, r.uri, r.rating);
        }
        let mut diary = String::from("Date,Name,Year,Letterboxd URI,Rating,Rewatch,Tags,Watched Date\n");
        for d in &ex.diary {
            diary += &format!("{},{},{},u,,{},,{}\n", iso(d.log_day), d.name, d.year, if d.rewatch { "Yes" } else { "" }, iso(d.view_day));
        }
        let favs: Vec<String> = ex.favorites.iter().map(|c| format!("https://boxd.it/{c}")).collect();
        let profile = format!("Date Joined,Username,Favorite Films\n2022-01-01,{user},\"{}\"\n", favs.join(", "));
        let path = dir.join(format!("letterboxd-{user}-2026-10-19-13-22-utc.zip"));
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, body) in [("watched.csv", &watched), ("ratings.csv", &ratings), ("diary.csv", &diary), ("profile.csv", &profile)] {
            z.start_file(name, opt).unwrap();
            z.write_all(body.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        println!("{}", path.display());
    }
}
