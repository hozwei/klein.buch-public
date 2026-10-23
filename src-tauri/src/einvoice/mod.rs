//! E-Rechnung-Schicht (XRechnung + ZUGFeRD).
//!
//! - `parser` (Block 11): Liest XRechnung-XML und extrahiert XML aus ZUGFeRD-PDFs.
//! - `generator` (Block 3): Erzeugt EN-16931-konformes XRechnung-XML aus Invoice-Daten.
//! - `validator` (Block 3): Bridge zum KoSIT-Sidecar.
//! - `mustang_bridge` (Block 3): Bridge zum Mustang-Sidecar (ZUGFeRD-PDF/A-3-Erzeugung).
//! - `types`: gemeinsame Datentypen (Profile, BT-Felder, Cardinality).

pub mod generator;
pub mod mustang_bridge;
pub mod parser;
pub mod types;
pub mod validator;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}

/// Name der Umgebungsvariable für den Sidecar-Mock (KoSIT/Mustang übersprungen).
pub const SIDECAR_MOCK_ENV: &str = "KLEIN_BUCH_SIDECAR_MOCK";

/// Ob der Sidecar-Mock greift: nur in Debug-/Test-Builds **und** mit
/// `KLEIN_BUCH_SIDECAR_MOCK=1` (Plan W1.3). In Release-Builds nie — dort würde
/// ein Mock ungeprüfte E-Rechnungen festschreiben.
pub fn sidecar_mock_active() -> bool {
    cfg!(debug_assertions) && std::env::var(SIDECAR_MOCK_ENV).as_deref() == Ok("1")
}

/// Ergebnis der Startprüfung [`mock_policy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockPolicy {
    Ok,
    RefuseStart,
}

/// Startprüfung (pure): Ein Release-Build (`compiled_in == false`) mit gesetztem
/// Mock-Schalter startet nicht — fail closed, auch bei anderen Werten als `1`,
/// damit eine falsch gesetzte Variable nicht still ignoriert wird.
pub fn mock_policy(compiled_in: bool, env: Option<&str>) -> MockPolicy {
    match env.map(str::trim) {
        Some(v) if !v.is_empty() && !compiled_in => MockPolicy::RefuseStart,
        _ => MockPolicy::Ok,
    }
}

#[cfg(test)]
mod mock_policy_tests {
    use super::*;

    #[test]
    fn release_build_with_mock_env_refuses_start() {
        assert_eq!(mock_policy(false, Some("1")), MockPolicy::RefuseStart);
        assert_eq!(mock_policy(false, Some("true")), MockPolicy::RefuseStart);
    }

    #[test]
    fn release_build_without_mock_env_starts() {
        assert_eq!(mock_policy(false, None), MockPolicy::Ok);
        assert_eq!(mock_policy(false, Some("")), MockPolicy::Ok);
    }

    #[test]
    fn debug_build_may_use_mock() {
        assert_eq!(mock_policy(true, Some("1")), MockPolicy::Ok);
        assert_eq!(mock_policy(true, None), MockPolicy::Ok);
    }
}
