//! Health-Check des Java-Sidecars beim Serverstart (Lauf F4b, R3-014).
//!
//! Bisher fiel ein fehlender oder kaputter Sidecar erst beim ersten
//! Festschreiben bzw. Einlesen einer E-Rechnung auf. Jetzt prüft der Server
//! direkt nach dem Start im Hintergrund: Launcher, beide JARs und die
//! XRechnung-Konfiguration vorhanden, Java auffindbar und lauffähig
//! (`java -version`, dieselbe Auswahl wie `klein-buch-java.sh`). Bei einem
//! Problem: deutliche ERROR-Zeile im Container-Log und ein Inbox-Hinweis beim
//! nächsten Entsperren ([`crate::notify::deferred`]). Kein Abbruch — alles
//! außer E-Rechnungen funktioniert weiter, und der Betreiber soll die Instanz
//! erreichen, um das Problem zu sehen.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::validator::{launcher_path, LAUNCHER_FILE, SCENARIOS_REL_PATH};
use crate::app_ctx::AppCtx;
use crate::notify::deferred::DeferredNotice;

/// JARs im Sidecar (`lib/`), aufgerufen über den Launcher.
pub const JARS: &[&str] = &["lib/kosit-validator.jar", "lib/mustang.jar"];
/// Höchstdauer der Probe `java -version` (der erste JVM-Start kann auf einem
/// langsamen NAS einige Sekunden dauern).
pub const JAVA_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Java wie im Launcher: `KLEIN_BUCH_JAVA`, sonst die gebündelte Laufzeit
/// `jre/bin/java` (ausführbar), sonst `JAVA_HOME/bin/java`, sonst `java` aus
/// dem `PATH`.
pub fn resolve_java(
    sidecar_dir: &Path,
    klein_buch_java: Option<OsString>,
    java_home: Option<OsString>,
) -> PathBuf {
    if let Some(java) = klein_buch_java.filter(|v| !v.is_empty()) {
        return PathBuf::from(java);
    }
    let bundled = sidecar_dir.join("jre").join("bin").join("java");
    if is_executable(&bundled) {
        return bundled;
    }
    if let Some(home) = java_home.filter(|v| !v.is_empty()) {
        return PathBuf::from(home).join("bin").join("java");
    }
    PathBuf::from("java")
}

fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

/// Fehlende Dateien des Sidecars (Pfade relativ zum Sidecar-Ordner).
pub fn missing_files(sidecar_dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    if !launcher_path(sidecar_dir).is_file() {
        problems.push(format!("{LAUNCHER_FILE} fehlt"));
    }
    for rel in JARS.iter().chain(std::iter::once(&SCENARIOS_REL_PATH)) {
        if !sidecar_dir.join(rel).is_file() {
            problems.push(format!("{rel} fehlt"));
        }
    }
    problems
}

/// Prüft den Sidecar mit einem gegebenen Java; leer = einsatzbereit.
pub async fn check_with(sidecar_dir: &Path, java: &Path) -> Vec<String> {
    let mut problems = missing_files(sidecar_dir);
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let outcome = match cmd.spawn() {
        Ok(mut child) => match tokio::time::timeout(JAVA_PROBE_TIMEOUT, child.wait()).await {
            Ok(Ok(status)) if status.success() => None,
            Ok(Ok(status)) => Some(format!("Java endet mit {status}")),
            Ok(Err(e)) => Some(format!("Java nicht lauffähig ({e})")),
            Err(_) => Some(format!(
                "Java antwortet nicht binnen {} s",
                JAVA_PROBE_TIMEOUT.as_secs()
            )),
        },
        Err(e) => Some(format!("Java nicht startbar ({e})")),
    };
    if let Some(problem) = outcome {
        problems.push(problem);
    }
    problems
}

/// Prüft den Sidecar mit dem Java aus der Umgebung (wie der Launcher).
pub async fn check(sidecar_dir: &Path) -> Vec<String> {
    let java = resolve_java(
        sidecar_dir,
        std::env::var_os("KLEIN_BUCH_JAVA"),
        std::env::var_os("JAVA_HOME"),
    );
    check_with(sidecar_dir, &java).await
}

