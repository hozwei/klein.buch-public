//! KoSIT-Validator-Bridge (Shell).
//!
//! Ruft den gebündelten KoSIT-Validator über `klein-buch-java.{bat,sh}`
//! mit dem `validator`-Sub-Command auf. Eingabe: XML, Output: KoSIT-Report
//! als XML (stdout). Wird zu [`ValidationReport`] geparst.
//!
//! ## Architektur
//!
//! - [`build_args`] — pure: konstruiert die Command-Args.
//! - [`parse_report`] — pure: parst Roh-Report-XML in [`ValidationReport`].
//! - [`validate`] — async I/O: schreibt XML als Temp-File, ruft den
//!   Launcher, sammelt stdout, parst.
//!
//! Mock-Modus über `KLEIN_BUCH_SIDECAR_MOCK=1` — Bridge liefert immer
//! `Passed`. Nur in Debug-/Test-Builds wirksam ([`super::sidecar_mock_active`]);
//! ein Release-Build mit gesetztem Schalter startet nicht (Plan W1.3).

use std::path::{Path, PathBuf};

use crate::einvoice::types::{ValidationFinding, ValidationReport, ValidationStatus};
use crate::error::{Error, Result};

/// Sub-Verzeichnisse / Files im Sidecar-Bundle.
pub const LAUNCHER_FILE_WINDOWS: &str = "klein-buch-java.bat";
pub const LAUNCHER_FILE_UNIX: &str = "klein-buch-java.sh";
pub const SCENARIOS_REL_PATH: &str = "xrechnung-config/scenarios.xml";

/// Liefert den plattform-spezifischen Launcher-Pfad im Sidecar-Bundle.
pub fn launcher_path(sidecar_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        sidecar_dir.join(LAUNCHER_FILE_WINDOWS)
    } else {
        sidecar_dir.join(LAUNCHER_FILE_UNIX)
    }
}

/// Argumente für den Validator-Aufruf. Pure Funktion — testbar ohne I/O.
///
/// `output_dir` ist Pflicht: KoSIT schreibt sein `<filename>-report.xml`
/// per Default ins cwd. Wenn das cwd zufällig das Sidecar-Bundle ist,
/// triggert Tauri-Dev-Mode beim File-Watching einen Rebuild der App
/// mitten in der Pipeline → Crash. Daher leiten wir die Reports explizit
/// in ein Temp-Verzeichnis um.
pub fn build_args(xml_path: &Path, scenarios_path: &Path, output_dir: &Path) -> Vec<String> {
    vec![
        "validator".to_string(),
        "-s".to_string(),
        scenarios_path.to_string_lossy().into_owned(),
        "-o".to_string(),
        output_dir.to_string_lossy().into_owned(),
        xml_path.to_string_lossy().into_owned(),
    ]
}

