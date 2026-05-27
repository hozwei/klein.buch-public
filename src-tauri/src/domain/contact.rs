//! Contact-Domain (Functional Core). Pure Validierung, kein I/O.
//!
//! Pflichtfelder pro PRD §5.4 / §14 UStG:
//! - `name` (non-empty trimmed)
//! - vollständige Adresse: `street`, `postal_code`, `city` (deutscher Rechnungs-Pflichtangabe)
//! - `country_code` ist 2-Letter ISO (DE-Default vom Schema)
//! - `vat_id` optional; wenn gesetzt muss das Format zur Länder-Konvention passen
//!   (Block 2: DE-VAT-ID = "DE" + 9 Ziffern. Andere EU-Länder: nur strukturelle
//!   Längen-/Prefix-Prüfung. Online-VIES-Check ist out-of-scope für v0.1.)

use serde::{Deserialize, Serialize};

/// Eingabe-Datenmodell für create/update. Wird von Tauri-Commands deserialisiert.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContactInput {
    pub contact_type: ContactType,
    pub name: String,
    pub legal_form: Option<String>,
    pub vat_id: Option<String>,
    pub tax_number: Option<String>,
    pub street: String,
    pub postal_code: String,
    pub city: String,
    pub country_code: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    /// Akzeptiert der Kunde XRechnung/ZUGFeRD? Default true.
    pub accepts_einvoice: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ContactType {
    #[default]
    Customer,
    Vendor,
    Both,
    Partner,
}

impl ContactType {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ContactType::Customer => "customer",
            ContactType::Vendor => "vendor",
            ContactType::Both => "both",
            ContactType::Partner => "partner",
        }
    }

    pub fn from_db_str(s: &str) -> Result<Self, ValidationError> {
        match s {
            "customer" => Ok(ContactType::Customer),
            "vendor" => Ok(ContactType::Vendor),
            "both" => Ok(ContactType::Both),
            "partner" => Ok(ContactType::Partner),
            other => Err(ValidationError::InvalidContactType(other.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("Name ist Pflichtfeld.")]
    NameRequired,
    #[error("Straße ist Pflichtfeld.")]
    StreetRequired,
    #[error("PLZ ist Pflichtfeld.")]
    PostalCodeRequired,
    #[error("Stadt ist Pflichtfeld.")]
    CityRequired,
    #[error("Land ist Pflichtfeld (2-Letter ISO).")]
    CountryRequired,
    #[error("Land muss 2 Buchstaben sein (ISO-3166).")]
    InvalidCountryFormat,
    #[error("PLZ muss 5 Ziffern haben (DE).")]
    InvalidGermanPostalCode,
    #[error("USt-IdNr. (DE) muss Format 'DE' + 9 Ziffern haben.")]
    InvalidGermanVatId,
    #[error("USt-IdNr. (EU) muss mit 2-Letter-Country-Prefix beginnen.")]
    InvalidEuVatId,
    #[error("E-Mail-Adresse ist ungültig.")]
    InvalidEmail,
    #[error("IBAN ist ungültig (Länge nicht im IBAN-Bereich 15–34).")]
    InvalidIbanLength,
    #[error("Contact-Type unbekannt: {0}")]
    InvalidContactType(String),
}

/// Validiert Eingabe für create/update. Whitespace-only Felder zählen als leer.
/// Gibt **alle** Fehler zurück — Frontend kann sie aggregiert anzeigen.
pub fn validate(input: &ContactInput) -> Result<(), Vec<ValidationError>> {
    let mut errs = Vec::new();

    if input.name.trim().is_empty() {
        errs.push(ValidationError::NameRequired);
    }
    if input.street.trim().is_empty() {
        errs.push(ValidationError::StreetRequired);
    }
    if input.postal_code.trim().is_empty() {
        errs.push(ValidationError::PostalCodeRequired);
    }
    if input.city.trim().is_empty() {
        errs.push(ValidationError::CityRequired);
    }
    let cc = input.country_code.trim().to_uppercase();
    if cc.is_empty() {
        errs.push(ValidationError::CountryRequired);
    } else if cc.len() != 2 || !cc.chars().all(|c| c.is_ascii_alphabetic()) {
        errs.push(ValidationError::InvalidCountryFormat);
    } else if cc == "DE"
        && !input.postal_code.trim().is_empty()
        && !is_valid_de_postal_code(input.postal_code.trim())
    {
        errs.push(ValidationError::InvalidGermanPostalCode);
    }

    if let Some(vat) = input
        .vat_id
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if let Err(e) = validate_vat_id(vat) {
            errs.push(e);
        }
    }

    if let Some(email) = input
        .email
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if !is_valid_email(email) {
            errs.push(ValidationError::InvalidEmail);
        }
    }

    if let Some(iban) = input
        .iban
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let normalized: String = iban.chars().filter(|c| !c.is_whitespace()).collect();
        if !(15..=34).contains(&normalized.len()) {
            errs.push(ValidationError::InvalidIbanLength);
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

/// Deutsche PLZ: genau 5 Ziffern.
fn is_valid_de_postal_code(s: &str) -> bool {
    s.len() == 5 && s.chars().all(|c| c.is_ascii_digit())
}

/// USt-IdNr.-Format-Check.
///
/// - DE: "DE" + 9 Ziffern.
/// - andere EU-Länder: 2-Letter Prefix gefolgt von alphanumerischen Zeichen
///   (mindestens 8 weitere). Strukturelle Prüfung — kein VIES.
/// - Eingabe wird whitespace-stripped + uppercased.
pub fn validate_vat_id(raw: &str) -> Result<(), ValidationError> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();

    if cleaned.len() < 4 {
        return Err(ValidationError::InvalidEuVatId);
    }
    let (prefix, rest) = cleaned.split_at(2);
    if !prefix.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(ValidationError::InvalidEuVatId);
    }

    if prefix == "DE" {
        if rest.len() == 9 && rest.chars().all(|c| c.is_ascii_digit()) {
            Ok(())
        } else {
            Err(ValidationError::InvalidGermanVatId)
        }
    } else {
        if rest.len() >= 8 && rest.chars().all(|c| c.is_ascii_alphanumeric()) {
            Ok(())
        } else {
            Err(ValidationError::InvalidEuVatId)
        }
    }
}

