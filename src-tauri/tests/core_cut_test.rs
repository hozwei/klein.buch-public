//! Plan W4/W5 — Kern-Schnitt: Registry und Tauri-Freiheit.
//!
//! - Die Registry (`commands::registry::REGISTRY`) enthält genau die Commands,
//!   die das Frontend in `src/lib/api.ts` aufruft (gelesen per Regex).
//! - Seit W5 gibt es kein Tauri mehr: weder im Code (`tauri::`, `AppHandle`)
//!   noch in `Cargo.toml`, `package.json`, den Icons oder `static/` (Lauf F6).

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
    let api = api_ts_commands();
    assert_eq!(
        api.len(),
        177,
        "api.ts ruft 177 Commands auf (Plan W5, W7, W8, W9, W12; Lauf F: +Rückstand ×2, \
         +Pause ×2, −payment_accounts_ensure_defaults, +Tages-Backup-Uhrzeit, \
         +Nummernkreise ×2, F1d: +vorgemerkte Entwürfe ×2, +Restore-Prüfung der Abos, \
         F2a: +Entwurf verwerfen, F3a: +Backup herunterladen, F5a: +Entwurf bearbeiten, F6: −2 doppelte Leser)"
    );
    // Lauf F6: die nur noch gesperrt mitgeführten Desktop-Commands samt Kern
    // entfernt (Archiv über GET /files/archive/{id}, Auth über /api/auth/*,
    // Lizenzen unter /legal/, MS-Graph E9)
    for gone in [
        "invoices_open_pdf",
        "invoices_reveal_pdf",
        "attachments_open",
        "attachments_reveal",
        "euer_reveal_path",
        "backup_open_folder",
        "backup_reveal_path",
        "backup_restore_preview",
        "backup_set_target",
        "backup_set_sftp_target",
        "mail_oauth_status",
        "mail_oauth_connect",
        "mail_oauth_disconnect",
        "third_party_licenses_path",
        "backup_needs_onboarding",
        "backup_is_unlocked",
        "backup_setup_passphrase",
        "backup_unlock",
        // doppelt zu quotes_get (Anhänge) bzw. fiscal_year_overview (autoYearClose)
        "quotes_attachments_list",
        "fiscal_year_auto_close_get",
    ] {
        assert!(find(gone).is_none(), "{gone} entfällt seit Lauf F6");
        assert!(
            !api.contains(gone),
            "{gone} ruft api.ts noch per invoke auf"
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
    // Plan W11: Factory Reset (E7) und Drop-Folder (E11) samt Code entfernt
    for gone in [
        "factory_reset_check",
        "factory_reset",
        "drop_folder_settings_get",
        "drop_folder_settings_set",
        "drop_folder_sync_now",
    ] {
        assert!(find(gone).is_none(), "{gone} entfällt laut Plan W11");
    }
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
    // Lauf F6: Desktop-Icons, Vorlagen-Logos, Lizenz-HTML der Desktop-Fassung
    // und der OAuth-Kern sind entfernt; das Frontend hängt nicht an Tauri.
    let root = manifest_dir().join("..");
    for gone in [
        "src-tauri/icons",
        "src-tauri/resources/handbook/third-party-licenses.html",
        "src-tauri/src/mail/oauth_ms.rs",
        "static/tauri.svg",
        "static/vite.svg",
        "static/svelte.svg",
    ] {
        assert!(!root.join(gone).exists(), "{gone} entfällt (Lauf F6)");
    }
    let package = std::fs::read_to_string(root.join("package.json")).unwrap();
    assert!(
        !package.contains("@tauri-apps") && !package.contains("\"tauri"),
        "Tauri in package.json"
    );
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

/// Lauf F6-fix: `.github/scripts/openapi_from_registry.py` (ZAP-API-Scan in
/// `security.yml`) liest die Command-Namen per Regex aus `registry.rs`
/// (Paare `("name", h::name)` in `ENTRIES`). Dieselbe Erkennung muss genau die
/// Namen der Registry finden — sonst scannt ZAP zu wenig oder bricht ab.
#[test]
fn registry_source_matches_openapi_pattern() {
    let src = std::fs::read_to_string(
        manifest_dir()
            .join("src")
            .join("commands")
            .join("registry.rs"),
    )
    .expect("registry.rs lesbar");
    fn ident(s: &str) -> usize {
        s.find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .unwrap_or(s.len())
    }
    // Nachbau von r'\(\s*"([a-z0-9_]+)",\s*h::[a-z0-9_]+,?\s*\)'
    let mut found = BTreeSet::new();
    for (idx, _) in src.match_indices('(') {
        let rest = src[idx + 1..].trim_start();
        let Some(rest) = rest.strip_prefix('"') else {
            continue;
        };
        let n = ident(rest);
        if n == 0 {
            continue;
        }
        let name = &rest[..n];
        let Some(rest) = rest[n..].strip_prefix("\",") else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix("h::") else {
            continue;
        };
        let m = ident(rest);
        if m == 0 {
            continue;
        }
        let rest = &rest[m..];
        let rest = rest.strip_prefix(',').unwrap_or(rest);
        if rest.trim_start().starts_with(')') {
            found.insert(name.to_string());
        }
    }
    let registry: BTreeSet<String> = REGISTRY.iter().map(|c| c.name.to_string()).collect();
    assert_eq!(
        found, registry,
        "openapi_from_registry.py fände andere Namen als REGISTRY"
    );
    assert!(found.len() >= 100, "Untergrenze des Skripts (100)");
}

/// Lauf F6-fix: `tools/veroeffentlichen/Veroeffentlichen.ps1` übernimmt keinen
/// Pfad, in dem ein Ordner einen dieser Namen trägt (`$NamenAus`; bei Änderung
/// dort hier nachziehen). Dateien, die Tests oder der Build lesen, dürfen daher
/// nicht unter solchen Ordnern liegen — im öffentlichen CI fehlten sie sonst
/// (so geschehen mit der App-Icon-Vorlage unter `docs/assets/logo/`).
const PUBLISH_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "memory",
    "data",
    "build",
    "target",
    "node_modules",
    ".svelte-kit",
    "dist",
    "out",
    ".cache",
    ".idea",
    "logo",
];

#[test]
fn files_read_by_tests_survive_publication() {
    let root = manifest_dir().join("..");
    // Vom Test-Code über CARGO_MANIFEST_DIR gelesene Repo-Dateien
    for rel in [
        "docs/assets/app-icon/logo-1024.png",
        "docs/assets/app-icon/logo-1024.svg",
        "inputs/pdf-templates/default.typ",
        "src/lib/api.ts",
        "compose.example.yaml",
    ] {
        assert!(root.join(rel).is_file(), "{rel} fehlt");
        for part in rel.split('/') {
            assert!(
                !PUBLISH_EXCLUDED_DIRS.contains(&part),
                "{rel}: Pfadteil {part} wird nicht veröffentlicht"
            );
        }
    }
    assert!(
        !root.join("docs/assets/logo").exists(),
        "App-Icon-Vorlage liegt unter docs/assets/app-icon/ (Lauf F6-fix)"
    );
    // Kein Ordner mit ausgeschlossenem Namen in den veröffentlichten Quellbäumen
    fn walk(dir: &Path, hits: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if PUBLISH_EXCLUDED_DIRS.contains(&name.as_str()) {
                    hits.push(path.display().to_string());
                } else {
                    walk(&path, hits);
                }
            }
        }
    }
    let mut hits = Vec::new();
    for tree in [
        "src-tauri/src",
        "src-tauri/tests",
        "src-tauri/resources/handbook",
        "src-tauri/migrations",
        "inputs",
        "docs",
        "static",
        "src",
        "legal",
    ] {
        walk(&root.join(tree), &mut hits);
    }
    assert!(
        hits.is_empty(),
        "Ordner, die das Veröffentlichen auslässt: {hits:?}"
    );
}
