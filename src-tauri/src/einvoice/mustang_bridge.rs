//! Mustang-Bridge (Shell).
//!
//! Wickelt zwei Mustang-CLI-Aufrufe in eine einzige Bridge-Funktion:
//!
//! 1. **a3only** — Plain-PDF → PDF/A-3 (XMP-Meta, Font-Embedding,
//!    Transparency-Flattening, JS-Strip).
//! 2. **combine** — PDF/A-3 + XRechnung-XML → ZUGFeRD-PDF/A-3 mit
//!    eingebettetem XML (Relationship `Alternative`, AF-Dict-Eintrag).
//!
//! Public API ist `create_zugferd(pdf_bytes, xml) -> Vec<u8>` (siehe
//! PRD §6.10). Caller liefert ein Plain-PDF und das fertige XRechnung-
//! XML, bekommt ein ZUGFeRD-PDF/A-3 zurück.
//!
//! Mock-Modus über `KLEIN_BUCH_SIDECAR_MOCK=1` — Bridge liefert die
//! Original-PDF-Bytes unverändert zurück. CI + Cowork-Sandbox nutzen das.
//!
//! Block 11 ergänzt `extract_xml(pdf_bytes) -> String` für Empfangs-
//! Parser. Hier nur als Stub-Signatur.

use std::path::Path;

use crate::einvoice::validator::launcher_path;
use crate::error::{Error, Result};

/// Konstruiert die Args für den `combine`-Sub-Aufruf. Pure.
///
/// `combine` nutzt intern `ZUGFeRDExporterFromPDFA` und konvertiert das
/// Quell-PDF in einem Schritt nach PDF/A-3 **und** bettet das XML ein —
/// ein separater `a3only`-Vorlauf ist nicht nötig (der nutzte
/// `ZUGFeRDExporterFromA1`, der striktes PDF/A-1-Input verlangt; unser
/// Typst-PDF ist Plain-PDF 1.7).
///
/// Kritische Flags (siehe Mustang `Main.java`):
/// - `--format zf` ZUGFeRD (CII-Container), `--version 2`
/// - `--profile e` = EN16931 (Mustang lowercased + matcht Single-Char-Codes;
///   `EN16931` als Literal würde `Unknown profile` werfen)
/// - `-i` ignoriert PDF/A-Konformitätsfehler des **Inputs** (Typst-PDF ist
///   kein PDF/A) — der Output wird trotzdem als PDF/A-3 geschrieben
/// - `--no-additional-attachments` unterdrückt den interaktiven
///   Attachment-Prompt (sonst blockt Mustang auf stdin → Pipeline hängt)
pub fn build_combine_args(
    source_pdf: &Path,
    source_xml: &Path,
    out_zugferd: &Path,
    profile: &str,
) -> Vec<String> {
    vec![
        "mustang-zugferd".to_string(),
        "--action".to_string(),
        "combine".to_string(),
        "--source".to_string(),
        source_pdf.to_string_lossy().into_owned(),
        "--source-xml".to_string(),
        source_xml.to_string_lossy().into_owned(),
        "--out".to_string(),
        out_zugferd.to_string_lossy().into_owned(),
        "--format".to_string(),
        "zf".to_string(),
        "--version".to_string(),
        "2".to_string(),
        "--profile".to_string(),
        profile.to_string(),
        "-i".to_string(),
        "--no-additional-attachments".to_string(),
    ]
}

/// Konstruiert die Args für `extract` (Block 11).
pub fn build_extract_args(source_zugferd: &Path, out_xml: &Path) -> Vec<String> {
    vec![
        "mustang-zugferd".to_string(),
        "--action".to_string(),
        "extract".to_string(),
        "--source".to_string(),
        source_zugferd.to_string_lossy().into_owned(),
        "--out".to_string(),
        out_xml.to_string_lossy().into_owned(),
    ]
}

/// Wandelt Plain-PDF + XRechnung-XML in ein ZUGFeRD-PDF/A-3 um.
///
/// Ein einziger `combine`-Aufruf konvertiert nach PDF/A-3 und bettet das
/// XML ein. Mock-Modus liefert `pdf_bytes` unverändert.
pub async fn create_zugferd(pdf_bytes: &[u8], xml: &str, sidecar_dir: &Path) -> Result<Vec<u8>> {
    create_zugferd_with_profile(pdf_bytes, xml, sidecar_dir, "e").await
}