/// Pragmatischer E-Mail-Check: `local@domain.tld`, keine Whitespace, mindestens
/// ein Punkt in der Domain. Voller RFC-5322-Check ist out-of-scope.
fn is_valid_email(s: &str) -> bool {
    if s.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    let parts: Vec<&str> = s.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let (local, domain) = (parts[0], parts[1]);
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if !domain.contains('.') {
        return false;
    }
    let tld = domain.rsplit('.').next().unwrap_or("");
    !tld.is_empty() && tld.chars().all(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_input() -> ContactInput {
        ContactInput {
            contact_type: ContactType::Customer,
            name: "Beispiel GmbH".into(),
            legal_form: Some("GmbH".into()),
            vat_id: Some("DE123456789".into()),
            tax_number: None,
            street: "Musterstr. 1".into(),
            postal_code: "84028".into(),
            city: "Landshut".into(),
            country_code: "DE".into(),
            email: Some("info@beispiel.de".into()),
            phone: None,
            iban: Some("DE89370400440532013000".into()),
            bic: None,
            accepts_einvoice: true,
            notes: None,
        }
    }

    #[test]
    fn happy_path_validates() {
        assert!(validate(&ok_input()).is_ok());
    }

    #[test]
    fn name_required() {
        let mut i = ok_input();
        i.name = "   ".into();
        let errs = validate(&i).unwrap_err();
        assert!(errs.contains(&ValidationError::NameRequired));
    }

    #[test]
    fn address_fields_required() {
        let mut i = ok_input();
        i.street = "".into();
        i.postal_code = "".into();
        i.city = "".into();
        let errs = validate(&i).unwrap_err();
        assert!(errs.contains(&ValidationError::StreetRequired));
        assert!(errs.contains(&ValidationError::PostalCodeRequired));
        assert!(errs.contains(&ValidationError::CityRequired));
    }

    #[test]
    fn country_must_be_two_letters() {
        let mut i = ok_input();
        i.country_code = "DEU".into();
        let errs = validate(&i).unwrap_err();
        assert!(errs.contains(&ValidationError::InvalidCountryFormat));
    }

    #[test]
    fn german_postal_code_must_be_five_digits() {
        let mut i = ok_input();
        i.postal_code = "1234".into();
        let errs = validate(&i).unwrap_err();
        assert!(errs.contains(&ValidationError::InvalidGermanPostalCode));
    }

    #[test]
    fn foreign_postal_code_not_checked_strictly() {
        let mut i = ok_input();
        i.country_code = "AT".into();
        i.postal_code = "4020".into();
        assert!(validate(&i).is_ok());
    }

    #[test]
    fn de_vat_id_format() {
        assert!(validate_vat_id("DE123456789").is_ok());
        assert!(validate_vat_id("de 123 456 789").is_ok());
        assert_eq!(
            validate_vat_id("DE12345").unwrap_err(),
            ValidationError::InvalidGermanVatId
        );
        assert_eq!(
            validate_vat_id("DE12345678A").unwrap_err(),
            ValidationError::InvalidGermanVatId
        );
    }

    #[test]
    fn eu_vat_id_format() {
        assert!(validate_vat_id("ATU12345678").is_ok());
        assert!(validate_vat_id("FR12345678901").is_ok());
        assert_eq!(
            validate_vat_id("12X456789").unwrap_err(),
            ValidationError::InvalidEuVatId
        );
    }

    #[test]
    fn empty_optional_fields_ok() {
        let mut i = ok_input();
        i.vat_id = Some("".into());
        i.email = None;
        i.iban = None;
        assert!(validate(&i).is_ok());
    }

    #[test]
    fn email_format_check() {
        let mut i = ok_input();
        i.email = Some("not-an-email".into());
        assert!(validate(&i)
            .unwrap_err()
            .contains(&ValidationError::InvalidEmail));
        i.email = Some("a@b".into());
        assert!(validate(&i)
            .unwrap_err()
            .contains(&ValidationError::InvalidEmail));
        i.email = Some("a@b.de".into());
        assert!(validate(&i).is_ok());
    }

    #[test]
    fn iban_length_check() {
        let mut i = ok_input();
        i.iban = Some("DE89".into());
        assert!(validate(&i)
            .unwrap_err()
            .contains(&ValidationError::InvalidIbanLength));
        i.iban = Some("DE 89 3704 0044 0532 0130 00".into());
        assert!(validate(&i).is_ok());
    }

    #[test]
    fn contact_type_serde_round_trip() {
        for v in [
            ContactType::Customer,
            ContactType::Vendor,
            ContactType::Both,
            ContactType::Partner,
        ] {
            let s = v.as_db_str();
            assert_eq!(ContactType::from_db_str(s).unwrap(), v);
        }
        assert!(ContactType::from_db_str("xxx").is_err());
    }

    #[test]
    fn it_compiles() {}
}
