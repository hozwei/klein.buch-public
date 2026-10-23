//! Plan W4/W5 — Kern-Schnitt: Registry und Tauri-Freiheit.
//!
//! - Die Registry (`commands::registry::REGISTRY`) enthält genau die Commands,
//!   die das Frontend in `src/lib/api.ts` aufruft (gelesen per Regex), plus die
//!   vier, die seit W5 über `/api/auth/*` laufen.
//! - Seit W5 gibt es kein Tauri mehr: weder im Code (`tauri::`, `AppHandle`)
//!   noch in `Cargo.toml`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use klein_buch_lib::commands::registry::{find, REGISTRY};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Alle `invoke`-Namen aus `src/lib/api.ts`.
fn api_ts_commands() -> BTreeSet<String> {
    let api = manifest_dir()
        .join("..")
        .join("src")
        .join("lib")
        .join("api.ts");
    let text = std::fs::read_to_string(&api).expect("src/lib/api.ts lesbar");
    let mut out = BTreeSet::new();
    for (idx, _) in text.match_indices("invoke") {
        let rest = &text[idx + "invoke".len()..];
        // optionaler Typparameter `<…>`
        let rest = match rest.strip_prefix('<') {
            Some(r) => match r.find(">(") {
                Some(end) => &r[end + 1..],
                None => continue,
            },
            None => rest,
        };
        let Some(rest) = rest.strip_prefix("(\"") else {
            continue;
        };
        let Some(end) = rest.find('"') else {
            continue;
        };
        let name = &rest[..end];
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        {
            out.insert(name.to_string());
        }
    }
    out
}

#[test]
fn registry_matches_frontend_api() {
    let mut api = api_ts_commands();
    assert_eq!(api.len(), 179, "api.ts ruft 179 Commands auf (Plan W5)");
    // seit W5 über /api/auth/* (transport.ts)
    for name in [
        "backup_needs_onboarding",
        "backup_is_unlocked",
        "backup_setup_passphrase",
        "backup_unlock",
    ] {
        assert!(
            api.insert(name.to_string()),
            "{name} ruft api.ts nicht mehr per invoke auf"
        );
    }
    let registry: BTreeSet<String> = REGISTRY.iter().map(|c| c.name.to_string()).collect();
    let missing: Vec<_> = api.difference(&registry).collect();
    let extra: Vec<_> = registry.difference(&api).collect();
    assert!(
        missing.is_empty(),
        "in api.ts, aber nicht in der Registry: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "in der Registry, aber nicht in api.ts: {extra:?}"
    );
    assert_eq!(REGISTRY.len(), registry.len(), "keine doppelten Namen");
    assert!(
        find("invoices_archive_paths").is_none(),
        "entfällt laut Plan W4"
    );
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_tauri_anywhere() {
    let src = manifest_dir().join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut hits = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("tauri::")
                || code.contains("tauri_plugin")
                || code.contains("AppHandle")
            {
                hits.push(format!(
                    "{}:{}: {}",
                    f.strip_prefix(&src).unwrap().display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(hits.is_empty(), "Tauri-Bezug im Code:\n{}", hits.join("\n"));

    let cargo = std::fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    for line in cargo.lines().filter(|l| !l.trim_start().starts_with('#')) {
        assert!(!line.contains("tauri"), "Tauri in Cargo.toml: {line}");
    }
    for gone in ["build.rs", "tauri.conf.json", "capabilities"] {
        assert!(
            !manifest_dir().join(gone).exists(),
            "{gone} entfällt (Plan W5)"
        );
    }
}

/// Ein Registry-Handler übersetzt das camelCase-JSON in das Args-Struct und
/// meldet ohne offenen Pool `Error::Locked` (W5: HTTP 423).
#[tokio::test]
async fn registry_handler_parses_camel_case_and_reports_locked() {
    use klein_buch_lib::app_ctx::AppCtx;
    use klein_buch_lib::config::Paths;
    use klein_buch_lib::error::Error;
    let dir = tempfile::tempdir().unwrap();
    let ctx = AppCtx::for_tests(Paths::under(dir.path()));
    let spec = find("contacts_list").expect("contacts_list registriert");
    let err = (spec.handler)(ctx.clone(), serde_json::json!({ "includeArchived": true }))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Locked), "{err:?}");

    std::fs::create_dir_all(&ctx.paths.data_dir).unwrap();
    let pool = klein_buch_lib::db::open_pool(&ctx.paths.db_file, None)
        .await
        .unwrap();
    klein_buch_lib::db::MIGRATOR.run(&pool).await.unwrap();
    ctx.set_pool(pool);
    let v = (spec.handler)(ctx.clone(), serde_json::json!({ "includeArchived": true }))
        .await
        .unwrap();
    assert!(v.is_array(), "{v}");
    // Fehlendes Pflichtfeld → Fehler beim Einlesen der Argumente, kein Panic.
    let get = find("contacts_get").unwrap();
    assert!((get.handler)(ctx.clone(), serde_json::json!({}))
        .await
        .is_err());
}
