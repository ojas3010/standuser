//! standuser-core: Letterboxd export -> JoJo Stand card.
//!
//! parse -> timeline -> stats -> grading -> type -> match -> ability -> name -> svg
//!
//! ```no_run
//! # fn run(zip: &[u8], table_json: &str) -> Result<(), Box<dyn std::error::Error>> {
//! let ex = standuser_core::parse::parse_zip(zip, Some("letterboxd-user-2026-10-19-13-22-utc.zip"))?;
//! let table = standuser_core::lookup::Lookup::from_json(table_json)?;
//! let a = standuser_core::analysis::analyze(&ex, &table);
//! let svg = standuser_core::svg::card_svg(&a.card, standuser_core::svg::Theme::Auto);
//! # Ok(()) }
//! ```

pub mod ability;
pub mod analysis;
pub mod dates;
pub mod font;
pub mod grading;
pub mod lookup;
pub mod matching;
pub mod name;
pub mod palette;
pub mod parse;
pub mod pool;
pub mod rng;
pub mod share;
pub mod stats;
pub mod svg;
pub mod timeline;
pub mod types;
