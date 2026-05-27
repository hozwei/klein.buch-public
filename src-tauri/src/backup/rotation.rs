//! Backup-Retention (Block 4 → G1-BKP.4) — Großvater-Vater-Sohn (GVS), getiert.
//!
//! Jedes Backup wird bei Erstellung mit **genau einem** `retention_tag` versehen:
//!
//! - `yearly`  — erstellt am 01.01.
//! - `monthly` — erstellt am 1. eines (anderen) Monats
//! - `daily`   — sonst
//! - `manual`  — manuelle Backups **und** `pre_restore`-Backups → nie auto-geprunt
//!
//! **Tier-Modell (G1-BKP.4, ADR 0034):** Es gibt zwei physische Ablagen mit
//! getrennter Aufbewahrung — der **Floor** (immer lokal, `paths.backups_dir`,
//! kurz) und die **Off-Site**-Spiegelung (gewähltes Ziel, lang). Da das Schema
//! keine Tier-Spalte hat (kein Migrations-Aufwand, schema v25 bleibt), wird ein
//! Eintrag **am `target_path`** zugeordnet: liegt er unter `floor_dir` → Floor,
//! sonst (anderer Ordner oder `sftp://`-URI) → Off-Site. Ist **kein** Off-Site
//! konfiguriert, ist der Floor die einzige Kopie und erbt die **lange** Policy
//! (kein Verlust für Nutzer ohne Off-Site).
//!
//! Die Pruning-Entscheidung [`plan_deletions`]/[`plan_tiered_deletions`] ist
//! **pure** (testbar mit injizierten Einträgen). [`run`] ist die Imperative
//! Shell, die Dateien löscht und `backup_history` bereinigt.

use crate::error::Result;
use chrono::Datelike;
use sqlx::{Row, SqlitePool};
use std::path::Path;

/// Minimaler View auf einen Backup-History-Eintrag für die Pruning-Logik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRef {
    pub id: String,
    /// ISO-8601-UTC — sortiert lexikographisch == chronologisch.
    pub created_at: String,
    pub retention_tag: String,
    pub target_path: String,
}

/// Wie viele Backups je Klasse behalten werden.
#[derive(Debug, Clone, Copy)]
pub struct RetentionPolicy {
    pub keep_daily: usize,
    pub keep_monthly: usize,
    pub keep_yearly: usize,
}

impl RetentionPolicy {
    /// **Floor** (lokal, kurz, G1-BKP.4): Sicherheitsboden, der Off-Site-Ausfälle
    /// überbrückt — nicht das Langzeit-Archiv. ADR 0034 nennt „kurz (z. B. 7)".
    pub fn floor() -> Self {
        Self {
            keep_daily: 7,
            keep_monthly: 3,
            keep_yearly: 1,
        }
    }

    /// **Off-Site / Langzeit** (lang, G1-BKP.4): das eigentliche Aufbewahrungs-
    /// Archiv (GoBD-Geist) — 30 Tage / 12 Monate / 7 Jahre. Auch der Floor erbt
    /// diese Policy, solange **kein** Off-Site-Ziel konfiguriert ist.
    pub fn offsite() -> Self {
        Self {
            keep_daily: 30,
            keep_monthly: 12,
            keep_yearly: 7,
        }
    }
}

impl Default for RetentionPolicy {
    /// Default = die lange (Off-Site-)Policy. Rückwärtskompatibel zum bisherigen
    /// 30/12/7-Verhalten.
    fn default() -> Self {
        Self::offsite()
    }
}

/// Bestimmt den `retention_tag` eines neuen Backups aus Erstellungsdatum +
/// Trigger-Grund.
pub fn classify_retention(date: chrono::NaiveDate, trigger_reason: &str) -> &'static str {
    match trigger_reason {
        "manual" | "pre_restore" => "manual",
        _ => {
            if date.month() == 1 && date.day() == 1 {
                "yearly"
            } else if date.day() == 1 {
                "monthly"
            } else {
                "daily"
            }
        }
    }
}

/// Pure: ermittelt, welche Einträge gelöscht werden sollen. `manual` wird nie
/// geprunt. Pro Klasse werden die `keep` neuesten behalten, der Rest gelöscht.
pub fn plan_deletions(entries: &[BackupRef], policy: &RetentionPolicy) -> Vec<BackupRef> {
    let mut to_delete = Vec::new();
    for (tag, keep) in [
        ("daily", policy.keep_daily),
        ("monthly", policy.keep_monthly),
        ("yearly", policy.keep_yearly),
    ] {
        let mut group: Vec<&BackupRef> =
            entries.iter().filter(|e| e.retention_tag == tag).collect();
        // Neueste zuerst.
        group.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        for e in group.into_iter().skip(keep) {
            to_delete.push(e.clone());
        }
    }
    to_delete
}

