//! Lastgrenzen für teure Arbeit (Plan W2.6; Werte laut Plan W5.8).
//!
//! Typst-Render, JVM-Starts (KoSIT/Mustang) und die Passphrase-KDF laufen
//! außerhalb der async-Runtime (`spawn_blocking` bzw. eigener Prozess) und sind
//! hier nach oben begrenzt, damit parallele Anfragen den Server nicht
//! auslasten. Der HTTP-Dispatcher (W5) nutzt dieselben Semaphoren.

use tokio::sync::Semaphore;

/// Gleichzeitige Typst-Renders.
pub static RENDER: Semaphore = Semaphore::const_new(2);
/// Gleichzeitige JVM-Starts (KoSIT, Mustang).
pub static JVM: Semaphore = Semaphore::const_new(2);
/// Gleichzeitige Passphrase-Ableitungen (Argon2id, 64 MiB je Lauf).
pub static KDF: Semaphore = Semaphore::const_new(1);

/// Führt `f` in `spawn_blocking` aus, nachdem eine Genehmigung von `limit`
/// vorliegt. Ein Abbruch des Blocking-Tasks wird zu `Error::Other`.
pub async fn run_blocking<T, F>(limit: &'static Semaphore, f: F) -> crate::error::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> crate::error::Result<T> + Send + 'static,
{
    let _permit = limit
        .acquire()
        .await
        .map_err(|e| crate::error::Error::Other(anyhow::anyhow!("Semaphore geschlossen: {e}")))?;
    tokio::task::spawn_blocking(f).await.map_err(|e| {
        crate::error::Error::Other(anyhow::anyhow!("Hintergrund-Task abgebrochen: {e}"))
    })?
}
