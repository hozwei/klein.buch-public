//! Factory-Reset-Freigabe — **Functional Core** (G1-RESET, ADR 0036).
//!
//! Reine Prüfung der nicht-I/O-Bedingungen für ein vollständiges Zurücksetzen
//! der lokalen Instanz. Die Passphrase-Verifikation (DB-I/O) und der eigentliche
//! Datei-Nuke liegen in der Imperative Shell (`commands::factory_reset`).
//!
//! GoBD-Bezug: Ein Factory Reset ist die **eine** sanktionierte Total-Löschung
//! (ADR 0036). Selektives Beleg-Löschen bleibt verboten. Bestehen
//! **festgeschriebene** (aufbewahrungspflichtige) Belege, ist der Reset nur nach
//! **Export** ODER nach getippter **Aufbewahrungs-Quittung** zulässig
//! (prüfungssicherer Default).

/// Erwartetes Tipp-Bestätigungswort (G1-RESET.2).
pub const CONFIRM_WORD: &str = "LÖSCHEN";

/// Exakter Wortlaut der Aufbewahrungs-Quittung (G1-RESET.3). Nur relevant, wenn
/// festgeschriebene Belege existieren und **nicht** exportiert wurde.
pub const RETENTION_RECEIPT: &str = "Ich habe meine Aufbewahrungspflicht erfüllt";

/// Eingabe für die Freigabe-Prüfung (alles, was ohne I/O entscheidbar ist).
pub struct ResetRequest<'a> {
    /// Vom Nutzer getipptes Bestätigungswort (muss [`CONFIRM_WORD`] sein).
    pub confirm_word: &'a str,
    /// `true`, wenn der Nutzer im Reset-Flow den Daten-Export ausgeführt hat.
    pub export_confirmed: bool,
    /// Getippte Aufbewahrungs-Quittung (leer, wenn exportiert wurde).
    pub retention_receipt: &'a str,
    /// Anzahl festgeschriebener, aufbewahrungspflichtiger Belege.
    pub locked_documents: i64,
}

/// Prüft die nicht-Passphrase-Bedingungen für den Factory Reset (pure).
///
/// Reihenfolge der Fehlermeldungen ist UI-relevant: zuerst das Tipp-Wort, dann
/// das GoBD-Gating. Liefert `Ok(())`, wenn der Reset (abgesehen von der
/// separat geprüften Passphrase) zulässig ist.
pub fn check_reset_allowed(req: &ResetRequest) -> Result<(), String> {
    if req.confirm_word.trim() != CONFIRM_WORD {
        return Err(format!(
            "Bitte zur Bestätigung genau »{CONFIRM_WORD}« eingeben."
        ));
    }
    if req.locked_documents > 0 {
        let receipt_ok = req.retention_receipt.trim() == RETENTION_RECEIPT;
        if !req.export_confirmed && !receipt_ok {
            return Err(format!(
                "Es bestehen {} festgeschriebene, aufbewahrungspflichtige Belege. \
                 Bitte zuerst die Daten exportieren ODER die Aufbewahrungs-Quittung \
                 bestätigen (genauer Wortlaut: »{RETENTION_RECEIPT}«).",
                req.locked_documents
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req<'a>(
        confirm_word: &'a str,
        export_confirmed: bool,
        retention_receipt: &'a str,
        locked_documents: i64,
    ) -> ResetRequest<'a> {
        ResetRequest {
            confirm_word,
            export_confirmed,
            retention_receipt,
            locked_documents,
        }
    }

    #[test]
    fn rejects_wrong_confirm_word() {
        assert!(check_reset_allowed(&req("loschen", false, "", 0)).is_err());
        assert!(check_reset_allowed(&req("", false, "", 0)).is_err());
        // Trimming erlaubt, Inhalt muss exakt stimmen.
        assert!(check_reset_allowed(&req("  LÖSCHEN  ", false, "", 0)).is_ok());
    }

    #[test]
    fn empty_instance_needs_only_confirm_word() {
        // Keine festgeschriebenen Belege → weder Export noch Quittung nötig.
        assert!(check_reset_allowed(&req(CONFIRM_WORD, false, "", 0)).is_ok());
    }

    #[test]
    fn locked_documents_require_export_or_receipt() {
        // Festgeschriebene Belege, weder Export noch Quittung → blockiert.
        assert!(check_reset_allowed(&req(CONFIRM_WORD, false, "", 3)).is_err());
        // Export bestätigt → erlaubt.
        assert!(check_reset_allowed(&req(CONFIRM_WORD, true, "", 3)).is_ok());
        // Getippte Quittung (exakt) → erlaubt.
        assert!(check_reset_allowed(&req(CONFIRM_WORD, false, RETENTION_RECEIPT, 3)).is_ok());
        // Falsche Quittung → blockiert.
        assert!(check_reset_allowed(&req(CONFIRM_WORD, false, "habe alles", 3)).is_err());
    }
}
