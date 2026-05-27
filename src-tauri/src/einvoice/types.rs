//! Gemeinsame E-Rechnung-Typen.

use serde::{Deserialize, Serialize};

/// Validations-Ergebnis aus dem KoSIT-Sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationStatus {
    /// Strikt valide gegen XRechnung 3.0 + EN-16931 Schematron.
    Passed,
    /// Findings, aber alle nur als Warning eingestuft.
    Warning,
    /// Mindestens ein Error — Rechnung darf nicht ausgehen.
    Failed,
}

/// Aus dem KoSIT-Sidecar gemappter Report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub status: ValidationStatus,
    pub error_count: u32,
    pub warning_count: u32,
    /// Roh-XML-Report (für Audit + UI-Anzeige).
    pub raw_xml: String,
    /// Erste 20 Findings, kondensiert (für Toast/Inline-Display).
    pub findings: Vec<ValidationFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFinding {
    pub severity: String,
    pub rule_id: Option<String>,
    pub message: String,
    pub location: Option<String>,
}

/// Schlanke Variante des [`ValidationReport`] **ohne** das (oft sehr große)
/// Roh-XML. Für den E-Rechnung-Empfang (Block 11): wird ans Frontend gereicht
/// und als JSON in `expenses.einvoice_validation_report` persistiert.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSummary {
    pub status: ValidationStatus,
    pub error_count: u32,
    pub warning_count: u32,
    pub findings: Vec<ValidationFinding>,
}

impl ValidationSummary {
    /// Verdichtet einen [`ValidationReport`] (verwirft das Roh-XML).
    pub fn from_report(r: &ValidationReport) -> Self {
        Self {
            status: r.status.clone(),
            error_count: r.error_count,
            warning_count: r.warning_count,
            findings: r.findings.clone(),
        }
    }

    /// Maschinenlesbarer Status-String für die DB-Spalte
    /// `einvoice_validation_status` (`passed` | `warning` | `failed`).
    pub fn status_str(&self) -> &'static str {
        match self.status {
            ValidationStatus::Passed => "passed",
            ValidationStatus::Warning => "warning",
            ValidationStatus::Failed => "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_status_serializes_lowercase() {
        let s = serde_json::to_string(&ValidationStatus::Passed).unwrap();
        assert_eq!(s, "\"passed\"");
    }
}
