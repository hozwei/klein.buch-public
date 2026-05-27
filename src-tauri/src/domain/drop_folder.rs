//! Drop-Folder (Block PV1-DROP) — **Functional Core**.
//!
//! Pure Klassifikation einer Datei im Watched-Folder plus die Berechnung des
//! Ziel-Unterordners fuer erfolgreich importierte Belege. Keine I/O, keine
//! Time-Quellen — alles deterministisch ueber Dateinamen und ein uebergebenes
//! Datum.
//!
//! Wird von [`crate::scheduler::drop_folder`] benutzt; alle FS-Operationen
//! (Read/Move/Notify) liegen in der Shell.

use chrono::{Datelike, NaiveDate};

/// Wie soll der Scheduler eine im Drop-Folder gefundene Datei behandeln?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropClassification {
    /// `.xml` — direkt in die XRechnung-Parse-Pipeline.
    Xml,
    /// `.pdf` — XML wird via Mustang aus PDF/A-3 extrahiert (ZUGFeRD).
    Pdf,
    /// Versteckte System-/Sync-Artefakte (`.DS_Store`, `._*`, `Thumbs.db`,
    /// `desktop.ini`, OneDrive-`.tmp`-Reste). Komplett ignorieren, NICHT nach
    /// `failed/` verschieben — sonst wandern bei jedem Sync System-Files in
    /// die Fehler-Liste.
    IgnoreHidden,
    /// Andere Endungen (z. B. `.zip`, `.docx`, `.eml`). Wandern nach
    /// `failed/{file}.unsupported`, damit Manuel manuell triagieren kann.
    /// Der Reason landet im `failed/`-Suffix.
    IgnoreOther,
}

/// Klassifikation einer Datei anhand des Dateinamens (NICHT des Inhalts).
///
/// Case-insensitive, weil Windows Dateiendungen unterschiedlich liefert
/// (`Rechnung.PDF` vs. `rechnung.pdf`). Keine MIME-/Magic-Bytes-Pruefung — das
/// macht die Shell beim eigentlichen Lesen (PDF-Magic `%PDF`, UTF-8-Check).
pub fn classify_file(file_name: &str) -> DropClassification {
    // Versteckte Datei-Praefixe der gaengigen Sync-Clients und Betriebssysteme.
    // Reihenfolge: nach Haeufigkeit (Windows-Setup ist das primaere Target).
    let lower = file_name.to_ascii_lowercase();
    if lower.starts_with('.')
        || lower.starts_with("._")
        || lower == "thumbs.db"
        || lower == "desktop.ini"
        || lower.ends_with(".tmp")
        || lower.ends_with(".partial")
        || lower.ends_with(".crdownload")
    {
        return DropClassification::IgnoreHidden;
    }

    // Endungs-Check (case-insensitive). Dateien ohne Endung -> IgnoreOther.
    match lower.rsplit_once('.') {
        Some((_, "xml")) => DropClassification::Xml,
        Some((_, "pdf")) => DropClassification::Pdf,
        _ => DropClassification::IgnoreOther,
    }
}

/// Liefert den relativen Ziel-Unterordner fuer erfolgreich importierte Dateien:
/// `processed/YYYY-MM/`. Datum ist das Importdatum (heute in der Schale), nicht
/// das Beleg-Datum — der Sub-Ordner spiegelt den Sync-Lauf, nicht den
/// Rechnungs-Zeitraum.
///
/// Der Forward-Slash-Separator ist hier ein Logik-Marker; die Shell baut den
/// echten Pfad komponenten-weise via [`std::path::Path::join`] zusammen (siehe
/// Memory `feedback_windows_path_separator_in_tests`).
pub fn processed_subdir(date: NaiveDate) -> String {
    format!("processed/{:04}-{:02}", date.year(), date.month())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_xml_case_insensitive() {
        assert_eq!(classify_file("Rechnung.xml"), DropClassification::Xml);
        assert_eq!(classify_file("rechnung.XML"), DropClassification::Xml);
        assert_eq!(
            classify_file("XRechnung-2026-0001.Xml"),
            DropClassification::Xml
        );
    }

    #[test]
    fn classifies_pdf_case_insensitive() {
        assert_eq!(classify_file("Beleg.pdf"), DropClassification::Pdf);
        assert_eq!(classify_file("BELEG.PDF"), DropClassification::Pdf);
        assert_eq!(classify_file("ZUGFeRD_Sample.Pdf"), DropClassification::Pdf);
    }

    #[test]
    fn ignores_hidden_system_artifacts() {
        assert_eq!(classify_file(".DS_Store"), DropClassification::IgnoreHidden);
        assert_eq!(
            classify_file("._Rechnung.xml"),
            DropClassification::IgnoreHidden
        );
        assert_eq!(classify_file("Thumbs.db"), DropClassification::IgnoreHidden);
        assert_eq!(
            classify_file("desktop.ini"),
            DropClassification::IgnoreHidden
        );
        assert_eq!(
            classify_file(".hidden_dotfile"),
            DropClassification::IgnoreHidden
        );
    }

    #[test]
    fn ignores_browser_and_sync_temp_files() {
        // Halb-fertige Downloads/Syncs duerfen den Drop-Folder nicht stoeren.
        assert_eq!(
            classify_file("Rechnung.xml.tmp"),
            DropClassification::IgnoreHidden
        );
        assert_eq!(
            classify_file("Rechnung.pdf.partial"),
            DropClassification::IgnoreHidden
        );
        assert_eq!(
            classify_file("Rechnung.pdf.crdownload"),
            DropClassification::IgnoreHidden
        );
    }

    #[test]
    fn other_extensions_route_to_failed() {
        assert_eq!(
            classify_file("Rechnung.zip"),
            DropClassification::IgnoreOther
        );
        assert_eq!(
            classify_file("Rechnung.docx"),
            DropClassification::IgnoreOther
        );
        assert_eq!(
            classify_file("Rechnung.eml"),
            DropClassification::IgnoreOther
        );
        // Datei ohne Endung -> IgnoreOther (nicht IgnoreHidden, da keine
        // bekannte Sync-Heuristik greift).
        assert_eq!(
            classify_file("rechnung_ohne_endung"),
            DropClassification::IgnoreOther
        );
    }

    #[test]
    fn processed_subdir_uses_iso_month() {
        let d = NaiveDate::from_ymd_opt(2026, 5, 27).unwrap();
        assert_eq!(processed_subdir(d), "processed/2026-05");
        let d2 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(processed_subdir(d2), "processed/2026-01");
        let d3 = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        assert_eq!(processed_subdir(d3), "processed/2026-12");
    }
}
