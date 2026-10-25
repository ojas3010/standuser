use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> (PathBuf, PathBuf) {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../standuser-core/tests/fixtures/synth");
    (d.join("letterboxd-synthuser-2026-06-30-12-00-utc.zip"), d.join("table.json"))
}

fn out_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("standuser-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

#[test]
fn writes_svg_png_and_json() {
    let (zip, table) = fixture();
    let out = out_dir("ok");
    let r = Command::new(env!("CARGO_BIN_EXE_standuser")).arg(&zip).args(["--table"]).arg(&table).args(["--out"]).arg(&out).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let svg = std::fs::read_to_string(out.join("card.svg")).unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("class=\"f-ink\"") && svg.contains("prefers-color-scheme"));
    let png = std::fs::read(out.join("card.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 1080);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1350);
    let j: serde_json::Value = serde_json::from_slice(&std::fs::read(out.join("stand.json")).unwrap()).unwrap();
    assert_eq!(j["username"], "synthuser");
    // the join day is read from the zip's profile.csv; no other profile field reaches any output
    assert_eq!(j["joined"], "2023-01-05");
    let everything = [std::fs::read(out.join("card.svg")).unwrap(), std::fs::read(out.join("stand.json")).unwrap(), r.stdout.clone(), r.stderr.clone()].concat();
    let everything = String::from_utf8_lossy(&everything);
    for field in ["synthetic@example.invalid", "Thetic"] {
        assert!(!everything.contains(field), "profile field {field} leaked");
    }
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("synthuser") && stdout.contains("Nearest canon"));
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn dark_png_differs_from_light() {
    let (zip, table) = fixture();
    let (a, b) = (out_dir("l"), out_dir("d"));
    for (o, t) in [(&a, "light"), (&b, "dark")] {
        let r = Command::new(env!("CARGO_BIN_EXE_standuser")).arg(&zip).args(["--table"]).arg(&table).args(["--out"]).arg(o).args(["--theme", t]).output().unwrap();
        assert!(r.status.success());
    }
    assert_ne!(std::fs::read(a.join("card.png")).unwrap(), std::fs::read(b.join("card.png")).unwrap());
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn friendly_errors() {
    let bin = env!("CARGO_BIN_EXE_standuser");
    let r = Command::new(bin).output().unwrap();
    assert_eq!(r.status.code(), Some(2));
    let r = Command::new(bin).arg("/nonexistent/export.zip").output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("cannot read"));
    let bad = std::env::temp_dir().join(format!("standuser-bad-{}.zip", std::process::id()));
    std::fs::write(&bad, b"not a zip").unwrap();
    let r = Command::new(bin).arg(&bad).output().unwrap();
    assert_eq!(r.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&r.stderr).contains("ZIP"));
    let _ = std::fs::remove_file(&bad);
}

/// A bad command line must fail (exit 2) and never carry on with defaults.
#[test]
fn bad_options_exit_nonzero_without_writing_anything() {
    let (zip, _) = fixture();
    let bin = env!("CARGO_BIN_EXE_standuser");
    let out = out_dir("badopts");
    for args in [
        vec!["--table"],
        vec!["--joined"],
        vec!["--username"],
        vec!["--out"],
        vec!["--theme"],
        vec!["--table", "--json"],
        vec!["--theme", "purple"],
        vec!["--joined", "yesterday"],
        vec!["--bogus"],
    ] {
        let r = Command::new(bin).arg(&zip).args(&args).args(["--out"]).arg(&out).output().unwrap();
        // "--out" is appended after the bad option, so for `--out` alone the bad option swallows nothing and fails on its own
        let r = if args == vec!["--out"] { Command::new(bin).arg(&zip).arg("--out").output().unwrap() } else { r };
        assert_eq!(r.status.code(), Some(2), "{args:?}: stdout {:?} stderr {:?}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
        assert!(!out.join("card.svg").exists(), "{args:?} still wrote a card");
    }
    // two ZIPs are rejected instead of one being ignored
    let r = Command::new(bin).arg(&zip).arg(&zip).output().unwrap();
    assert_eq!(r.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn name_table_prints_the_five_best_candidates_and_the_favourites_flag_changes_the_affinity() {
    let (zip, table) = fixture();
    let names = zip.parent().unwrap().join("names.json");
    let run = |extra: &[&str]| {
        let out = out_dir("names");
        let r = Command::new(env!("CARGO_BIN_EXE_standuser")).arg(&zip).args(["--table"]).arg(&table).args(["--names"]).arg(&names).args(["--out"]).arg(&out).arg("--name-table").args(extra).output().unwrap();
        let _ = std::fs::remove_dir_all(&out);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        String::from_utf8_lossy(&r.stdout).to_string()
    };
    let plain = run(&[]);
    assert!(plain.starts_with("route: titles"), "{plain}");
    assert_eq!(plain.lines().filter(|l| l.contains(" track ") && l.contains(" of ")).count(), 5 + 0, "{plain}");
    assert_eq!(plain.matches('*').count(), 1, "exactly the pick is starred: {plain}");
    assert!(plain.contains("first-track rule:"));
    assert!(!plain.contains("Nearest canon"), "only the table is printed");
    // the favourites flag takes codes or boxd.it URLs. Make every film the pool shows a favourite: their A moves to the top
    let ratings = std::fs::read_to_string(zip.parent().unwrap().join("files/ratings.csv")).unwrap();
    let codes: Vec<String> = ratings.lines().skip(1).filter_map(|l| l.split(',').nth(3)).map(|u| u.rsplit('/').next().unwrap().to_string()).collect();
    let all = codes.join(",");
    let fav = run(&["--favorites", &all]);
    assert_ne!(fav, plain, "favourites raise A for every film, so the A column cannot stay the same");
    assert_eq!(fav, run(&["--favorites", &all]), "deterministic");
}
