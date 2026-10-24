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
//! AfA-Formular bei jeder Production-Installation crashte. Seit R7-INPUTS
//! bringt das Programm die Default-Inputs (`specs`, `pdf-templates`,
//! `mail-templates`, `branding`) als read-only Quelle mit; im Container kopiert
//! der Serverstart fehlende Dateien in das Volume `/inputs` (Seed-Manifest,
//! ADR 0039). Diese Tests sichern, dass eine spätere Änderung des Nutzers (z. B.
//! `afa-tabellen.json` editiert) NICHT vom nächsten Serverstart überschrieben
//! wird und dass eine angebotene `.neu`-Fassung nach dem Löschen nicht
//! wiederkommt (Lauf F, D-10).

use std::fs;
use std::str::FromStr;

use klein_buch_lib::config::{ensure_inputs_seeded, seed_inputs_for_test, Paths};
use klein_buch_lib::db::MIGRATOR;
use klein_buch_lib::seed;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
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
    // Nur das Seed-Manifest (Plan W11), keine Dateien.
    let names: Vec<String> = fs::read_dir(&target)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n != klein_buch_lib::seed::MANIFEST)
        .collect();
    assert!(names.is_empty(), "Ziel darf nicht befüllt sein: {names:?}");
}

/// Plan W4: `ensure_inputs_seeded` läuft immer (auch im Debug-Build) und liest
/// die Seed-Quelle aus `resource_dir/inputs` — im Container
/// `KLEIN_BUCH_RESOURCE_DIR/inputs` → Volume `/inputs`.
#[test]
fn ensure_inputs_seeded_uses_resource_dir_and_keeps_edits() {
    let tmp = TempDir::new().unwrap();
    let paths = Paths::under(tmp.path());
    write_file(
        &paths
            .resource_dir
            .join("inputs")
            .join("specs")
            .join("afa-tabellen.json"),
        r#"{"version":"BMF-bundled"}"#,
    );
    write_file(
        &paths
            .resource_dir
            .join("inputs")
            .join("mail-templates")
            .join("rechnung.txt"),
        "Hallo",
    );
    write_file(
        &paths.inputs_dir.join("specs").join("afa-tabellen.json"),
        r#"{"version":"BMF-user"}"#,
    );
    ensure_inputs_seeded(&paths).unwrap();
    assert_eq!(
        fs::read_to_string(paths.inputs_dir.join("specs").join("afa-tabellen.json")).unwrap(),
        r#"{"version":"BMF-user"}"#,
        "User-Edit bleibt"
    );
    assert_eq!(
        fs::read_to_string(paths.inputs_dir.join("mail-templates").join("rechnung.txt")).unwrap(),
        "Hallo"
    );
}

/// Fehlt die Seed-Quelle (falsches Image, beschädigte Installation), bricht der
/// Start mit klarer Meldung ab statt später an „Datei nicht gefunden“.
#[test]
fn ensure_inputs_seeded_fails_without_seed_source() {
    let tmp = TempDir::new().unwrap();
    let paths = Paths::under(tmp.path());
    let err = ensure_inputs_seeded(&paths).unwrap_err();
    assert!(err.to_string().contains("Seed-Quelle"), "{err}");
}

/// Quelle und Ziel identisch (Entwicklung mit dem Repo-`inputs/`) → No-op.
#[test]
fn ensure_inputs_seeded_is_noop_when_source_is_target() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    write_file(&repo.join("inputs").join("specs").join("a.json"), "x");
    let paths = Paths::from_dirs(
        tmp.path().join("data"),
        repo.join("inputs"),
        repo.clone(),
        repo.join("web"),
    );
    ensure_inputs_seeded(&paths).unwrap();
    assert_eq!(
        fs::read_to_string(repo.join("inputs").join("specs").join("a.json")).unwrap(),
        "x"
    );
}

/// Lauf F, D-10: Die neue Fassung einer geänderten Vorlage wird genau einmal
/// angeboten und gemeldet — nach dem Löschen der `.neu`-Datei und weiteren
/// Serverstarts entsteht weder die Datei noch ein zweiter Hinweis; eine doppelt
/// ausgelieferte Hinweis-Liste (Abbruch zwischen Inbox und Manifest) legt dank
/// `dedup_key` keinen zweiten Eintrag an.
#[tokio::test]
async fn neu_file_is_offered_and_notified_once() {
    let tmp = TempDir::new().unwrap();
    let bundle = tmp.path().join("bundle");
    let target = tmp.path().join("inputs");
    let rel = "pdf-templates/default.typ";
    write_file(&bundle.join(rel), "v1");
    seed::seed(&bundle, &target, "2026.10.0").unwrap();
    write_file(&target.join(rel), "eigene Fassung");
    write_file(&bundle.join(rel), "v2");
    seed::seed(&bundle, &target, "2026.11.0").unwrap();
    let neu = target.join(format!("{rel}{}", seed::NEW_SUFFIX));
    assert!(neu.exists());

    let db = tmp.path().join("kb.sqlite");
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db.display()))
        .unwrap()
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    let count = |pool: sqlx::SqlitePool| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notifications WHERE dedup_key LIKE 'seed:%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };

    // Abbruch nach der Inbox, vor dem Manifest: dieselben Hinweise noch einmal
    let pending = seed::read_manifest(&target);
    assert_eq!(seed::deliver_notices(&pool, &target).await.unwrap(), 1);
    fs::write(
        target.join(seed::MANIFEST),
        serde_json::to_vec_pretty(&pending).unwrap(),
    )
    .unwrap();
    seed::deliver_notices(&pool, &target).await.unwrap();
    assert_eq!(
        count(pool.clone()).await,
        1,
        "Hinweis nur einmal in der Inbox"
    );

    // Nutzer löscht die .neu-Datei; zwei weitere Serverstarts mit Entsperren
    fs::remove_file(&neu).unwrap();
    for _ in 0..2 {
        seed::seed(&bundle, &target, "2026.11.0").unwrap();
        assert!(!neu.exists(), ".neu kommt nicht wieder");
        assert_eq!(seed::deliver_notices(&pool, &target).await.unwrap(), 0);
    }
    assert_eq!(count(pool.clone()).await, 1);
    assert_eq!(
        fs::read_to_string(target.join(rel)).unwrap(),
        "eigene Fassung"
    );

    // Eine weitere Image-Fassung wird wieder angeboten und gemeldet
    write_file(&bundle.join(rel), "v3");
    seed::seed(&bundle, &target, "2026.12.0").unwrap();
    assert_eq!(fs::read_to_string(&neu).unwrap(), "v3");
    assert_eq!(seed::deliver_notices(&pool, &target).await.unwrap(), 1);
    assert_eq!(count(pool.clone()).await, 2);
}
