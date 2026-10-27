//! Bedingte Re-Auth im Kern (W14-038, Lauf F4a).
//!
//! Commands mit [`super::registry::Auth::Conditional`] verlangen eine frische
//! Bestätigung (Passphrase + zweiter Faktor) nur für bestimmte Änderungen —
//! welche, entscheidet der Kern gegen den gespeicherten Stand (z. B. IBAN,
//! BIC, §19-Status beim Verkäuferprofil). Der HTTP-Dispatcher führt jeden
//! Command in [`with_reauth`] aus und gibt mit, ob die Sitzung frisch bestätigt
//! ist (Re-Auth der letzten 5 min oder Anmeldung der letzten 5 min); der Kern
//! ruft bei einer betroffenen Änderung [`require_fresh`].
//!
//! Ohne Angabe (Aufruf außerhalb des Dispatchers) gilt die Sitzung als **nicht**
//! bestätigt — fail-closed; interne Aufrufer müssen ausdrücklich
//! [`with_reauth`] verwenden.

use std::future::Future;

use crate::error::{Error, Result};

tokio::task_local! {
    static REAUTH_FRESH: bool;
}

/// Führt `fut` mit der Angabe aus, ob die aufrufende Sitzung frisch bestätigt
/// ist. Ein eigener Task (`tokio::spawn`) erbt die Angabe nicht.
pub async fn with_reauth<F: Future>(fresh: bool, fut: F) -> F::Output {
    REAUTH_FRESH.scope(fresh, fut).await
}

/// Ist die aufrufende Sitzung frisch bestätigt? Ohne Angabe: nein.
pub fn is_fresh() -> bool {
    REAUTH_FRESH.try_with(|f| *f).unwrap_or(false)
}

/// Verlangt eine frische Bestätigung; sonst [`Error::ReauthRequired`]
/// (HTTP 403 `reauth_required`, das Frontend öffnet den Bestätigungsdialog
/// und wiederholt die Anfrage).
pub fn require_fresh() -> Result<()> {
    if is_fresh() {
        Ok(())
    } else {
        Err(Error::ReauthRequired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fail_closed_without_scope_and_not_inherited_by_spawn() {
        assert!(matches!(require_fresh(), Err(Error::ReauthRequired)));
        with_reauth(true, async {
            assert!(require_fresh().is_ok());
            let spawned = tokio::spawn(async { is_fresh() }).await.unwrap();
            assert!(!spawned, "eigener Task erbt die Bestätigung nicht");
        })
        .await;
        with_reauth(false, async {
            assert!(matches!(require_fresh(), Err(Error::ReauthRequired)));
        })
        .await;
    }
}
