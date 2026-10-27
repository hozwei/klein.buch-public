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
    // U6-006: Wörter der Oberfläche (Ausstellen, Fertigstellen, „Entwurf verwerfen“).
    let mut parts = Vec::new();
    if open_invoice_drafts > 0 {
        let noun = if open_invoice_drafts == 1 {
            "Rechnungs-Entwurf"
        } else {
            "Rechnungs-Entwürfe"
        };
        parts.push(format!("{open_invoice_drafts} {noun}"));
    }
    if open_quote_drafts > 0 {
        let noun = if open_quote_drafts == 1 {
            "Angebots-Entwurf"
        } else {
            "Angebots-Entwürfe"
        };
        parts.push(format!("{open_quote_drafts} {noun}"));
    }
    Some(format!(
        "Anonymisierung nicht möglich: Es gibt noch offene Entwürfe ({}). Stell Rechnungen \
         zuerst aus und Angebote fertig oder verwirf die Entwürfe („Entwurf verwerfen“ im \
         Menü „Mehr“). Ausgestellte und fertiggestellte Belege behalten die Empfängerdaten; \
         verworfene Entwürfe verlieren sie bei der Anonymisierung.",
        parts.join(" und ")
    ))
}

/// Nennt `text` den Namen eines Kontakts (Lauf F3b)? Ganzer Name oder ein
/// Namensteil ab vier Zeichen, ohne Groß-/Kleinschreibung (Unicode, also auch
/// „MÜLLER“ = „Müller“); die Rechtsform „GmbH“ zählt nicht als Namensteil.
/// Großzügig gemeint: Ein Treffer ersetzt bei der Anonymisierung nur die
/// Buchungsvorlage eines pausierten Kosten-Abos durch einen Platzhalter.
pub fn mentions_name(text: &str, name: &str) -> bool {
    let text = text.to_lowercase();
    let name = name.trim().to_lowercase();
    if name.chars().count() >= 2 && text.contains(&name) {
        return true;
    }
    name.split(|c: char| !c.is_alphanumeric())
        .filter(|part| part.chars().count() >= 4 && *part != "gmbh")
        .any(|part| text.contains(part))
}

#[cfg(test)]
mod tests {
    #[test]
    fn mentions_name_finds_full_name_and_parts() {
        use super::mentions_name;
        assert!(mentions_name("Miete Büro – Hans Müller", "Hans Müller"));
        assert!(mentions_name("Miete Büro MÜLLER", "Hans Müller"));
        assert!(mentions_name("Miete bei Li", "Li"));
        assert!(!mentions_name("Miete Büro", "Hans Müller"));
        assert!(!mentions_name("Lizenz Software GmbH", "Beispiel GmbH"));
        assert!(!mentions_name("Hosting", ""));
    }

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
        assert!(msg.contains("2 Rechnungs-Entwürfe"));

        let msg2 = anonymization_blocker(0, 3).expect("should block");
        assert!(msg2.contains("3 Angebots-Entwürfe"));

        let both = anonymization_blocker(1, 1).expect("should block");
        assert!(both.contains("1 Rechnungs-Entwurf und 1 Angebots-Entwurf"));
        assert!(both.contains("„Entwurf verwerfen“"));
    }
}
