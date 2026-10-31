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
        186,
        "api.ts ruft 186 Commands auf (Plan W5, W7, W8, W9, W12; Lauf F: +Rückstand ×2, \
         +Pause ×2, −payment_accounts_ensure_defaults, +Tages-Backup-Uhrzeit, \
         +Nummernkreise ×2, F1d: +vorgemerkte Entwürfe ×2, +Restore-Prüfung der Abos, \
         F2a: +Entwurf verwerfen, F3a: +Backup herunterladen, F5a: +Entwurf bearbeiten, F6: −2 doppelte Leser; \
         Lauf 2026.11 U4: +Abgangs-Vorschau der Anlagen; \
         Lauf 2026.11.1 V1: +5 Steuerstatus-Commands, V2: +Zahlung stornieren, \
         V4: +Angebots-Entwurf bearbeiten, +Rückstand-Vorschau, ADR 0042)"
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

/// Lauf F6-fix, vervollständigt in F7a: Nachbau des Veröffentlichungs-Filters
/// `Ausgeschlossen` aus `tools/veroeffentlichen/Veroeffentlichen.ps1` (das Werkzeug
/// selbst wird nicht veröffentlicht). Ein Pfadteil (Ordner oder Datei), der in
/// `$NamenAus` steht oder auf ein Muster aus `$MusterAus` passt, wird samt Inhalt
/// ausgelassen. PowerShell vergleicht dabei ohne Rücksicht auf Groß-/Kleinschreibung
/// (`-contains`, `-like`). **Beide Listen bei jeder Änderung im Skript hier
/// nachziehen.** Dateien, die Tests oder der Image-Build lesen, dürfen nicht
/// darunter fallen — im öffentlichen CI fehlten sie sonst (so geschehen mit der
/// App-Icon-Vorlage unter `docs/assets/logo/`).
const PUBLISH_EXCLUDED_NAMES: &[&str] = &[
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
    "CLAUDE.md",
    "TASKS.md",
    "dashboard.html",
    "PRD-klein-buch.md",
    "REVIEW-LOG.md",
    "REVIEW-V2026.5.md",
    "RELEASE-1.0-GUIDE.md",
    "QA-HANDBUCH.md",
];
const PUBLISH_EXCLUDED_PATTERNS: &[&str] = &[
    "HB-AUDIT*.md",
    "*.db",
    "*.sqlite*",
    "*.bak",
    "*.log",
    "*.kbk",
    ".env",
    ".env.*",
    ".~lock.*",
    "~$*",
    "*~",
    "*.part",
    "*.tmp",
    "Thumbs.db",
    ".DS_Store",
    "desktop.ini",
    ".sync_*.db*",
    ".nextcloudsync.log",
    ".owncloudsync.log",
];

