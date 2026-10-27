//! Debug-Ausgabe ohne Geheimnisse (Lauf F4d, Hinweis aus F4a).
//!
//! Structs mit Passphrase-, Passwort-, Token- oder Code-Feldern leiten
//! `Debug` nicht ab, sondern nutzen [`redacted_debug!`]: Geheime Felder
//! erscheinen als `***` (bzw. `Some(***)`/`None`), alle übrigen normal. So
//! gibt ein späteres `tracing::debug!("{args:?}")` oder ein `assert_eq!` in
//! einem Test nie die Passphrase aus (Backup-Hardline: Passphrase nie in
//! Logs).

use std::fmt;

/// Feldtyp, dessen Inhalt in der Debug-Ausgabe geschwärzt wird.
pub(crate) trait SecretField {
    /// `true`, wenn ein Wert vorhanden ist (für `Some(***)` vs. `None`).
    fn present(&self) -> Option<bool>;
}

impl SecretField for String {
    fn present(&self) -> Option<bool> {
        None
    }
}

impl SecretField for Option<String> {
    fn present(&self) -> Option<bool> {
        Some(self.is_some())
    }
}

/// Hülle für die Debug-Ausgabe eines geheimen Felds.
pub(crate) struct Redacted<'a, T: SecretField>(pub &'a T);

impl<T: SecretField> fmt::Debug for Redacted<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.present() {
            None => f.write_str("***"),
            Some(true) => f.write_str("Some(***)"),
            Some(false) => f.write_str("None"),
        }
    }
}

/// `impl Debug` mit geschwärzten Feldern:
/// `redacted_debug!(Typ { offen_a, offen_b } secret { passphrase });`
macro_rules! redacted_debug {
    ($ty:ident { $($field:ident),* $(,)? } secret { $($secret:ident),+ $(,)? }) => {
        impl ::std::fmt::Debug for $ty {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.debug_struct(stringify!($ty))
                    $(.field(stringify!($field), &self.$field))*
                    $(.field(stringify!($secret), &$crate::redact::Redacted(&self.$secret)))+
                    .finish()
            }
        }
    };
}
pub(crate) use redacted_debug;

#[cfg(test)]
mod tests {
    struct Probe {
        user: String,
        passphrase: String,
        password: Option<String>,
        token: Option<String>,
    }
    redacted_debug!(Probe { user } secret { passphrase, password, token });

    #[test]
    fn secret_fields_never_appear_in_debug_output() {
        let p = Probe {
            user: "kb".into(),
            passphrase: "tinte-gletscher-moped-ahorn".into(),
            password: Some("geheim-123".into()),
            token: None,
        };
        let out = format!("{p:?} {p:#?}");
        assert!(!out.contains("tinte-gletscher"), "{out}");
        assert!(!out.contains("geheim-123"), "{out}");
        assert!(out.contains("\"kb\""), "{out}");
        assert!(out.contains("passphrase: ***"), "{out}");
        assert!(out.contains("password: Some(***)"), "{out}");
        assert!(out.contains("token: None"), "{out}");
    }
}