/// Wie [`create_zugferd`], aber mit explizitem ZUGFeRD-Profil
/// (`e`=EN16931, `x`=XRechnung, `b`=BASIC, `t`=EXTENDED, …).
pub async fn create_zugferd_with_profile(
    pdf_bytes: &[u8],
    xml: &str,
    sidecar_dir: &Path,
    profile: &str,
) -> Result<Vec<u8>> {
    if std::env::var("KLEIN_BUCH_SIDECAR_MOCK").as_deref() == Ok("1") {
        return Ok(pdf_bytes.to_vec());
    }

    let launcher = launcher_path(sidecar_dir);
    if !launcher.exists() {
        return Err(Error::Sidecar(format!(
            "Launcher nicht gefunden: {}",
            launcher.display()
        )));
    }

    let tmpdir = tempfile::tempdir()?;
    let input_pdf = tmpdir.path().join("input.pdf");
    let xml_path = tmpdir.path().join("invoice.xml");
    let out_zugferd = tmpdir.path().join("zugferd.pdf");

    tokio::fs::write(&input_pdf, pdf_bytes).await?;
    tokio::fs::write(&xml_path, xml).await?;

    // cwd auf tmpdir setzen — vermeidet, dass Java-Sidecar relative Outputs
    // (Logs, temp-XML, etc.) ins Sidecar-Bundle schreibt. Tauri-Dev-Mode-
    // Watcher würde sonst die App mitten in der Pipeline rebuilten.
    let cwd = tmpdir.path();

    // Ein-Schritt-Combine: Plain-PDF → PDF/A-3 + eingebettetes XML.
    let combine_log = run_launcher(
        &launcher,
        cwd,
        &build_combine_args(&input_pdf, &xml_path, &out_zugferd, profile),
        "combine",
    )
    .await?;

    if !out_zugferd.exists() {
        return Err(Error::Sidecar(format!(
            "combine produzierte keine Output-Datei ({}). Mustang-Ausgabe:\n{}",
            out_zugferd.display(),
            tail(&combine_log, 30)
        )));
    }

    let bytes = tokio::fs::read(&out_zugferd).await?;
    validate_combine_output(&bytes, &combine_log)?;
    Ok(bytes)
}

/// R3-005: prüft den Mustang-`combine`-Output auf Plausibilität, bevor er
/// ins write-once-Archiv geht. Pure, damit ohne Sidecar testbar.
///
/// Mustang lieferte schon bei exit-code 0 leere oder partiell geschriebene
/// Output-Files (I/O-Fehler, JVM-OOM). Ohne diese Validierung würde eine
/// 0-Byte-„Rechnung" mit gültigem SHA-256 archiviert — formal hashable, aber
/// inhaltlich tot. GoBD-Bruch.
///
/// Zwei Validierungen:
///  1. nicht leer
///  2. echte PDF-Magic-Bytes (`%PDF-`) — fängt partielle Writes ab
pub fn validate_combine_output(bytes: &[u8], combine_log: &str) -> Result<()> {
    if bytes.is_empty() {
        return Err(Error::Sidecar(format!(
            "Mustang schrieb eine leere PDF-Datei ({} Bytes). Mustang-Ausgabe:\n{}",
            bytes.len(),
            tail(combine_log, 30)
        )));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(Error::Sidecar(format!(
            "Mustang-Output ist keine valide PDF (Magic-Bytes fehlen, {} Bytes geschrieben). Mustang-Ausgabe:\n{}",
            bytes.len(),
            tail(combine_log, 30)
        )));
    }
    Ok(())
}

/// Extrahiert XML aus einem ZUGFeRD-PDF (Block 11). Mock liefert leeren
/// String. Production verifiziert mit Re-Embed-Roundtrip.
pub async fn extract_xml(pdf_bytes: &[u8], sidecar_dir: &Path) -> Result<String> {
    if std::env::var("KLEIN_BUCH_SIDECAR_MOCK").as_deref() == Ok("1") {
        return Ok(String::new());
    }
    let launcher = launcher_path(sidecar_dir);
    if !launcher.exists() {
        return Err(Error::Sidecar(format!(
            "Launcher nicht gefunden: {}",
            launcher.display()
        )));
    }
    let tmpdir = tempfile::tempdir()?;
    let in_pdf = tmpdir.path().join("zugferd.pdf");
    let out_xml = tmpdir.path().join("invoice.xml");
    tokio::fs::write(&in_pdf, pdf_bytes).await?;
    run_launcher(
        &launcher,
        tmpdir.path(),
        &build_extract_args(&in_pdf, &out_xml),
        "extract",
    )
    .await?;
    let xml = tokio::fs::read_to_string(&out_xml).await?;
    Ok(xml)
}

