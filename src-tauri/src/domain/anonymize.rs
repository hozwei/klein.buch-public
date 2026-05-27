//! Domain-Core für die DSGVO-Anonymisierung (Art. 17, Block 19) — **pure**, keine I/O.
//!
//! Manuel-Entscheidung (AskUserQuestion 2026-05-24):
//! - `name` → `"Anonymisiert #<kurz-id>"` (Platzhalter mit kurzem ID-Kürzel für
//!   interne Nachvollziehbarkeit in Listen — Pseudonymisierung statt voller
//!   Anonymisierung). Alle übrigen personenbezogenen Felder → NULL.
//! - Anonymisierung nur erlaubt, solange KEINE offenen Entwürfe (unlocked
//!   Rechnungen/Angebote) für den Kontakt existieren. Festgeschriebene Belege
//!   bleiben über ihren Buyer-Snapshot erhalten (§147 AO / GoBD).
//!
//! Diese Schicht trifft nur die textuellen/logischen Entscheidungen; das
//! eigentliche Überschreiben + die Zählung der Entwürfe macht die Shell
//! (`db::repo::contacts`).

/// Präfix des Platzhalter-Namens. Bewusst stabil — Tests + UI hängen daran.
pub const PLACEHOLDER_PREFIX: &str = "Anonymisiert #";

/// Kurzes, deterministisches ID-Kürzel aus der Kontakt-UUID: die letzten 8
/// Hex-Stellen (der Zufalls-/Node-Anteil von UUIDv7, höhere Entropie als der
/// zeitbasierte Präfix), in Großschreibung. Genug, um zwei anonymisierte
/// Kontakte in einer Liste auseinanderzuhalten.
pub fn short_id(contact_id: &str) -> String {
    let hex: String = contact_id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    if hex.is_empty() {
        return "????????".to_string();
    }
    let start = hex.len().saturating_sub(8);
    hex[start..].to_uppercase()
}

/// Der Platzhalter-Name für einen anonymisierten Kontakt.
pub fn anonymized_name(contact_id: &str) -> String {
    format!("{PLACEHOLDER_PREFIX}{}", short_id(contact_id))
}

/// Prüft, ob ein Kontakt anonymisiert werden darf. `None` = erlaubt,
/// `Some(meldung)` = blockiert (offene Entwürfe vorhanden).
pub fn anonymization_blocker(open_invoice_drafts: i64, open_quote_drafts: i64) -> Option<String> {
    if open_invoice_drafts <= 0 && open_quote_drafts <= 0 {
        return None;
    }
    let mut parts = Vec::new();
    if open_invoice_drafts > 0 {
        parts.push(format!("{open_invoice_drafts} offene(r) Rechnungs-Entwurf"));
    }
    if open_quote_drafts > 0 {
        parts.push(format!("{open_quote_drafts} offene(s) Angebot im Entwurf"));
    }
    Some(format!(
        "Anonymisierung nicht möglich: {}. Bitte diese Belege zuerst festschreiben \
         (ausstellen/versenden) oder löschen. Festgeschriebene Belege bleiben über ihren \
         Empfänger-Snapshot erhalten.",
        parts.join(" und ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_has_prefix_and_short_id() {
        let id = "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
        let name = anonymized_name(id);
        assert!(name.starts_with(PLACEHOLDER_PREFIX));
        // letzte 8 Hex-Stellen, groß: "2E3F4A5B"
        assert_eq!(name, "Anonymisiert #2E3F4A5B");
    }

    #[test]
    fn short_id_is_eight_chars() {
        let s = short_id("0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b");
        assert_eq!(s.len(), 8);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn short_id_handles_degenerate_input() {
        assert_eq!(short_id(""), "????????");
        assert_eq!(short_id("ab"), "AB"); // weniger als 8 Stellen → alles nehmen
    }

    #[test]
    fn no_blocker_when_no_open_drafts() {
        assert!(anonymization_blocker(0, 0).is_none());
        assert!(anonymization_blocker(-1, 0).is_none());
    }

    #[test]
    fn blocker_mentions_counts() {
        let msg = anonymization_blocker(2, 0).expect("should block");
        assert!(msg.contains('2'));
        assert!(msg.contains("Rechnungs-Entwurf"));

        let msg2 = anonymization_blocker(0, 3).expect("should block");
        assert!(msg2.contains('3'));
        assert!(msg2.contains("Angebot"));

        let both = anonymization_blocker(1, 1).expect("should block");
        assert!(both.contains(" und "));
    }
}
