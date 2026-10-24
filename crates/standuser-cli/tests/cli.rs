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
