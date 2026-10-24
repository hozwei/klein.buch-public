//! Plan W5.5 — Start fail-closed: Prüfungen vor dem Port-Bind.
//!
//! Fälle: leeres `/data` → `setup`; Marker + DB → `locked`; DB ohne Marker,
//! Marker ohne DB, Archiv ohne beides, Klartext-DB, wirkungsloses `chmod`,
//! kein Mountpoint → Start verweigert. Zusätzlich über das Binary: Exit ≠ 0,
//! bevor der Port gebunden wird.

mod server_support;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use klein_buch_lib::server::config::StartupOptions;
use klein_buch_lib::server::instance;
use klein_buch_lib::server::startup;
use klein_buch_lib::server::state::InstanceState;
use server_support::*;

fn prepare_in(
    root: &Path,
) -> klein_buch_lib::error::Result<std::sync::Arc<klein_buch_lib::server::state::ServerState>> {
    let paths = prepare_dirs(root);
    klein_buch_lib::server::prepare(ctx_for(paths), &local_config())
}

fn err_text(root: &Path) -> String {
    match prepare_in(root) {
        Ok(_) => panic!("Start hätte verweigert werden müssen"),
        Err(e) => e.to_string(),
    }
}

/// Echte SQLCipher-DB (wie nach der Einrichtung) unter `root/data`.
async fn encrypted_db(root: &Path) {
    let paths = klein_buch_lib::config::Paths::under(root);
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    let pool = klein_buch_lib::db::open_pool(&paths.db_file, Some(PASS))
        .await
        .unwrap();
    klein_buch_lib::db::MIGRATOR.run(&pool).await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn empty_data_is_setup_marker_plus_db_is_locked() {
    let dir = tempfile::tempdir().unwrap();
    let st = prepare_in(dir.path()).unwrap();
    assert_eq!(st.instance_state(), InstanceState::Setup);

    let dir = tempfile::tempdir().unwrap();
    encrypted_db(dir.path()).await;
    instance::write_marker(&dir.path().join("data")).unwrap();
    let st = prepare_in(dir.path()).unwrap();
    assert_eq!(st.instance_state(), InstanceState::Locked);
    let marker = instance::read_marker(&dir.path().join("data"))
        .unwrap()
        .unwrap();
    assert!(!marker.instance_id.is_empty());
}

#[tokio::test]
async fn db_without_marker_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    encrypted_db(dir.path()).await;
    let msg = err_text(dir.path());
    assert!(msg.contains("ohne Instanz-Marker"), "{msg}");
}

#[test]
fn marker_without_db_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    instance::write_marker(&data).unwrap();
    let msg = err_text(dir.path());
    assert!(msg.contains("ohne Datenbank"), "{msg}");
}

#[test]
fn archive_without_db_and_marker_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("data").join("archive").join("2026");
    std::fs::create_dir_all(&archive).unwrap();
    std::fs::write(archive.join("RE-2026-0001.pdf"), b"%PDF").unwrap();
    let msg = err_text(dir.path());
    assert!(
        msg.contains("keine Neueinrichtung über einem Archiv"),
        "{msg}"
    );
}

#[tokio::test]
async fn plaintext_db_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let paths = klein_buch_lib::config::Paths::under(dir.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    let pool = klein_buch_lib::db::open_pool(&paths.db_file, None)
        .await
        .unwrap();
    klein_buch_lib::db::MIGRATOR.run(&pool).await.unwrap();
    pool.close().await;
    instance::write_marker(&paths.data_dir).unwrap();
    let msg = err_text(dir.path());
    assert!(msg.contains("unverschlüsselt"), "{msg}");
}

fn chmod_noop(_: &Path) -> std::io::Result<()> {
    Ok(())
}

fn chmod_fails(_: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
}

