//! Footer-Verify für das User-Handbuch (G2-DOC.2.9).
//!
//! Erzwingt, dass jede Handbuch-Seite (alle `*.md` außer `README.md`)
//! mit dem in `README.md` festgelegten Versionierungs-Footer endet:
//!
//! ```text
//! ---
//!
//! *Letzte Aktualisierung: TT.MM.JJJJ · Klein.Buch X.Y*
//! ```
//!
//! Format-Regeln, die der Test prüft:
//! - Vor dem schließenden Block steht eine Leerzeile.
//! - Davor steht eine Trennlinie `---` auf eigener Zeile.
//! - Die letzte sichtbare Zeile ist die Cursive-Zeile mit Datum und
//!   Versions-String, eingerahmt von `*`.
//! - Das Datum ist im **deutschen Format** `TT.MM.JJJJ`
//!   (Manuel-Hardline 27.05.2026 — ISO-Form `YYYY-MM-DD` verwirrt
//!   deutsche Endnutzer).
//! - Der App-Versions-Teil hat die Form `Klein.Buch <semver-prefix>`,
//!   wobei der Versions-Prefix mindestens `MAJOR.MINOR` enthält.
//!
//! Bewusst eigenständige Datei (nicht zusammen mit
//! `handbook_resources_test.rs`), damit Front-Matter- und Footer-
//! Verträge getrennt einsehbar bleiben (Sub-Block-Trennung
//! G2-DOC.2.1 vs. G2-DOC.2.9).

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

fn handbook_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/handbook")
}

fn is_german_date(s: &str) -> bool {
    // Format: TT.MM.JJJJ — exakt 10 Zeichen, Punkte an Position 2 und 5.
    let bytes = s.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    bytes[2] == b'.'
        && bytes[5] == b'.'
        && bytes[..2].iter().all(|b| b.is_ascii_digit())
        && bytes[3..5].iter().all(|b| b.is_ascii_digit())
        && bytes[6..10].iter().all(|b| b.is_ascii_digit())
}

fn is_version_prefix(s: &str) -> bool {
    let mut parts = s.split('.');
    let major = parts.next();
    let minor = parts.next();
    match (major, minor) {
        (Some(a), Some(b)) => {
            !a.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && !b.is_empty()
                && b.chars().all(|c| c.is_ascii_digit())
        }
        _ => false,
    }
}

fn check_footer(path: &PathBuf, raw: &str) -> Result<(), String> {
    // Normalisiere CRLF zu LF, trimme Trailing-Whitespace.
    let normalized = raw.replace("\r\n", "\n");
    let trimmed = normalized.trim_end_matches('\n').trim_end_matches(' ');

    // Letzte nicht-leere Zeile ist die Footer-Zeile.
    let last_line = trimmed
        .lines()
        .last()
        .ok_or_else(|| "Datei ist leer".to_string())?;

    let footer_text = last_line
        .strip_prefix("*Letzte Aktualisierung: ")
        .and_then(|s| s.strip_suffix('*'))
        .ok_or_else(|| {
            format!(
                "Footer-Zeile entspricht nicht dem Schema \
                 `*Letzte Aktualisierung: TT.MM.JJJJ · Klein.Buch X.Y*`. \
                 Gefunden: {last_line:?}"
            )
        })?;

    // Format `TT.MM.JJJJ · Klein.Buch X.Y[.Z]`.
    let (date, rest) = footer_text
        .split_once(" · ")
        .ok_or_else(|| format!("Footer ohne Mitte-Trenner ` · `: {last_line:?}"))?;

    if !is_german_date(date) {
        return Err(format!(
            "Footer-Datum {date:?} ist nicht deutsches Format `TT.MM.JJJJ`"
        ));
    }

    let version = rest
        .strip_prefix("Klein.Buch ")
        .ok_or_else(|| format!("Footer ohne `Klein.Buch <version>` -Suffix: {rest:?}"))?;

    if !is_version_prefix(version) {
        return Err(format!(
            "Footer-Versions-String {version:?} ist nicht `MAJOR.MINOR[.PATCH]`"
        ));
    }

    // Vor der Footer-Zeile muss eine Leerzeile, davor eine `---`-Linie stehen.
    let lines: Vec<&str> = trimmed.lines().collect();
    let n = lines.len();
    if n < 3 {
        return Err("Datei zu kurz für Footer-Block".to_string());
    }
    let above_footer = lines[n - 2];
    let separator = lines[n - 3];
    if !above_footer.is_empty() {
        return Err(format!(
            "Vor der Footer-Zeile fehlt eine Leerzeile (gefunden {above_footer:?})"
        ));
    }
    if separator.trim() != "---" {
        return Err(format!(
            "Vor dem Footer fehlt eine `---`-Trennlinie (gefunden {separator:?})"
        ));
    }

    let _ = path; // suppress unused warning in non-error paths
    Ok(())
}

#[test]
fn every_handbook_page_ends_with_versioned_footer() {
    let dir = handbook_dir();
    let entries = fs::read_dir(&dir).expect("read_dir handbook");

    let mut checked: usize = 0;
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension() != Some(OsStr::new("md")) {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if filename.eq_ignore_ascii_case("README.md") {
            continue;
        }

        let raw = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!("kann {} nicht lesen: {e}", path.display());
        });

        if let Err(e) = check_footer(&path, &raw) {
            panic!("{}: {e}", path.display());
        }
        checked += 1;
    }

    assert!(
        checked >= 1,
        "Mindestens eine Handbuch-Seite muss den Footer-Test durchlaufen."
    );
}
