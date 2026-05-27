//! Build-Time-Verify für `<HelpAnchor slug="…">`-Aufrufe (G2-DOC.4-A).
//!
//! Walkt rekursiv `klein-buch/src/**/*.svelte`, sammelt jeden Slug-Wert,
//! den ein `<HelpAnchor slug="…">` oder `<HelpAnchor slug='…'>`-Tag
//! nennt, und prüft jeden Slug gegen den Front-Matter-Index der
//! `src-tauri/resources/handbook/*.md`-Dateien (Dateiname == Slug, das
//! garantiert `handbook_resources_test`).
//!
//! Bricht den Build, sobald Code auf einen Slug zeigt, den es im
//! Handbuch nicht gibt. Pendant zu `handbook_resources_test`, das die
//! Markdown-Seiten validiert — gemeinsam halten beide Tests die zwei
//! Seiten der Anker-Konvention konsistent.
//!
//! Bewusst ohne `regex`-/`walkdir`-Dependency: Manueller Walker + ein
//! handgeschriebener Tag-Parser reichen für diese kleine, geschlossene
//! Konvention. Die Komponente selbst (`HelpAnchor.svelte`) enthält im
//! Schreib-Stand `<HelpAnchor`-Erwähnungen nur in Kommentaren ohne
//! `slug=`-Attribut und wird daher korrekt ignoriert; sollte sich das
//! ändern, würde der Test sie wie jede andere Stelle prüfen.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

fn frontend_src_dir() -> PathBuf {
    // `CARGO_MANIFEST_DIR` zeigt auf `klein-buch/src-tauri`. Das Svelte-
    // Frontend liegt eine Ebene höher unter `src/`.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri hat einen Parent")
        .join("src")
}

fn handbook_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/handbook")
}

/// Listet alle Handbuch-Slugs (= Dateiname ohne `.md`) im Resource-
/// Verzeichnis. README.md ist Konvention und wird ausgespart.
fn collect_handbook_slugs() -> BTreeSet<String> {
    let dir = handbook_dir();
    let entries = fs::read_dir(&dir).unwrap_or_else(|e| {
        panic!("Handbuch-Verzeichnis nicht lesbar ({}): {e}", dir.display());
    });

    let mut out: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension() != Some(OsStr::new("md")) {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if stem.eq_ignore_ascii_case("README") {
            continue;
        }
        out.insert(stem.to_string());
    }
    out
}

/// Rekursiv alle `*.svelte`-Dateien unter `dir` einsammeln.
fn collect_svelte_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.is_dir() {
            collect_svelte_files(&path, out);
        } else if path.extension() == Some(OsStr::new("svelte")) {
            out.push(path);
        }
    }
}

/// Ein gefundener `<HelpAnchor>`-Tag mit dem extrahierten `slug`-Wert
/// und der 1-basierten Zeile im Quelltext (für aussagekräftige Fehler).
#[derive(Debug)]
struct AnchorRef {
    file: PathBuf,
    line: usize,
    slug: String,
}

/// Liefert 1-basierte Zeilennummer eines Byte-Offsets im Quelltext.
fn line_of_offset(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

/// Sucht alle `<HelpAnchor ...>`-Tags in `src` und extrahiert den
/// `slug`-Attributwert. Erkennt mehrzeilige Tag-Bodys und beide Quote-
/// Sorten (`"…"` / `'…'`). Self-close (`/>`) und normale Close (`>`)
/// werden gleich behandelt — wir parsen den Eröffnungs-Tag, nicht den
/// Inhalt.
fn extract_anchor_refs(file: &Path, src: &str) -> Vec<AnchorRef> {
    const NEEDLE: &str = "<HelpAnchor";
    let bytes = src.as_bytes();
    let mut out: Vec<AnchorRef> = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel) = src[cursor..].find(NEEDLE) {
        let tag_start = cursor + rel;
        let after_needle = tag_start + NEEDLE.len();

        // Echtes Tag oder doch nur `<HelpAnchorSomething`? Wir akzeptieren
        // nur, wenn direkt nach `HelpAnchor` ein Whitespace, `/`, `>`
        // folgt (oder Datei-Ende).
        let next_ch = bytes.get(after_needle).copied();
        let is_tag = match next_ch {
            None => false,
            Some(b'>') | Some(b'/') => true,
            Some(b) if (b as char).is_whitespace() => true,
            _ => false,
        };
        if !is_tag {
            cursor = after_needle;
            continue;
        }

        // Tag-Ende suchen: erstes `>` ab `after_needle`. Quotes
        // respektieren, damit ein `>` innerhalb eines Attribut-Wertes
        // den Tag nicht vorzeitig schließt.
        let tag_body_end = find_tag_end(bytes, after_needle).unwrap_or(bytes.len());
        let tag_body = &src[after_needle..tag_body_end];

        if let Some(slug) = parse_slug_attr(tag_body) {
            out.push(AnchorRef {
                file: file.to_path_buf(),
                line: line_of_offset(src, tag_start),
                slug,
            });
        }

        cursor = tag_body_end.max(after_needle + 1);
    }

    out
}

fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    let mut in_quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match in_quote {
            Some(q) => {
                if b == q {
                    in_quote = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_quote = Some(b);
                } else if b == b'>' {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

/// Sucht innerhalb eines Tag-Bodys nach `slug="…"` oder `slug='…'` und
/// liefert den ent-quote-ten Wert. Ignoriert Treffer, deren `slug`-
/// Match nicht an einer Attribut-Grenze sitzt (kein willkürliches
/// Submatch wie `myslug=`).
fn parse_slug_attr(tag_body: &str) -> Option<String> {
    let bytes = tag_body.as_bytes();
    let mut i = 0usize;
    while i + 5 <= bytes.len() {
        if &bytes[i..i + 4] == b"slug" {
            // Vorher: Tag-Start oder Whitespace?
            let left_ok = i == 0 || (bytes[i - 1] as char).is_whitespace();
            // Nachher: `=` mit ggf. Whitespace dazwischen?
            let mut j = i + 4;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if left_ok && j < bytes.len() && bytes[j] == b'=' {
                let mut k = j + 1;
                while k < bytes.len() && (bytes[k] as char).is_whitespace() {
                    k += 1;
                }
                if k < bytes.len() && (bytes[k] == b'"' || bytes[k] == b'\'') {
                    let quote = bytes[k];
                    let value_start = k + 1;
                    let mut end = value_start;
                    while end < bytes.len() && bytes[end] != quote {
                        end += 1;
                    }
                    if end < bytes.len() {
                        let raw = &tag_body[value_start..end];
                        let trimmed = raw.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

#[test]
fn frontend_source_directory_exists() {
    let dir = frontend_src_dir();
    assert!(
        dir.is_dir(),
        "Frontend-Source-Verzeichnis fehlt: {}",
        dir.display()
    );
}

#[test]
fn every_help_anchor_slug_matches_a_handbook_page() {
    let src_dir = frontend_src_dir();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_svelte_files(&src_dir, &mut files);
    assert!(
        !files.is_empty(),
        "Keine *.svelte-Files unter {} gefunden — Walker kaputt?",
        src_dir.display()
    );

    let handbook = collect_handbook_slugs();
    assert!(
        !handbook.is_empty(),
        "Handbuch-Index ist leer — Resource-Bundle fehlt?"
    );

    let mut refs: Vec<AnchorRef> = Vec::new();
    for file in &files {
        let raw = fs::read_to_string(file).unwrap_or_else(|e| {
            panic!("Kann {} nicht lesen: {e}", file.display());
        });
        if !raw.contains("<HelpAnchor") {
            continue;
        }
        refs.extend(extract_anchor_refs(file, &raw));
    }

    // Fehlende Slugs gesammelt ausgeben, damit ein Bulk-Fehler nicht
    // hundert einzelne Test-Reruns kostet.
    let mut missing: BTreeMap<String, Vec<(PathBuf, usize)>> = BTreeMap::new();
    for r in &refs {
        if !handbook.contains(&r.slug) {
            missing
                .entry(r.slug.clone())
                .or_default()
                .push((r.file.clone(), r.line));
        }
    }

    if !missing.is_empty() {
        let mut msg = String::new();
        msg.push_str("HelpAnchor-Slugs ohne passende Handbuch-Seite:\n");
        for (slug, places) in &missing {
            msg.push_str(&format!("  - slug=\"{slug}\" — referenziert in:\n"));
            for (file, line) in places {
                msg.push_str(&format!("      {}:{line}\n", file.display()));
            }
        }
        msg.push_str(
            "Bekannte Handbuch-Slugs (Dateinamen unter \
             src-tauri/resources/handbook/*.md):\n",
        );
        for slug in &handbook {
            msg.push_str(&format!("  - {slug}\n"));
        }
        panic!("{msg}");
    }
}

// --- Innenleben ----------------------------------------------------------

#[test]
fn parser_picks_single_line_self_closing_tag() {
    let src = r#"<HelpAnchor slug="euer-export" />"#;
    let path = PathBuf::from("dummy.svelte");
    let refs = extract_anchor_refs(&path, src);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].slug, "euer-export");
}

#[test]
fn parser_picks_multi_line_tag_and_single_quotes() {
    let src = r#"
<HelpAnchor
  slug='backup-und-wiederherstellen'
  heading="Wiederherstellen"
/>
"#;
    let path = PathBuf::from("dummy.svelte");
    let refs = extract_anchor_refs(&path, src);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].slug, "backup-und-wiederherstellen");
}

#[test]
fn parser_ignores_helpanchorish_tags_without_attribute() {
    // `<HelpAnchorButton ...>` ist kein `<HelpAnchor>` — der Marker
    // matched nur, wenn nach `HelpAnchor` ein Tag-Trenner kommt.
    let src = r#"<HelpAnchorButton slug="foo" />"#;
    let path = PathBuf::from("dummy.svelte");
    let refs = extract_anchor_refs(&path, src);
    assert!(refs.is_empty(), "Falscher Treffer auf <HelpAnchorButton>");
}

#[test]
fn parser_ignores_other_attributes_with_slug_substring() {
    // `myslug=` sieht zwar an einer Stelle wie `slug=` aus, ist aber
    // kein eigenes `slug`-Attribut (linke Seite ist kein Whitespace).
    let src = r#"<HelpAnchor myslug="foo" slug="kontakte" />"#;
    let path = PathBuf::from("dummy.svelte");
    let refs = extract_anchor_refs(&path, src);
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].slug, "kontakte");
}

#[test]
fn parser_ignores_empty_slug_value() {
    let src = r#"<HelpAnchor slug="" />"#;
    let path = PathBuf::from("dummy.svelte");
    let refs = extract_anchor_refs(&path, src);
    assert!(refs.is_empty(), "Leere Slugs sollen ignoriert werden");
}
