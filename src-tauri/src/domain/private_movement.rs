//! Privatentnahme/-einlage (Functional Core) — Phase 2B, Block 9.
//!
//! Wird **NICHT** in der EÜR aggregiert — Privatbewegungen sind EÜR-neutral
//! (Block 13 klammert sie aus). Sie dienen nur der Vollständigkeit der Kasse:
//! Geld, das zwischen Geschäft und Privat fließt, ohne Betriebsausgabe/-einnahme
//! zu sein.
//!
//! - `entnahme` = Privatentnahme (Geld raus aus dem Geschäft).
//! - `einlage`  = Privateinlage (Geld rein ins Geschäft).
//! - `amount_cents` ist für **beide** Richtungen **positiv**; die Richtung
//!   steckt in `movement_type`.
//!
//! Alle Funktionen pure, keine I/O. Belegnummern-Allokation (`PV-{YYYY}-{NNNN}`)
//! und Persistenz liegen in [`crate::db::numbering`] /
//! [`crate::db::repo::private_movements`].

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Erlaubte Bewegungs-Typen — synchron zum CHECK in `0008_private_movements.sql`.
pub const VALID_MOVEMENT_TYPES: &[&str] = &["entnahme", "einlage"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateMovementInput {
    pub movement_date: NaiveDate,
    /// 'entnahme' | 'einlage'.
    pub movement_type: String,
    /// Immer positiv (Richtung über `movement_type`).
    pub amount_cents: i64,
    /// Betroffenes Zahlungs-Konto (optional).
    pub account_id: Option<String>,
    pub description: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrivateMovementValidationError {
    MovementTypeInvalid { value: String },
    AmountNotPositive { amount: i64 },
    DescriptionMissing,
}

/// Validiert eine [`PrivateMovementInput`]. Aggregiert alle Fehler.
pub fn validate_private_movement(
    input: &PrivateMovementInput,
) -> Result<(), Vec<PrivateMovementValidationError>> {
    use PrivateMovementValidationError as E;
    let mut errs = Vec::new();

    if !VALID_MOVEMENT_TYPES.contains(&input.movement_type.as_str()) {
        errs.push(E::MovementTypeInvalid {
            value: input.movement_type.clone(),
        });
    }
    if input.amount_cents <= 0 {
        errs.push(E::AmountNotPositive {
            amount: input.amount_cents,
        });
    }
    if input.description.trim().is_empty() {
        errs.push(E::DescriptionMissing);
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Menschenlesbare deutsche Fehlermeldung (für Toasts/UI).
pub fn message(e: &PrivateMovementValidationError) -> String {
    use PrivateMovementValidationError as E;
    match e {
        E::MovementTypeInvalid { value } => {
            format!("Ungültige Bewegungs-Art: {value} (erlaubt: entnahme, einlage).")
        }
        E::AmountNotPositive { .. } => "Der Betrag muss größer als 0 sein.".into(),
        E::DescriptionMissing => "Beschreibung ist erforderlich.".into(),
    }
}

/// Maschinenlesbarer Variantenname für DTOs/Logs.
pub fn variant_name(e: &PrivateMovementValidationError) -> &'static str {
    use PrivateMovementValidationError as E;
    match e {
        E::MovementTypeInvalid { .. } => "MovementTypeInvalid",
        E::AmountNotPositive { .. } => "AmountNotPositive",
        E::DescriptionMissing => "DescriptionMissing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> PrivateMovementInput {
        PrivateMovementInput {
            movement_date: NaiveDate::from_ymd_opt(2026, 5, 20).unwrap(),
            movement_type: "entnahme".into(),
            amount_cents: 50_000,
            account_id: None,
            description: "Privatentnahme Mai".into(),
            notes: None,
        }
    }

    #[test]
    fn valid_entnahme_passes() {
        assert!(validate_private_movement(&good()).is_ok());
    }

    #[test]
    fn valid_einlage_passes() {
        let mut m = good();
        m.movement_type = "einlage".into();
        assert!(validate_private_movement(&m).is_ok());
    }

    #[test]
    fn invalid_type_flagged() {
        let mut m = good();
        m.movement_type = "spende".into();
        let err = validate_private_movement(&m).unwrap_err();
        assert!(err.iter().any(|x| matches!(
            x,
            PrivateMovementValidationError::MovementTypeInvalid { .. }
        )));
    }

    #[test]
    fn non_positive_amount_flagged() {
        let mut m = good();
        m.amount_cents = 0;
        let err = validate_private_movement(&m).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, PrivateMovementValidationError::AmountNotPositive { .. })));

        m.amount_cents = -10;
        let err = validate_private_movement(&m).unwrap_err();
        assert!(err
            .iter()
            .any(|x| matches!(x, PrivateMovementValidationError::AmountNotPositive { .. })));
    }

    #[test]
    fn missing_description_flagged() {
        let mut m = good();
        m.description = "  ".into();
        let err = validate_private_movement(&m).unwrap_err();
        assert!(err.contains(&PrivateMovementValidationError::DescriptionMissing));
    }
}
