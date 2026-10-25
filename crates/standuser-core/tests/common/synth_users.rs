//! Seeded synthetic users built from the real film table: ratings, favourites and rewatches, no real person's data.
use standuser_core::lookup::Lookup;
use standuser_core::parse::{DiaryEntry, Export, Rating, Watched};
use standuser_core::rng::SplitMix64;

/// `n` users: (export, ability template 1..=17). Films come from the table, 70% of them from films that have name candidates.
pub fn users(n: usize, lk: &Lookup) -> Vec<(Export, u8)> {
    let with: Vec<usize> = (0..lk.films.len()).filter(|&i| lk.names.contains_key(&lk.films[i].tmdb_id)).collect();
    let all: Vec<usize> = (0..lk.films.len()).collect();
    let mut out = vec![];
    for u in 0..n {
        let mut r = SplitMix64::new(0x5EED_0000 + u as u64);
        let mut ex = Export::default();
        ex.username = Some(format!("synthetic{u}"));
        let count = 40 + r.below(160) as usize;
        let mut seen = std::collections::HashSet::new();
        let mut rated_codes = vec![];
        for k in 0..count {
            let pool = if r.below(10) < 7 { &with } else { &all };
            let fi = pool[r.below(pool.len() as u64) as usize];
            if !seen.insert(fi) {
                continue;
            }
            let f = &lk.films[fi];
            let name = f.key.split('|').next().unwrap().to_string();
            let uri = format!("https://boxd.it/s{u}x{k}");
            let day = 19000 + (k as i64) * 3;
            ex.watched.push(Watched { day, name: name.clone(), year: f.year, uri: uri.clone() });
            if r.below(10) < 7 {
                let rating = 0.5 + 0.5 * r.below(10) as f64;
                ex.ratings.push(Rating { name: name.clone(), year: f.year, rating, uri: uri.clone() });
                if rating >= 4.5 {
                    rated_codes.push(format!("s{u}x{k}"));
                }
            }
            if r.below(10) < 2 {
                ex.diary.push(DiaryEntry { log_day: day, name, year: f.year, view_day: day, rewatch: r.below(2) == 0 });
            }
        }
        ex.favorites = rated_codes.into_iter().take(4).collect();
        out.push((ex, 1 + (u % 17) as u8));
    }
    out
}