#[test]
fn archive_selftest_refuses_ineffective_chmod() {
    let dir = tempfile::tempdir().unwrap();
    let msg = startup::archive_selftest_with(dir.path(), chmod_noop)
        .unwrap_err()
        .to_string();
    assert!(msg.contains("Selbsttest"), "{msg}");
    let msg = startup::archive_selftest_with(dir.path(), chmod_fails)
        .unwrap_err()
        .to_string();
    assert!(msg.contains("Schreibschutz nicht setzbar"), "{msg}");
    // keine Reste
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn unmounted_volumes_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let paths = prepare_dirs(dir.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    std::fs::create_dir_all(&paths.inputs_dir).unwrap();
    let mut cfg = local_config();
    cfg.startup = StartupOptions {
        allow_unmounted: false,
        mountinfo: Some(format!(
            "36 35 98:0 / {} rw - ext4 /dev/sda1 rw\n",
            paths.data_dir.canonicalize().unwrap().display()
        )),
    };
    let msg = klein_buch_lib::server::prepare(ctx_for(paths.clone()), &cfg)
        .err()
        .unwrap()
        .to_string();
    assert!(
        msg.contains("Inputs-Verzeichnis") && msg.contains("kein eingebundenes Volume"),
        "{msg}"
    );

    cfg.startup.mountinfo = Some(format!(
        "36 35 98:0 / {} rw - ext4 /dev/sda1 rw\n37 35 98:0 / {} rw - ext4 /dev/sda2 rw\n",
        paths.data_dir.canonicalize().unwrap().display(),
        paths.inputs_dir.canonicalize().unwrap().display()
    ));
    assert!(klein_buch_lib::server::prepare(ctx_for(paths), &cfg).is_ok());
}

#[test]
fn leftovers_in_tmp_are_removed() {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().join("data").join("tmp");
    std::fs::create_dir_all(tmp.join("restore-upload")).unwrap();
    std::fs::write(tmp.join("snapshot-1.db"), b"x").unwrap();
    prepare_in(dir.path()).unwrap();
    assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);
}

/// Freier Port auf Loopback.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Das Binary bricht bei DB ohne Marker mit Exit ≠ 0 ab, ohne den Port zu
/// binden.
#[tokio::test]
async fn binary_exits_nonzero_before_bind() {
    let dir = tempfile::tempdir().unwrap();
    let paths = prepare_dirs(dir.path());
    encrypted_db(dir.path()).await;
    let port = free_port();
    let out = Command::new(env!("CARGO_BIN_EXE_klein-buch-server"))
        .env("KLEIN_BUCH_DATA_DIR", &paths.data_dir)
        .env("KLEIN_BUCH_INPUTS_DIR", &paths.inputs_dir)
        .env("KLEIN_BUCH_RESOURCE_DIR", &paths.resource_dir)
        .env("KLEIN_BUCH_WEB_DIR", &paths.web_dir)
        .env("KLEIN_BUCH_LISTEN", format!("127.0.0.1:{port}"))
        .env("KLEIN_BUCH_PUBLIC_URL", format!("http://127.0.0.1:{port}"))
        .env("KLEIN_BUCH_ALLOW_UNMOUNTED", "1")
        .env_remove("KLEIN_BUCH_SIDECAR_MOCK")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let log =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    assert!(log.contains("ohne Instanz-Marker"), "{log}");
    assert!(!log.contains("lauscht auf"), "{log}");
    assert!(
        std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(200)
        )
        .is_err(),
        "Port darf nicht gebunden sein"
    );
}

/// Fehlt eine Pflicht-Variable, startet das Binary nicht.
#[test]
fn binary_requires_paths_from_env() {
    let out = Command::new(env!("CARGO_BIN_EXE_klein-buch-server"))
        .env_remove("KLEIN_BUCH_DATA_DIR")
        .env("KLEIN_BUCH_LISTEN", "127.0.0.1:0")
        .env_remove("KLEIN_BUCH_SIDECAR_MOCK")
        .output()
        .unwrap();
    assert!(!out.status.success());
}

/// E7: Ein vorgemerkter Factory Reset wird im Server nie angewendet — der Start
/// bricht ab, DB, Archiv und Marker bleiben unverändert.
#[tokio::test]
async fn factory_reset_marker_is_refused_not_applied() {
    let dir = tempfile::tempdir().unwrap();
    encrypted_db(dir.path()).await;
    let data = dir.path().join("data");
    instance::write_marker(&data).unwrap();
    let archive = data.join("archive").join("2026");
    std::fs::create_dir_all(&archive).unwrap();
    std::fs::write(archive.join("RE-2026-0001.pdf"), b"%PDF").unwrap();
    std::fs::write(data.join("FACTORY_RESET_PENDING.json"), b"{}").unwrap();

    let msg = err_text(dir.path());
    assert!(
        msg.contains("Factory Reset ist im Server-Betrieb abgeschaltet"),
        "{msg}"
    );
    assert!(data.join("klein-buch.sqlite").exists(), "DB unverändert");
    assert!(
        archive.join("RE-2026-0001.pdf").exists(),
        "Archiv unverändert"
    );
    assert!(instance::marker_path(&data).exists(), "Marker unverändert");
}

/// Ein zweiter Prozess auf demselben Datenverzeichnis startet nicht.
#[test]
fn second_instance_on_same_data_dir_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let first = prepare_in(dir.path()).unwrap();
    let msg = err_text(dir.path());
    assert!(msg.contains("läuft bereits"), "{msg}");
    drop(first);
    assert!(
        prepare_in(dir.path()).is_ok(),
        "nach Ende des ersten wieder frei"
    );
}