/// Schlüssel des Inbox-Hinweises: höchstens einer je Version und Berliner Tag.
pub fn notice_key(ctx: &AppCtx) -> String {
    format!(
        "sidecar_health:{}:{}",
        env!("CARGO_PKG_VERSION"),
        crate::clock::berlin_date(ctx.clock().now_utc())
    )
}

/// Wertet das Ergebnis aus: INFO bei Erfolg, sonst ERROR im Log und ein
/// Hinweis für die Inbox beim nächsten Entsperren.
pub fn record(ctx: &AppCtx, problems: &[String]) {
    if problems.is_empty() {
        tracing::info!("Sidecar bereit (KoSIT-Validator, Mustang)");
        return;
    }
    let list = problems.join("; ");
    tracing::error!(
        "Sidecar nicht einsatzbereit ({}): {list} — E-Rechnungen (Festschreiben, Prüfen, \
         Einlesen) scheitern, bis das Image repariert bzw. neu gezogen ist",
        ctx.paths.sidecar_dir.display()
    );
    ctx.deferred_notices.push(DeferredNotice {
        title: "E-Rechnungs-Komponente nicht einsatzbereit".into(),
        body: format!(
            "Beim Start war die Java-Komponente für E-Rechnungen (KoSIT-Validator, Mustang) \
             nicht einsatzbereit: {list}. Rechnungen festschreiben sowie E-Rechnungen prüfen \
             und einlesen scheitern, bis das Image repariert bzw. neu gezogen ist. Alles andere \
             funktioniert. Details stehen im Container-Log."
        ),
        severity: "urgent",
        action_url: None,
        dedup_key: notice_key(ctx),
    });
}

/// Startprüfung im Hintergrund (`serve_main`). Mit aktivem Sidecar-Mock
/// (nur Debug-/Test-Builds) entfällt sie.
pub async fn startup_check(ctx: &AppCtx) {
    if super::sidecar_mock_active() {
        tracing::info!("Sidecar-Mock aktiv — Health-Check übersprungen");
        return;
    }
    let problems = check(&ctx.paths.sidecar_dir).await;
    record(ctx, &problems);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_resolution_follows_the_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path();
        assert_eq!(
            resolve_java(sidecar, Some("/x/java".into()), Some("/home".into())),
            PathBuf::from("/x/java")
        );
        assert_eq!(
            resolve_java(sidecar, None, Some("/opt/jdk".into())),
            PathBuf::from("/opt/jdk/bin/java")
        );
        assert_eq!(resolve_java(sidecar, None, None), PathBuf::from("java"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let bin = sidecar.join("jre/bin");
            std::fs::create_dir_all(&bin).unwrap();
            let java = bin.join("java");
            std::fs::write(&java, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(
                resolve_java(sidecar, None, None),
                PathBuf::from("java"),
                "nicht ausführbar"
            );
            std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
            assert_eq!(resolve_java(sidecar, None, Some("/opt/jdk".into())), java);
        }
    }

    #[tokio::test]
    async fn missing_parts_and_broken_java_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let problems = check_with(dir.path(), Path::new("/nonexistent/bin/java")).await;
        assert!(problems.iter().any(|p| p == "klein-buch-java.sh fehlt"));
        assert!(problems.iter().any(|p| p == "lib/mustang.jar fehlt"));
        assert!(problems
            .iter()
            .any(|p| p == "lib/kosit-validator.jar fehlt"));
        assert!(problems
            .iter()
            .any(|p| p == "xrechnung-config/scenarios.xml fehlt"));
        assert!(problems
            .iter()
            .any(|p| p.starts_with("Java nicht startbar")));
        assert!(
            problems
                .iter()
                .all(|p| !p.contains(&*dir.path().to_string_lossy())),
            "keine Serverpfade im Hinweis: {problems:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn complete_sidecar_with_working_java_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let s = dir.path();
        for rel in [LAUNCHER_FILE, JARS[0], JARS[1], SCENARIOS_REL_PATH] {
            let p = s.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        }
        // `/bin/true` nimmt `-version` hin und endet mit 0.
        assert!(check_with(s, Path::new("/bin/true")).await.is_empty());
        let failing = check_with(s, Path::new("/bin/false")).await;
        assert_eq!(failing.len(), 1, "{failing:?}");
        assert!(failing[0].starts_with("Java endet mit"), "{failing:?}");
    }
}
