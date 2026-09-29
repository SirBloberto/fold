use std::fs;
use std::path::{Path, PathBuf};

fn time_of(source: &str) -> f32 {
    source
        .lines()
        .find_map(|line| line.strip_prefix("// time "))
        .map_or(0.0, |seconds| seconds.trim().parse().expect("a time is a number"))
}

fn files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("the conformance folder exists")
        .map(|entry| entry.expect("the folder is readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "fld"))
        .collect();
    files.sort();
    files
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

#[test]
fn every_file_matches_its_reference() {
    let bless = std::env::var_os("FOLD_BLESS").is_some();
    let actual = Path::new(env!("CARGO_TARGET_TMPDIR")).join("conformance");
    fs::create_dir_all(&actual).expect("the output folder can be made");
    let files = files();
    assert!(!files.is_empty(), "no conformance files");
    let mut failures = Vec::new();
    for file in &files {
        let name = file.file_stem().expect("a file has a name").to_string_lossy();
        let source = fs::read_to_string(file).expect("a conformance file is text");
        let (got, kind, other) = match fold::load(&source) {
            Ok(mut picture) => (picture.png(time_of(&source)), "png", "txt"),
            Err(error) => (format!("{error}\n").into_bytes(), "txt", "png"),
        };
        let reference = file.with_extension(kind);
        if bless {
            fs::write(&reference, &got).expect("a reference can be written");
            let _ = fs::remove_file(file.with_extension(other));
            continue;
        }
        let same = match fs::read(&reference) {
            Ok(want) if kind == "txt" => text(&want) == text(&got),
            Ok(want) => want == got,
            Err(_) => {
                failures.push(format!("{name}: no {kind} reference (run with FOLD_BLESS=1)"));
                continue;
            }
        };
        if !same {
            let path = actual.join(format!("{name}.{kind}"));
            fs::write(&path, &got).expect("the actual output can be written");
            failures.push(format!("{name}: differs from its reference, see {}", path.display()));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