/// Parst den Roh-KoSIT-Report. Pragmatische Heuristik:
/// - KoSIT 1.6 (`rep:report`): `<rep:assessment><rep:reject>` oder eine
///   `<rep:message level="error">` → `Failed`; nur `level="warning"` →
///   `Warning` (Plan W1.11).
/// - SVRL: `outcome="invalid"` oder ein `<svrl:failed-assert` mit
///   `role="error"` → `Failed`; nur `role="warning"` → `Warning`.
/// - sonst → `Passed`.
///
/// Sammelt bis zu 20 Findings für UI-Anzeige (Regel-ID aus `code` bzw. `id`).
pub fn parse_report(raw_xml: &str) -> ValidationReport {
    let lower = raw_xml.to_lowercase();
    // Nur Attribute der `<rep:message …>`-Tags zählen: der Report enthält auch
    // die Szenario-Konfiguration (`<s:customLevel level="error">…`).
    let kosit_messages = kosit_messages(raw_xml);
    let kosit_errors = kosit_messages
        .iter()
        .filter(|m| m.severity == "error")
        .count() as u32;
    let kosit_warnings = kosit_messages
        .iter()
        .filter(|m| m.severity == "warning")
        .count() as u32;
    let invalid_marker = lower.contains("outcome=\"invalid\"")
        || lower.contains("<rep:reject")
        || kosit_errors > 0
        || lower.contains("role=\"error\"")
        || lower.contains("role=\"fatal\"")
        || lower.contains("flag=\"fatal\"")
        || lower.contains("severity=\"error\"");
    let warning_marker = kosit_warnings > 0
        || lower.contains("role=\"warning\"")
        || lower.contains("severity=\"warning\"")
        || lower.contains("flag=\"warning\"");

    let error_count = count_occurrences(&lower, "<svrl:failed-assert") as u32
        + count_occurrences(&lower, "<failed-assert") as u32
        + kosit_errors;
    let warning_count = count_occurrences(&lower, "role=\"warning\"") as u32
        + count_occurrences(&lower, "severity=\"warning\"") as u32
        + kosit_warnings;

    let status = if invalid_marker {
        ValidationStatus::Failed
    } else if warning_marker {
        ValidationStatus::Warning
    } else {
        ValidationStatus::Passed
    };

    // Findings — flach extrahiert aus failed-assert/text. Limit 20.
    let mut findings = Vec::new();
    for chunk in raw_xml.split("<svrl:failed-assert").skip(1).take(20) {
        let severity = if chunk.contains("role=\"warning\"") {
            "warning"
        } else if chunk.contains("role=\"error\"") || chunk.contains("flag=\"fatal\"") {
            "error"
        } else {
            "info"
        };
        let message = extract_between(chunk, "<svrl:text>", "</svrl:text>")
            .unwrap_or_else(|| chunk.chars().take(200).collect());
        let rule_id = extract_attr(chunk, "id=\"");
        let location = extract_attr(chunk, "location=\"");
        findings.push(ValidationFinding {
            severity: severity.to_string(),
            rule_id,
            message: message.trim().to_string(),
            location,
        });
    }
    let room = 20usize.saturating_sub(findings.len());
    findings.extend(kosit_messages.into_iter().take(room));
    // Fehler zuerst, damit die Meldung beim Festschreiben die Ursache nennt.
    findings.sort_by_key(|f| f.severity != "error");

    ValidationReport {
        status,
        error_count,
        warning_count,
        raw_xml: raw_xml.to_string(),
        findings,
    }
}

/// KoSIT 1.6: `<rep:message id=… level=… xpathLocation=… code="BR-…">Text</rep:message>`.
fn kosit_messages(raw_xml: &str) -> Vec<ValidationFinding> {
    let mut out = Vec::new();
    for chunk in raw_xml.split("<rep:message").skip(1) {
        // `<rep:messages>` o. Ä. ist kein Meldungs-Tag.
        if !chunk.starts_with(|c: char| c.is_whitespace() || c == '>') {
            continue;
        }
        let Some(tag_end) = chunk.find('>') else {
            continue;
        };
        let (attrs, rest) = chunk.split_at(tag_end);
        let severity = extract_attr(attrs, "level=\"")
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_else(|| "info".into());
        let message = rest[1..]
            .split("</rep:message>")
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        out.push(ValidationFinding {
            severity,
            rule_id: extract_attr(attrs, "code=\""),
            message,
            location: extract_attr(attrs, "xpathLocation=\""),
        });
    }
    out
}

fn count_occurrences(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    hay.matches(needle).count()
}

fn extract_between(s: &str, start: &str, end: &str) -> Option<String> {
    let i = s.find(start)?;
    let rest = &s[i + start.len()..];
    let j = rest.find(end)?;
    Some(rest[..j].to_string())
}

fn extract_attr(s: &str, marker: &str) -> Option<String> {
    let i = s.find(marker)?;
    let rest = &s[i + marker.len()..];
    let j = rest.find('"')?;
    Some(rest[..j].to_string())
}