/// Führt einen Mustang-Subcommand aus. Liefert die kombinierte
/// stdout+stderr-Ausgabe zurück (für Diagnose, weil Mustang bei
/// logischen Fehlern manchmal exit-code 0 liefert aber keine Datei
/// schreibt).
async fn run_launcher(
    launcher: &Path,
    cwd: &Path,
    args: &[String],
    action_label: &str,
) -> Result<String> {
    // Timeout + Logging: ein hängender Mustang/Java-Sidecar darf die Ausstellung
    // nicht unbegrenzt blockieren (sonst friert die App bei „Stelle aus…" ein).
    const MUSTANG_TIMEOUT_SECS: u64 = 120;
    tracing::info!(
        "Mustang-Sidecar startet: {action_label} ({}).",
        launcher.display()
    );
    let mut cmd = tokio::process::Command::new(launcher);
    cmd.args(args).current_dir(cwd).kill_on_drop(true);
    let output = match tokio::time::timeout(
        std::time::Duration::from_secs(MUSTANG_TIMEOUT_SECS),
        cmd.output(),
    )
    .await
    {
        Err(_) => {
            return Err(Error::Sidecar(format!(
                "Mustang ({action_label}) hat nach {MUSTANG_TIMEOUT_SECS}s nicht geantwortet — \
                 der Java-Sidecar hängt vermutlich. Bitte App neu starten; bleibt es, \
                 Sidecar/JRE prüfen."
            )));
        }
        Ok(Err(e)) => return Err(Error::Sidecar(format!("mustang-{action_label}-spawn: {e}"))),
        Ok(Ok(o)) => o,
    };
    tracing::info!(
        "Mustang-Sidecar fertig: {action_label} (exit {}).",
        output.status
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    if !output.status.success() {
        return Err(Error::Sidecar(format!(
            "mustang-{action_label} exit-code {}: {}",
            output.status,
            tail(&combined, 30)
        )));
    }
    Ok(combined)
}

/// Letzte `n` nicht-leere Zeilen eines Logs — hält Fehlermeldungen kompakt.
fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn combine_args_have_correct_flags_and_profile() {
        let args = build_combine_args(
            &PathBuf::from("/tmp/in.pdf"),
            &PathBuf::from("/tmp/i.xml"),
            &PathBuf::from("/tmp/z.pdf"),
            "e",
        );
        assert!(args.iter().any(|a| a == "combine"));
        // Quelle ist das Plain-PDF (kein a3only-Vorlauf mehr)
        let s = args.iter().position(|a| a == "--source").unwrap();
        assert_eq!(args[s + 1], "/tmp/in.pdf");
        let x = args.iter().position(|a| a == "--source-xml").unwrap();
        assert_eq!(args[x + 1], "/tmp/i.xml");
        // Profil als Single-Char-Code (Mustang lowercased + matcht so)
        let p = args.iter().position(|a| a == "--profile").unwrap();
        assert_eq!(args[p + 1], "e");
        // Nicht-interaktiv + PDF/A-Input-Errors ignorieren
        assert!(args.iter().any(|a| a == "-i"));
        assert!(args.iter().any(|a| a == "--no-additional-attachments"));
        // ZUGFeRD v2
        let f = args.iter().position(|a| a == "--format").unwrap();
        assert_eq!(args[f + 1], "zf");
        let v = args.iter().position(|a| a == "--version").unwrap();
        assert_eq!(args[v + 1], "2");
    }

    #[test]
    fn extract_args_for_block_11() {
        let args = build_extract_args(&PathBuf::from("/tmp/z.pdf"), &PathBuf::from("/tmp/o.xml"));
        assert_eq!(args[0], "mustang-zugferd");
        assert!(args.iter().any(|a| a == "extract"));
    }

    #[tokio::test]
    async fn mock_mode_returns_input_pdf_unchanged() {
        std::env::set_var("KLEIN_BUCH_SIDECAR_MOCK", "1");
        let pdf = b"%PDF-1.4\nfake".to_vec();
        let xml = "<Invoice/>";
        let out = create_zugferd(&pdf, xml, Path::new("/non/existent"))
            .await
            .unwrap();
        assert_eq!(out, pdf);
        std::env::remove_var("KLEIN_BUCH_SIDECAR_MOCK");
    }
}
