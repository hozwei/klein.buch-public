//! Front-Matter-Verify für das User-Handbuch (G2-DOC.2.1).
//!
//! Lädt alle `*.md`-Dateien unter `src-tauri/resources/handbook/` und prüft:
//! - jede Datei hat einen YAML-Front-Matter-Block,
//! - die fünf Pflichtfelder (`slug`, `title`, `category`, `order`, `keywords`)
//!   sind vorhanden und nicht-leer,
//! - `category` ist in der Whitelist,
//! - `slug` ist im Verzeichnis eindeutig,
//! - der Dateiname entspricht `<slug>.md`,
//! - `order` ist eine vorzeichenlose Integer,
//! - `keywords` hat mindestens drei Einträge.
//!
//! Bewusst minimalistisch: ein eigener kleiner Reader, damit `serde_yaml`
//! nicht als direkte Dependency aufgenommen werden muss. Der echte
//! Renderer in G2-DOC.3 zieht eine vollständige YAML-Library nach.
//!
//! `README.md` ist die Konventions-Datei und wird ausgespart.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

const ALLOWED_CATEGORIES: &[&str] = &[
    "erste-schritte",
    "bedienen",
    "recht-und-steuern",
    "faq",
    "troubleshooting",
    "glossar",
];

fn handbook_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/handbook")
}

#[derive(Debug)]
struct FrontMatter {
    slug: String,
    title: String,
    category: String,
    order: i64,
    keywords: Vec<String>,
}

/// Trennt `---\n<yaml>\n---\n<body>` und liefert `(yaml, body)`.
fn split_front_matter(raw: &str) -> Result<(&str, &str), String> {
    let stripped = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
        .ok_or_else(|| "Datei startet nicht mit '---' Front-Matter-Marker".to_string())?;
    // Endemarker ist eine Zeile, die nur "---" enthält.
    let end = stripped
        .split("\n---")
        .next()
        .ok_or_else(|| "Front-Matter-Endemarker fehlt".to_string())?;
    let after = &stripped[end.len()..];
    let body = after
        .strip_prefix("\n---\n")
        .or_else(|| after.strip_prefix("\n---\r\n"))
        .or_else(|| after.strip_prefix("\n---"))
        .ok_or_else(|| "Front-Matter-Endemarker fehlt".to_string())?;
    Ok((end, body))
}

fn parse_front_matter(yaml: &str) -> Result<FrontMatter, String> {
    let mut slug: Option<String> = None;
    let mut title: Option<String> = None;
    let mut category: Option<String> = None;
    let mut order: Option<i64> = None;
    let mut keywords: Option<Vec<String>> = None;

    for raw_line in yaml.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        // Sehr simpler Parser: nur top-level "key: value" und "key: [a, b, c]".
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| format!("Zeile ohne ':' im Front-Matter: {line:?}"))?;
        let key = key.trim();
        let value = value.trim();
        match key {
            "slug" => slug = Some(strip_quotes(value).to_string()),
            "title" => title = Some(strip_quotes(value).to_string()),
            "category" => category = Some(strip_quotes(value).to_string()),
            "order" => {
                order = Some(
                    value
                        .parse::<i64>()
                        .map_err(|e| format!("`order` ist keine Integer: {e}"))?,
                )
            }
            "keywords" => {
                let inner = value
                    .strip_prefix('[')
                    .and_then(|s| s.strip_suffix(']'))
                    .ok_or_else(|| {
                        format!(
                            "`keywords` muss als Inline-Liste `[a, b, c]` notiert sein: {value:?}"
                        )
                    })?;
                let items: Vec<String> = inner
                    .split(',')
                    .map(|s| strip_quotes(s.trim()).to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                keywords = Some(items);
            }
            // Andere Keys sind erlaubt (durchgewinkt).
            _ => {}
        }
    }

    Ok(FrontMatter {
        slug: slug.ok_or_else(|| "Pflichtfeld `slug` fehlt".to_string())?,
        title: title.ok_or_else(|| "Pflichtfeld `title` fehlt".to_string())?,
        category: category.ok_or_else(|| "Pflichtfeld `category` fehlt".to_string())?,
        order: order.ok_or_else(|| "Pflichtfeld `order` fehlt".to_string())?,
        keywords: keywords.ok_or_else(|| "Pflichtfeld `keywords` fehlt".to_string())?,
    })
}

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn is_kebab_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
        && !s.contains("--")
}

#[test]
fn handbook_resource_directory_exists() {
    let dir = handbook_dir();
    assert!(
        dir.is_dir(),
        "Handbuch-Verzeichnis fehlt: {}",
        dir.display()
    );
    let readme = dir.join("README.md");
    assert!(
        readme.is_file(),
        "README.md mit Konvention fehlt: {}",
        readme.display()
    );
    let img = dir.join("img");
    assert!(img.is_dir(), "Bilderverzeichnis fehlt: {}", img.display());
}

#[test]
fn front_matter_is_valid_for_every_handbook_page() {
    let dir = handbook_dir();
    let entries = fs::read_dir(&dir).expect("read_dir handbook");

    let mut seen_slugs: HashSet<String> = HashSet::new();
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

        let (yaml, _body) =
            split_front_matter(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let fm = parse_front_matter(yaml).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        assert!(
            is_kebab_case(&fm.slug),
            "{}: slug {:?} ist nicht kebab-case",
            path.display(),
            fm.slug
        );
        assert!(
            ALLOWED_CATEGORIES.contains(&fm.category.as_str()),
            "{}: category {:?} ist nicht in der Whitelist {:?}",
            path.display(),
            fm.category,
            ALLOWED_CATEGORIES
        );
        assert!(
            !fm.title.trim().is_empty(),
            "{}: title ist leer",
            path.display()
        );
        assert!(
            fm.order >= 0,
            "{}: order {} muss >= 0 sein",
            path.display(),
            fm.order
        );
        assert!(
            fm.keywords.len() >= 3,
            "{}: keywords hat nur {} Einträge, mindestens 3 erforderlich",
            path.display(),
            fm.keywords.len()
        );

        let expected_name = format!("{}.md", fm.slug);
        assert_eq!(
            filename,
            expected_name,
            "{}: Dateiname {:?} passt nicht zum slug {:?}",
            path.display(),
            filename,
            fm.slug
        );

        let inserted = seen_slugs.insert(fm.slug.clone());
        assert!(
            inserted,
            "{}: slug {:?} ist doppelt vergeben",
            path.display(),
            fm.slug
        );

        checked += 1;
    }

    assert!(
        checked >= 1,
        "Es muss mindestens eine Demo-Handbuch-Seite mit gültigem Front-Matter geben."
    );
}