/// Führt den Validator gegen das übergebene XML aus.
///
/// `KLEIN_BUCH_SIDECAR_MOCK=1` skippt in Debug-/Test-Builds den realen Aufruf
/// und liefert einen synthetischen `Passed`-Report (Tests ohne Java).
pub async fn validate(xml: &str, sidecar_dir: &Path) -> Result<ValidationReport> {
    if super::sidecar_mock_active() {
        return Ok(mock_report_passed());
    }

    let launcher = launcher_path(sidecar_dir);
    if !launcher.exists() {
        return Err(Error::Sidecar(format!(
            "Launcher nicht gefunden: {}",
            launcher.display()
        )));
    }

    // XML in Temp-File schreiben (KoSIT-Launcher liest Dateien, nicht stdin).
    let tmpdir = tempfile::tempdir()?;
    let xml_path = tmpdir.path().join("invoice.xml");
    tokio::fs::write(&xml_path, xml).await?;

    // Scenarios als absoluten Pfad — damit ist die cwd-Wahl frei.
    let scenarios_path = sidecar_dir.join(SCENARIOS_REL_PATH);
    let args = build_args(&xml_path, &scenarios_path, tmpdir.path());
    // Timeout + Logging: ein hängender Java-Sidecar darf die Ausstellung nicht
    // unbegrenzt blockieren (sonst friert die App bei „Stelle aus…" ein).
    const VALIDATOR_TIMEOUT_SECS: u64 = 120;
    tracing::info!("KoSIT-Validator startet: {}", launcher.display());
    // Plan W2.6: höchstens `limits::JVM` gleichzeitige Java-Starts.
    let _jvm = crate::limits::JVM
        .acquire()
        .await
        .map_err(|e| Error::Sidecar(format!("JVM-Semaphore: {e}")))?;
    let mut cmd = tokio::process::Command::new(&launcher);
    cmd.args(&args)
        // cwd auf tmpdir — verhindert dass etwaige relative File-Outputs
        // des Java-Sidecars im Sidecar-Bundle landen (Tauri-Dev-Mode-
        // Watcher würde rebuilten und die App mitten in der Pipeline killen).
        .current_dir(tmpdir.path())
        // Bei Timeout (Future-Drop) den hängenden Java-Prozess mit beenden.
        .kill_on_drop(true);
    let output = match tokio::time::timeout(
        std::time::Duration::from_secs(VALIDATOR_TIMEOUT_SECS),
        cmd.output(),
    )
    .await
    {
        Err(_) => {
            return Err(Error::Sidecar(format!(
                "KoSIT-Validator hat nach {VALIDATOR_TIMEOUT_SECS}s nicht geantwortet — \
                 der Java-Sidecar hängt vermutlich. Bitte App neu starten; bleibt es, \
                 Sidecar/JRE prüfen."
            )));
        }
        Ok(Err(e)) => return Err(Error::Sidecar(format!("validator-spawn: {e}"))),
        Ok(Ok(o)) => o,
    };
    tracing::info!("KoSIT-Validator fertig (exit {}).", output.status);

    // Mit `-o` schreibt KoSIT den Report als `<name>-report.xml` in den
    // Ausgabeordner; stdout ist nur die Tabelle. Bei Ablehnung (Exit ≠ 0) ist
    // der Report die einzige Quelle für den Grund (Plan W1.11).
    let report_file = tmpdir.path().join("invoice-report.xml");
    if let Ok(report_xml) = tokio::fs::read_to_string(&report_file).await {
        let report = parse_report(&report_xml);
        if output.status.success() || matches!(report.status, ValidationStatus::Failed) {
            return Ok(report);
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Sidecar(format!(
            "validator exit-code {}: {}",
            output.status, stderr
        )));
    }

    let report_xml = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(parse_report(&report_xml))
}

