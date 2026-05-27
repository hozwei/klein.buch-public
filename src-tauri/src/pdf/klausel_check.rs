//! §19-Klausel-Block-Check (Functional Core).
//!
//! Pre-Render-Check, der vor jedem PDF-Render im §19-Modus ausgeführt
//! wird. Verifiziert, dass das Typst-Template:
//!
//! 1. Den **Marker-Kommentar** `// §19-KLAUSEL-BLOCK: REQUIRED` trägt
//!    (Self-Documentation für den Template-Autor).
//! 2. Tatsächlich das **Daten-Feld** `data.kleinunternehmer.hinweis_text`
//!    nutzt (also den Klausel-Text *rendert*, nicht nur kommentiert).
//!
//! Wenn beides vorhanden → Ok. Wenn der Aussteller §19-Klein ist und
//! eines fehlt → Err — `lock_and_issue` bricht ab und die Rechnung
//! bleibt im Draft-Status.
//!
//! Bei `is_kleinunternehmer == false` ist der Check ein No-Op
//! (Templates für Regelbesteuerung dürfen den Block weglassen).

/// Marker, der **wortgleich** im Template stehen muss. Ändert sich nie
/// (siehe Memory `memory/klein-buch/pdf-typst.md`).
pub const MARKER_COMMENT: &str = "// §19-KLAUSEL-BLOCK: REQUIRED";

/// Daten-Feld-Referenz, die das Template tatsächlich verwenden muss.
pub const DATA_FIELD_REFERENCE: &str = "kleinunternehmer.hinweis_text";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KlauselCheckError {
    #[error(
        "Template fehlt der Marker-Kommentar '{MARKER_COMMENT}'. Dieses Template darf nicht für §19-Rechnungen verwendet werden."
    )]
    MissingMarker,
    #[error(
        "Template fehlt die Daten-Feld-Verwendung '{DATA_FIELD_REFERENCE}'. Marker ist da, aber Klausel-Text wird nicht gerendert."
    )]
    MissingDataField,
}

/// Strict-Check für §19. Caller ruft das nur, wenn der Aussteller
/// Kleinunternehmer ist.
pub fn verify_for_kleinunternehmer(template_source: &str) -> Result<(), KlauselCheckError> {
    if !template_source.contains(MARKER_COMMENT) {
        return Err(KlauselCheckError::MissingMarker);
    }
    if !template_source.contains(DATA_FIELD_REFERENCE) {
        return Err(KlauselCheckError::MissingDataField);
    }
    Ok(())
}

/// Toleranter Check für Settings-UI / Template-Browser. Gibt sowohl
/// Marker- als auch Feld-Status getrennt zurück, damit das UI sagen
/// kann: "Marker da, aber Feld fehlt — Template-Autor hat vergessen
/// den Text zu rendern."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateKlauselStatus {
    pub has_marker: bool,
    pub uses_data_field: bool,
}

impl TemplateKlauselStatus {
    pub fn is_klein_compatible(&self) -> bool {
        self.has_marker && self.uses_data_field
    }
}

pub fn inspect(template_source: &str) -> TemplateKlauselStatus {
    TemplateKlauselStatus {
        has_marker: template_source.contains(MARKER_COMMENT),
        uses_data_field: template_source.contains(DATA_FIELD_REFERENCE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_TEMPLATE: &str = r#"
        // §19-KLAUSEL-BLOCK: REQUIRED
        #if data.invoice.is_kleinunternehmer [
          #data.kleinunternehmer.hinweis_text
        ]
    "#;

    const TEMPLATE_NO_MARKER: &str = r#"
        // Kein Marker hier
        #data.kleinunternehmer.hinweis_text
    "#;

    const TEMPLATE_NO_FIELD: &str = r#"
        // §19-KLAUSEL-BLOCK: REQUIRED
        // Aber das Feld wird nicht genutzt
    "#;

    #[test]
    fn good_template_passes_strict_check() {
        assert!(verify_for_kleinunternehmer(GOOD_TEMPLATE).is_ok());
    }

    #[test]
    fn missing_marker_errors() {
        assert_eq!(
            verify_for_kleinunternehmer(TEMPLATE_NO_MARKER),
            Err(KlauselCheckError::MissingMarker)
        );
    }

    #[test]
    fn missing_data_field_errors_even_if_marker_present() {
        assert_eq!(
            verify_for_kleinunternehmer(TEMPLATE_NO_FIELD),
            Err(KlauselCheckError::MissingDataField)
        );
    }

    #[test]
    fn inspect_reports_both_dimensions() {
        assert_eq!(
            inspect(GOOD_TEMPLATE),
            TemplateKlauselStatus {
                has_marker: true,
                uses_data_field: true
            }
        );
        assert_eq!(
            inspect(TEMPLATE_NO_MARKER),
            TemplateKlauselStatus {
                has_marker: false,
                uses_data_field: true
            }
        );
        assert_eq!(
            inspect(TEMPLATE_NO_FIELD),
            TemplateKlauselStatus {
                has_marker: true,
                uses_data_field: false
            }
        );
    }

    #[test]
    fn is_klein_compatible_requires_both() {
        assert!(inspect(GOOD_TEMPLATE).is_klein_compatible());
        assert!(!inspect(TEMPLATE_NO_MARKER).is_klein_compatible());
        assert!(!inspect(TEMPLATE_NO_FIELD).is_klein_compatible());
    }
}
