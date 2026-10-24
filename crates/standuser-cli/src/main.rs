//! standuser export.zip -> card.svg and card.png

use standuser_core::{analysis, dates, lookup::Lookup, parse, svg};
use std::path::PathBuf;
use std::process::ExitCode;

const TABLE: &str = include_str!("../../standuser-core/data/films.json");

fn usage() -> ExitCode {
    eprintln!(
        "usage: standuser <letterboxd-export.zip> [options]\n\n\
         options:\n  \
         --out <dir>        where to write card.svg, card.png and stand.json (default: current directory)\n  \
         --table <file>     film lookup table JSON (default: the one built into this binary)\n  \
         --theme <t>        light | dark (png); the svg is always light with a dark-mode override (default: light)\n  \
         --joined <date>    YYYY-MM-DD, when the zip has no profile.csv\n  \
         --username <name>  override the username\n  \
         --json             print the analysis as JSON instead of the summary"
    );
    ExitCode::from(2)
}

fn bad(msg: String) -> ExitCode {
    eprintln!("standuser: {msg}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut input: Option<String> = None;
    let (mut out, mut table, mut theme, mut joined, mut user, mut json) = (PathBuf::from("."), None::<String>, "light".to_string(), None::<String>, None::<String>, false);
    while let Some(a) = args.next() {
        // every option that takes a value must get one; a bad command line stops here with exit code 2
        macro_rules! value {
            () => {
                match args.next() {
                    Some(v) if !v.starts_with("--") => v,
                    _ => return bad(format!("{a} needs a value")),
                }
            };
        }
        match a.as_str() {
            "--out" => out = PathBuf::from(value!()),
            "--table" => table = Some(value!()),
            "--theme" => {
                theme = value!();
                if theme != "light" && theme != "dark" {
                    return bad(format!("--theme must be light or dark, not {theme:?}"));
                }
            }
            "--joined" => joined = Some(value!()),
            "--username" => user = Some(value!()),
            "--json" => json = true,
            "-h" | "--help" => return usage(),
            x if x.starts_with("--") => {
                eprintln!("unknown option {x}");
                return usage();
            }
            x => {
                if input.is_some() {
                    return bad(format!("only one export ZIP can be given (got {x:?} after {:?})", input.unwrap()));
                }
                input = Some(x.to_string())
            }
        }
    }
    let Some(input) = input else { return usage() };

    let bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot read {input}: {e}");
            return ExitCode::from(1);
        }
    };
    let mut ex = match parse::parse_zip(&bytes, Some(&input)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if let Some(j) = joined {
        match dates::parse_date(&j) {
            Some(d) => ex.joined = Some(d),
            None => {
                eprintln!("--joined must look like 2022-07-06");
                return ExitCode::from(2);
            }
        }
    }
    if let Some(u) = user {
        ex.username = Some(u);
    }
    let table_json = match table {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("cannot read {p}: {e}");
                return ExitCode::from(1);
            }
        },
        None => TABLE.to_string(),
    };
    let lk = match Lookup::from_json(&table_json) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| (d.as_secs() / 86400) as i64);
    let a = analysis::analyze_at(&ex, &lk, today);

    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("cannot create {}: {e}", out.display());
        return ExitCode::from(1);
    }
    let svg_auto = svg::card_svg(&a.card, svg::Theme::Auto);
    let png_theme = if theme == "dark" { svg::Theme::Dark } else { svg::Theme::Light };
    let svg_png = svg::card_svg(&a.card, png_theme);
    let png = match render_png(&svg_png) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("png: {e}");
            return ExitCode::from(1);
        }
    };
    let write = |name: &str, data: &[u8]| std::fs::write(out.join(name), data).map_err(|e| eprintln!("cannot write {name}: {e}"));
    if write("card.svg", svg_auto.as_bytes()).is_err() || write("card.png", &png).is_err() || write("stand.json", serde_json::to_string_pretty(&a).unwrap().as_bytes()).is_err() {
        return ExitCode::from(1);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&a).unwrap());
    } else {
        let g: Vec<String> = a.card.grades.iter().zip(&a.card.prov).map(|(l, p)| format!("{l}{}", if *p { "*" } else { "" })).collect();
        println!("「{}」  {}  ({})", a.card.name, a.card.type_en, a.username);
        println!("Power Speed Range Stamina Precision Potential: {}", g.join(" "));
        println!("{}", a.card.ability);
        println!("Nearest canon: {}", a.card.nearest.join(" · "));
        for n in &a.notes {
            println!("note: {n}");
        }
        println!("wrote {}, {}, {}", out.join("card.svg").display(), out.join("card.png").display(), out.join("stand.json").display());
    }
    ExitCode::SUCCESS
}

fn render_png(svg: &str) -> Result<Vec<u8>, String> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pm = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty canvas")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pm.as_mut());
    pm.encode_png().map_err(|e| e.to_string())
}
