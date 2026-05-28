//! R7-INPUTS — First-Run-Copy von `inputs/`.
//!
//! Validiert das Verhalten von [`klein_buch_lib::config::seed_inputs_for_test`]
//! (der pure Helfer hinter `ensure_inputs_seeded`):
//! - kopiert fehlende Dateien aus dem Bundle-Mirror in den Ziel-Ordner,
//! - überschreibt NIE existierende Dateien (User-Edits gewinnen — `inputs/`-Hardline),
//! - ist idempotent (mehrfache Aufrufe sind ein no-op, wenn Ziel komplett ist),
//! - legt fehlende Subdirs an.
//!
//! Hintergrund: Bis V2026.5 fehlte `inputs/` komplett im Bundle, weshalb das
//! AfA-Formular bei jeder Production-Installation crashte. R7-INPUTS bundelt
//! die Default-Inputs (`specs`, `pdf-templates`, `mail-templates`, `branding`)
//! als read-only Resource-Mirror und kopiert sie beim ersten Start in den
//! user-editierbaren `app_local_data_dir/inputs/`. Diese Tests sichern, dass
//! eine spätere BMF-Update-Aktion des Users (z.B. `afa-tabellen.json` editiert)
//! NICHT vom nächsten App-Start überschrieben wird.

use std::fs;

use klein_buch_lib::config::seed_inputs_for_test;
use tempfile::TempDir;

/// Helper: legt eine Datei mit gegebenem Inhalt an. Eltern-Verzeichnisse
/// werden ggf. erstellt.
fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[test]
fn seeds_missing_files_from_bundle() {
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("target");

    write_file(
        &bundle.join("specs").join("afa-tabellen.json"),
        r#"{"version":"BMF-2024-12"}"#,
    );
    write_file(
        &bundle.join("pdf-templates").join("default.typ"),
        "// default",
    );
    write_file(
        &bundle.join("branding").join("logo-placeholder.png"),
        "PNGBYTES",
    );

    seed_inputs_for_test(&bundle, &target).unwrap();

    assert_eq!(
        fs::read_to_string(target.join("specs").join("afa-tabellen.json")).unwrap(),
        r#"{"version":"BMF-2024-12"}"#
    );
    assert_eq!(
        fs::read_to_string(target.join("pdf-templates").join("default.typ")).unwrap(),
        "// default"
    );
    assert_eq!(
        fs::read_to_string(target.join("branding").join("logo-placeholder.png")).unwrap(),
        "PNGBYTES"
    );
}

#[test]
fn does_not_overwrite_existing_user_edits() {
    // Der Kern der `inputs/`-Hardline: ein User, der `afa-tabellen.json` mit
    // einer neueren BMF-Tabelle gepflegt hat, darf vom Seeding NICHT
    // ueberschrieben werden.
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("target");

    write_file(
        &bundle.join("specs").join("afa-tabellen.json"),
        r#"{"version":"BMF-bundled"}"#,
    );
    // User hat schon was editiert — Inhalt UNTERSCHEIDET sich vom Bundle.
    write_file(
        &target.join("specs").join("afa-tabellen.json"),
        r#"{"version":"BMF-user-edited"}"#,
    );

    seed_inputs_for_test(&bundle, &target).unwrap();

    assert_eq!(
        fs::read_to_string(target.join("specs").join("afa-tabellen.json")).unwrap(),
        r#"{"version":"BMF-user-edited"}"#,
        "User-Edit darf NICHT ueberschrieben werden"
    );
}

#[test]
fn fills_partial_gaps_alongside_user_edits() {
    // Gemischtes Szenario: User hat eine Datei editiert, eine andere fehlt im
    // Ziel. Der Seeding-Lauf muss die fehlende kopieren UND die editierte
    // unangetastet lassen.
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("target");

    write_file(
        &bundle.join("specs").join("afa-tabellen.json"),
        "bundle-afa",
    );
    write_file(
        &bundle.join("pdf-templates").join("default.typ"),
        "bundle-template",
    );
    // Nur die AfA-Tabelle ist user-editiert. Das Template fehlt im Ziel.
    write_file(&target.join("specs").join("afa-tabellen.json"), "user-afa");

    seed_inputs_for_test(&bundle, &target).unwrap();

    assert_eq!(
        fs::read_to_string(target.join("specs").join("afa-tabellen.json")).unwrap(),
        "user-afa"
    );
    assert_eq!(
        fs::read_to_string(target.join("pdf-templates").join("default.typ")).unwrap(),
        "bundle-template"
    );
}

#[test]
fn is_idempotent() {
    // Zweiter Aufruf ohne Aenderungen ist no-op und faellt nicht ueber bereits
    // kopierte Dateien.
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("target");

    write_file(&bundle.join("specs").join("a.json"), "{}");
    write_file(&bundle.join("nested").join("deep").join("b.txt"), "x");

    seed_inputs_for_test(&bundle, &target).unwrap();
    // Zweiter Aufruf — darf nicht crashen, darf nichts verändern.
    seed_inputs_for_test(&bundle, &target).unwrap();

    assert_eq!(
        fs::read_to_string(target.join("specs").join("a.json")).unwrap(),
        "{}"
    );
    assert_eq!(
        fs::read_to_string(target.join("nested").join("deep").join("b.txt")).unwrap(),
        "x"
    );
}

#[test]
fn creates_nested_subdirs() {
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("target");

    write_file(
        &bundle
            .join("mail-templates")
            .join("subdir")
            .join("deeper")
            .join("note.txt"),
        "deep",
    );

    seed_inputs_for_test(&bundle, &target).unwrap();

    let expected = target
        .join("mail-templates")
        .join("subdir")
        .join("deeper")
        .join("note.txt");
    assert!(expected.is_file(), "tiefe Subdirs wurden nicht angelegt");
    assert_eq!(fs::read_to_string(&expected).unwrap(), "deep");
}

#[test]
fn empty_bundle_directory_is_noop() {
    // Bundle existiert, ist aber leer — kein Crash, Ziel bleibt leer.
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("target");
    fs::create_dir_all(&bundle).unwrap();

    seed_inputs_for_test(&bundle, &target).unwrap();

    assert!(target.is_dir(), "Ziel-Verzeichnis muss angelegt werden");
    let mut entries = fs::read_dir(&target).unwrap();
    assert!(entries.next().is_none(), "Ziel darf nicht befüllt sein");
}