/// Pure: ist `target_path` eine Datei im lokalen Floor-Ordner? `sftp://`-URIs und
/// andere Verzeichnisse zählen als Off-Site. Komponenten-basiert (`Path::starts_with`),
/// also separator-robust.
pub fn is_floor_path(target_path: &str, floor_dir: &Path) -> bool {
    Path::new(target_path).starts_with(floor_dir)
}

/// Pure: getierte Pruning-Entscheidung. Partitioniert die Einträge nach
/// `target_path` in Floor vs. Off-Site und wendet je Tier die passende Policy an.
pub fn plan_tiered_deletions(
    entries: &[BackupRef],
    floor_dir: &Path,
    floor_policy: &RetentionPolicy,
    offsite_policy: &RetentionPolicy,
) -> Vec<BackupRef> {
    let (floor, offsite): (Vec<BackupRef>, Vec<BackupRef>) = entries
        .iter()
        .cloned()
        .partition(|e| is_floor_path(&e.target_path, floor_dir));
    let mut out = plan_deletions(&floor, floor_policy);
    out.extend(plan_deletions(&offsite, offsite_policy));
    out
}

/// Imperative Shell: lädt `backup_history`, plant getierte Löschungen, entfernt
/// die Backup-Dateien und die zugehörigen History-Zeilen. Liefert Anzahl
/// gelöschter Backups. Fehlende Dateien sind kein Fehler (best-effort).
///
/// `floor_dir` = lokaler Floor (`paths.backups_dir`). Ist ein Off-Site-Ziel
/// konfiguriert, gilt für den Floor die **kurze** Policy, für Off-Site die
/// **lange**; ohne Off-Site erbt der Floor die lange Policy (einzige Kopie).
/// Off-Site-`sftp://`-Einträge werden nur aus der History entfernt — die
/// Remote-Datei wird **nicht** gelöscht (kein Netzwerk-Delete nach jedem Lock;
/// der eigene Server wird vom Nutzer verwaltet, siehe ADR 0034).
pub async fn run(pool: &SqlitePool, floor_dir: &Path) -> Result<usize> {
    let offsite_configured = crate::backup::target::offsite_target(pool, floor_dir)
        .await?
        .is_some();
    let floor_policy = if offsite_configured {
        RetentionPolicy::floor()
    } else {
        RetentionPolicy::offsite()
    };
    let offsite_policy = RetentionPolicy::offsite();

    let rows = sqlx::query("SELECT id, created_at, retention_tag, target_path FROM backup_history")
        .fetch_all(pool)
        .await?;

    let entries: Vec<BackupRef> = rows
        .into_iter()
        .map(|r| BackupRef {
            id: r.get("id"),
            created_at: r.get("created_at"),
            retention_tag: r.get("retention_tag"),
            target_path: r.get("target_path"),
        })
        .collect();

    let deletions = plan_tiered_deletions(&entries, floor_dir, &floor_policy, &offsite_policy);
    let count = deletions.len();

    for d in &deletions {
        // Lokale Datei best-effort löschen. Off-Site-SFTP (`sftp://`) wird nur aus
        // der History entfernt (keine Remote-Löschung pro Lock).
        if !d.target_path.starts_with("sftp://") {
            let _ = std::fs::remove_file(&d.target_path);
        }
        sqlx::query("DELETE FROM backup_history WHERE id = ?")
            .bind(&d.id)
            .execute(pool)
            .await?;
        crate::db::repo::audit_log::append(
            pool,
            "backup.rotation.prune",
            "backup",
            &d.id,
            Some(&format!(r#"{{"tag":"{}"}}"#, d.retention_tag)),
        )
        .await
        .ok();
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(date: &str) -> NaiveDate {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn classify_picks_yearly_on_jan_first() {
        assert_eq!(classify_retention(d("2026-01-01"), "auto_daily"), "yearly");
        assert_eq!(classify_retention(d("2026-03-01"), "auto_daily"), "monthly");
        assert_eq!(
            classify_retention(d("2026-03-15"), "auto_critical"),
            "daily"
        );
        assert_eq!(classify_retention(d("2026-01-01"), "manual"), "manual");
        assert_eq!(classify_retention(d("2026-01-01"), "pre_restore"), "manual");
    }

    fn mkref(id: &str, created: &str, tag: &str) -> BackupRef {
        BackupRef {
            id: id.into(),
            created_at: created.into(),
            retention_tag: tag.into(),
            target_path: format!("/tmp/{id}.kbk"),
        }
    }

    #[test]
    fn plan_keeps_newest_n_per_class() {
        let policy = RetentionPolicy {
            keep_daily: 2,
            keep_monthly: 1,
            keep_yearly: 1,
        };
        let entries = vec![
            mkref("d1", "2026-05-10T02:00:00Z", "daily"),
            mkref("d2", "2026-05-11T02:00:00Z", "daily"),
            mkref("d3", "2026-05-12T02:00:00Z", "daily"), // newest daily
            mkref("m1", "2026-04-01T02:00:00Z", "monthly"),
            mkref("m2", "2026-05-01T02:00:00Z", "monthly"), // newest monthly
            mkref("y1", "2025-01-01T02:00:00Z", "yearly"),
            mkref("man", "2020-01-02T02:00:00Z", "manual"), // never pruned
        ];
        let del = plan_deletions(&entries, &policy);
        let del_ids: Vec<&str> = del.iter().map(|e| e.id.as_str()).collect();

        // daily: keep d3,d2 → delete d1. monthly: keep m2 → delete m1.
        assert!(del_ids.contains(&"d1"));
        assert!(del_ids.contains(&"m1"));
        // behalten:
        assert!(!del_ids.contains(&"d2"));
        assert!(!del_ids.contains(&"d3"));
        assert!(!del_ids.contains(&"m2"));
        assert!(!del_ids.contains(&"y1"));
        // manual nie löschen:
        assert!(!del_ids.contains(&"man"));
        assert_eq!(del.len(), 2);
    }

    #[test]
    fn plan_empty_when_within_limits() {
        let entries = vec![
            mkref("d1", "2026-05-10T02:00:00Z", "daily"),
            mkref("m1", "2026-05-01T02:00:00Z", "monthly"),
        ];
        assert!(plan_deletions(&entries, &RetentionPolicy::default()).is_empty());
    }

    // ---- Tier-Modell (G1-BKP.4) ------------------------------------------

    fn mkref_at(id: &str, created: &str, tag: &str, path: &str) -> BackupRef {
        BackupRef {
            id: id.into(),
            created_at: created.into(),
            retention_tag: tag.into(),
            target_path: path.into(),
        }
    }

    #[test]
    fn policy_floor_is_shorter_than_offsite() {
        let f = RetentionPolicy::floor();
        let o = RetentionPolicy::offsite();
        assert!(f.keep_daily < o.keep_daily);
        assert!(f.keep_monthly < o.keep_monthly);
        assert!(f.keep_yearly < o.keep_yearly);
        // Default bleibt die lange Policy (Rückwärtskompatibilität).
        let d = RetentionPolicy::default();
        assert_eq!(d.keep_daily, o.keep_daily);
        assert_eq!(d.keep_yearly, o.keep_yearly);
    }

    #[test]
    fn is_floor_path_classifies() {
        let floor = Path::new("/data/backups");
        assert!(is_floor_path("/data/backups/klein-buch-x.kbk", floor));
        // Anderer Ordner = Off-Site.
        assert!(!is_floor_path("/mnt/onedrive/kb/klein-buch-x.kbk", floor));
        // SFTP-URI = Off-Site.
        assert!(!is_floor_path(
            "sftp://kb@nas:22/backups/klein-buch-x.kbk",
            floor
        ));
    }

    #[test]
    fn tiered_applies_per_tier_policy() {
        let floor = Path::new("/data/backups");
        let floor_pol = RetentionPolicy {
            keep_daily: 1,
            keep_monthly: 5,
            keep_yearly: 5,
        };
        let offsite_pol = RetentionPolicy {
            keep_daily: 2,
            keep_monthly: 5,
            keep_yearly: 5,
        };
        let entries = vec![
            // Floor-Dailies: keep 1 → die 2 ältesten löschen.
            mkref_at(
                "f1",
                "2026-05-10T02:00:00Z",
                "daily",
                "/data/backups/f1.kbk",
            ),
            mkref_at(
                "f2",
                "2026-05-11T02:00:00Z",
                "daily",
                "/data/backups/f2.kbk",
            ),
            mkref_at(
                "f3",
                "2026-05-12T02:00:00Z",
                "daily",
                "/data/backups/f3.kbk",
            ),
            // Off-Site-Dailies (Ordner): keep 2 → die 1 älteste löschen.
            mkref_at("o1", "2026-05-10T02:00:00Z", "daily", "/mnt/od/o1.kbk"),
            mkref_at("o2", "2026-05-11T02:00:00Z", "daily", "/mnt/od/o2.kbk"),
            mkref_at("o3", "2026-05-12T02:00:00Z", "daily", "/mnt/od/o3.kbk"),
            // Off-Site-SFTP zählt ebenfalls als Off-Site.
            mkref_at(
                "s1",
                "2026-05-09T02:00:00Z",
                "daily",
                "sftp://kb@nas:22/s1.kbk",
            ),
        ];
        let del: Vec<String> = plan_tiered_deletions(&entries, floor, &floor_pol, &offsite_pol)
            .iter()
            .map(|e| e.id.clone())
            .collect();

        // Floor: behalte f3 → lösche f1, f2.
        assert!(del.contains(&"f1".to_string()));
        assert!(del.contains(&"f2".to_string()));
        assert!(!del.contains(&"f3".to_string()));
        // Off-Site (4 Dailies: s1<o1<o2<o3), keep 2 → behalte o2,o3; lösche o1,s1.
        assert!(del.contains(&"o1".to_string()));
        assert!(del.contains(&"s1".to_string()));
        assert!(!del.contains(&"o2".to_string()));
        assert!(!del.contains(&"o3".to_string()));
        assert_eq!(del.len(), 4);
    }

    // ---- G1-HARDEN.5: Mindestzahl + neuestes Backup nie löschen --------------

    /// Das **global neueste** Backup darf NIE in der Lösch-Liste landen — sonst
    /// könnte die Rotation den einzig aktuellen Wiederherstellungspunkt kappen.
    /// Gilt für beide realen Policies (Floor kurz, Off-Site lang) bei einer
    /// Datenmenge weit über den Keep-Grenzen.
    #[test]
    fn never_deletes_global_newest() {
        let floor = Path::new("/data/backups");
        // 60 tägliche Floor-Backups (Keep-Daily Floor = 7) — weit über der Grenze.
        let mut entries = Vec::new();
        for day in 1..=28 {
            entries.push(mkref_at(
                &format!("f{day}"),
                &format!("2026-04-{day:02}T02:00:00Z"),
                "daily",
                &format!("/data/backups/f{day}.kbk"),
            ));
        }
        // Newest overall (Mai > April).
        entries.push(mkref_at(
            "newest",
            "2026-05-20T02:00:00Z",
            "daily",
            "/data/backups/newest.kbk",
        ));

        let del = plan_tiered_deletions(
            &entries,
            floor,
            &RetentionPolicy::floor(),
            &RetentionPolicy::offsite(),
        );
        let del_ids: Vec<&str> = del.iter().map(|e| e.id.as_str()).collect();
        assert!(
            !del_ids.contains(&"newest"),
            "das neueste Backup darf nie gelöscht werden, war aber in {del_ids:?}"
        );
        // Es bleibt mindestens die garantierte Mindestzahl (Floor keep_daily = 7) übrig.
        let kept = entries.len() - del.len();
        assert!(
            kept >= RetentionPolicy::floor().keep_daily,
            "es müssen mind. {} Backups überleben, es waren {kept}",
            RetentionPolicy::floor().keep_daily
        );
    }

    /// Pruning behält pro Klasse exakt die `keep` neuesten — die garantierte
    /// Mindestzahl bleibt also auch bei sehr vielen Einträgen erhalten.
    #[test]
    fn keeps_exactly_minimum_per_class() {
        let policy = RetentionPolicy {
            keep_daily: 7,
            keep_monthly: 3,
            keep_yearly: 1,
        };
        let mut entries = Vec::new();
        for i in 1..=50 {
            entries.push(mkref(
                &format!("d{i}"),
                &format!("2026-{:02}-{:02}T02:00:00Z", (i % 12) + 1, (i % 27) + 1),
                "daily",
            ));
        }
        let del = plan_deletions(&entries, &policy);
        let kept = entries.len() - del.len();
        assert_eq!(
            kept, policy.keep_daily,
            "es müssen genau keep_daily bleiben"
        );
    }
}