fn mock_report_passed() -> ValidationReport {
    ValidationReport {
        status: ValidationStatus::Passed,
        error_count: 0,
        warning_count: 0,
        raw_xml: "<mock outcome=\"valid\"/>".into(),
        findings: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_args_matches_expected_order() {
        let args = build_args(
            &PathBuf::from("/tmp/i.xml"),
            &PathBuf::from("/sidecar/xrechnung-config/scenarios.xml"),
            &PathBuf::from("/tmp/reports"),
        );
        assert_eq!(
            args,
            vec![
                "validator".to_string(),
                "-s".to_string(),
                "/sidecar/xrechnung-config/scenarios.xml".to_string(),
                "-o".to_string(),
                "/tmp/reports".to_string(),
                "/tmp/i.xml".to_string(),
            ]
        );
    }

    #[test]
    fn launcher_path_is_platform_specific() {
        let p = launcher_path(Path::new("/sidecar"));
        let s = p.to_string_lossy().to_string();
        if cfg!(target_os = "windows") {
            assert!(s.ends_with("klein-buch-java.bat"), "got {s}");
        } else {
            assert!(s.ends_with("klein-buch-java.sh"), "got {s}");
        }
    }

    #[test]
    fn parse_report_passed_for_clean_xml() {
        let raw = r#"<?xml version="1.0"?><report outcome="valid"/>"#;
        let r = parse_report(raw);
        assert_eq!(r.status, ValidationStatus::Passed);
        assert_eq!(r.error_count, 0);
        assert_eq!(r.warning_count, 0);
    }

    #[test]
    fn parse_report_failed_on_invalid_outcome() {
        let raw = r#"<rep:report outcome="invalid"><rep:assessment/></rep:report>"#;
        let r = parse_report(raw);
        assert_eq!(r.status, ValidationStatus::Failed);
    }

    #[test]
    fn parse_report_failed_on_failed_assert_error_role() {
        let raw = r#"<rep:report>
          <svrl:failed-assert role="error" id="BR-DE-15" location="/Invoice">
            <svrl:text>BuyerReference fehlt</svrl:text>
          </svrl:failed-assert>
        </rep:report>"#;
        let r = parse_report(raw);
        assert_eq!(r.status, ValidationStatus::Failed);
        assert_eq!(r.error_count, 1);
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].severity, "error");
        assert_eq!(r.findings[0].rule_id.as_deref(), Some("BR-DE-15"));
        assert_eq!(r.findings[0].location.as_deref(), Some("/Invoice"));
        assert!(r.findings[0].message.contains("BuyerReference fehlt"));
    }

    #[test]
    fn parse_report_warning_when_only_warnings() {
        let raw = r#"<rep:report>
          <svrl:failed-assert role="warning" id="BR-X">
            <svrl:text>etwas optional</svrl:text>
          </svrl:failed-assert>
        </rep:report>"#;
        let r = parse_report(raw);
        assert_eq!(r.status, ValidationStatus::Warning);
        assert_eq!(r.warning_count, 1);
    }

    /// Gekürzter KoSIT-1.6-Report einer abgelehnten Rechnung (synthetisch).
    /// Enthält wie das Original die Szenario-Konfiguration mit
    /// `<s:customLevel level="error">` — die darf nicht als Meldung zählen.
    const KOSIT16_REJECT: &str = r#"<rep:report xmlns:rep="http://www.xoev.de/de/validator/varl/1" valid="false">
      <rep:scenarioMatched><s:scenario><s:customLevel level="error">CII-SR-452</s:customLevel>
      <s:customLevel level="warning">BR-CL-23</s:customLevel></s:scenario>
      <rep:validationStepResult id="val-sch.1" valid="false">
        <rep:message id="val-sch.1.1" level="error" xpathLocation="/rsm:CrossIndustryInvoice[1]" code="BR-CO-26">[BR-CO-26]-In order for the buyer to automatically identify a supplier, the Seller identifier (BT-29) … shall be present.</rep:message>
        <rep:message id="val-sch.1.2" level="warning" xpathLocation="/rsm:CrossIndustryInvoice[1]" code="BR-DE-21">[BR-DE-21] Hinweis.</rep:message>
        <rep:message id="val-sch.2.1" level="error" xpathLocation="/rsm:CrossIndustryInvoice[1]" code="BR-DE-2">[BR-DE-2] Die Gruppe "SELLER CONTACT" (BG-6) muss übermittelt werden.</rep:message>
      </rep:validationStepResult></rep:scenarioMatched>
      <rep:assessment><rep:reject><rep:explanation><html/></rep:explanation></rep:reject></rep:assessment>
    </rep:report>"#;

    #[test]
    fn parse_report_understands_kosit_16_reject() {
        let r = parse_report(KOSIT16_REJECT);
        assert_eq!(r.status, ValidationStatus::Failed);
        assert_eq!(r.error_count, 2);
        assert_eq!(r.warning_count, 1);
        let ids: Vec<_> = r.findings.iter().map(|f| f.rule_id.as_deref()).collect();
        assert_eq!(
            ids,
            vec![Some("BR-CO-26"), Some("BR-DE-2"), Some("BR-DE-21")]
        );
        assert_eq!(r.findings[0].severity, "error");
        assert!(r.findings[1].message.contains("SELLER CONTACT"));
    }

    #[test]
    fn parse_report_kosit_16_accept_with_config_levels_is_passed() {
        let raw = r#"<rep:report valid="true"><s:scenario>
          <s:customLevel level="error">CII-SR-452</s:customLevel></s:scenario>
          <rep:assessment><rep:accept><rep:explanation><html/></rep:explanation></rep:accept></rep:assessment>
        </rep:report>"#;
        let r = parse_report(raw);
        assert_eq!(r.status, ValidationStatus::Passed);
        assert_eq!(r.error_count, 0);
        assert!(r.findings.is_empty());
    }

    #[test]
    fn mock_passed_report_has_passed_status() {
        let r = mock_report_passed();
        assert_eq!(r.status, ValidationStatus::Passed);
        assert_eq!(r.error_count, 0);
        assert_eq!(r.warning_count, 0);
    }
}
