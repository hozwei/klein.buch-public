//! `klein-buch-server` (Plan W5, ADR 0039): HTTP-Server bzw. Subcommands.
//!
//! - ohne Argument oder `serve`: Server starten (Konfiguration aus der Umgebung)
//! - `healthcheck`: fragt `/healthz` über Loopback ab (Docker-`HEALTHCHECK`)
//! - `version`: Version ausgeben

fn main() -> std::process::ExitCode {
    klein_buch_lib::server::main_entry(std::env::args_os().skip(1).collect())
}