/// PowerShell `-like` ohne Zeichenklassen (die Muster oben nutzen nur `*` und `?`),
/// ohne Rücksicht auf Groß-/Kleinschreibung.
fn like_ci(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let (mut pi, mut ni) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

fn publish_excluded(name: &str) -> bool {
    PUBLISH_EXCLUDED_NAMES
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
        || PUBLISH_EXCLUDED_PATTERNS.iter().any(|m| like_ci(m, name))
}

#[test]
fn publish_filter_replica_matches_script_semantics() {
    for name in [
        "logo",
        "LOGO",
        "Data",
        "claude.md",
        "a.DB",
        "x.sqlite-wal",
        "klein.sqlite",
        "backup.KBK",
        ".env.local",
        "notiz.txt~",
        "~$brief.docx",
        ".~lock.a.ods#",
        "HB-AUDIT-UI.md",
        ".sync_abc.db-wal",
        "server.log",
    ] {
        assert!(publish_excluded(name), "{name} müsste ausgeschlossen sein");
    }
    for name in [
        "default.typ",
        "logo-1024.png",
        "database.rs",
        "data-model.md",
        "logos",
        "builder.rs",
        "README.md",
        "versions.json",
        "env.rs",
        "out.rs.txt",
    ] {
        assert!(!publish_excluded(name), "{name} wird veröffentlicht");
    }
}

#[test]
fn files_read_by_tests_survive_publication() {
    let root = manifest_dir().join("..");
    // Von Tests (CARGO_MANIFEST_DIR, include_str!) und vom Image-Build (COPY im
    // Dockerfile) bzw. von CI-Schritten gelesene Dateien und Ordner
    for rel in [
        "docs/assets/app-icon/logo-1024.png",
        "docs/assets/app-icon/logo-1024.svg",
        "inputs/pdf-templates/default.typ",
        "inputs/mail-templates/invoice-de.txt",
        "src/lib/api.ts",
        "compose.example.yaml",
        "Dockerfile",
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        "vite.config.js",
        "tsconfig.json",
        "LICENSE",
        "COPYRIGHT",
        "THIRD-PARTY-NOTICES.md",
        ".trivyignore",
        "legal/build_legal.py",
        "legal/legal.css",
        "legal/check_base_images.py",
        "legal/check_java_inventory.py",
        "src-tauri/Cargo.toml",
        "src-tauri/Cargo.lock",
        "src-tauri/rust-toolchain.toml",
        "src-tauri/deny.toml",
        "src-tauri/about.toml",
        "src-tauri/about.hbs",
        "src-tauri/migrations/0032_gobd_hardening.sql",
        "src-tauri/src/server/passphrase_blocklist.txt",
        "src-tauri/sidecar/versions.json",
        "src-tauri/sidecar/klein-buch-java.sh",
        "src-tauri/sidecar/THIRD-PARTY-JAVA.md",
        "src-tauri/sidecar/lib/kosit-validator.jar",
        "src-tauri/sidecar/lib/mustang.jar",
        "src-tauri/sidecar/notices",
        "src-tauri/sidecar/xrechnung-config",
        "src-tauri/resources/handbook",
        "static",
    ] {
        assert!(root.join(rel).exists(), "{rel} fehlt");
        for part in rel.split('/') {
            assert!(
                !publish_excluded(part),
                "{rel}: Pfadteil {part} wird nicht veröffentlicht"
            );
        }
    }
    assert!(
        !root.join("docs/assets/logo").exists(),
        "App-Icon-Vorlage liegt unter docs/assets/app-icon/ (Lauf F6-fix)"
    );
    // Kein Ordner und keine Datei mit ausgeschlossenem Namen in den Bäumen, die
    // Tests, Build oder CI ganz lesen (Fixtures, docs/assets, resources, inputs, …).
    // In `docs/` liegen bewusst interne Dateien (z. B. `HB-AUDIT*.md`); dort nur Ordner.
    fn walk(dir: &Path, dirs_only: bool, hits: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = path.is_dir();
            if publish_excluded(&name) && (is_dir || !dirs_only) {
                hits.push(path.display().to_string());
            } else if is_dir {
                walk(&path, dirs_only, hits);
            }
        }
    }
    let mut hits = Vec::new();
    for tree in [
        "src-tauri/src",
        "src-tauri/tests",
        "src-tauri/resources",
        "src-tauri/migrations",
        "src-tauri/sidecar/lib",
        "src-tauri/sidecar/notices",
        "src-tauri/sidecar/xrechnung-config",
        "inputs",
        "docs/assets",
        "static",
        "src",
        "legal",
    ] {
        let dir = root.join(tree);
        assert!(dir.is_dir(), "{tree} fehlt");
        walk(&dir, false, &mut hits);
    }
    walk(&root.join("docs"), true, &mut hits);
    assert!(
        hits.is_empty(),
        "Einträge, die das Veröffentlichen auslässt: {hits:?}"
    );
}

/// Lauf F7-fix: Die Monitoring-Tabelle im Betriebshandbuch nennt die Logzeile
/// für Verbindungen, die die Grenze je Gegenstelle abweist (sie erscheinen nicht
/// im Zugriffsprotokoll), mit den Grenzen aus `server::http`.
#[test]
fn operations_guide_names_connection_limit_log_line() {
    use klein_buch_lib::server::http::{MAX_CONNECTIONS, MAX_CONNECTIONS_PER_PEER};
    let ops = std::fs::read_to_string(manifest_dir().join("../docs/operations-guide.md")).unwrap();
    let src = std::fs::read_to_string(manifest_dir().join("src/server/http.rs")).unwrap();
    assert!(
        src.contains("abgewiesen: mehr als {} offene Verbindungen dieser"),
        "Logzeile in server/http.rs geändert — Monitoring-Tabelle nachziehen"
    );
    assert!(ops.contains(&format!(
        "`Verbindung von … abgewiesen: mehr als {MAX_CONNECTIONS_PER_PEER} offene Verbindungen dieser Gegenstelle`"
    )));
    assert!(ops.contains(&format!(
        "Die globale Grenze von {MAX_CONNECTIONS} offenen Verbindungen"
    )));
}
