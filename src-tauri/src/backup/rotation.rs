//! Backup-Retention (Block 4 → G1-BKP.4 → Lauf F) — Großvater-Vater-Sohn (GVS)
//! nach **Kalender-Eimern**, getiert.
//!
//! **Kalender-Eimer (Lauf F, D-05, ADR 0040):** Je Ablage behält die Rotation
//! das **jüngste** automatische Backup je Berliner Kalendertag der letzten
//! `keep_daily` Tage *mit Backup*, je Monat der letzten `keep_monthly` Monate
//! und je Jahr der letzten `keep_yearly` Jahre — unabhängig davon, wie viele
//! Lock-Backups an einem Tag entstanden und ob am Monatsersten eines lief.
//! Gezählt werden Eimer mit Backups, nicht Kalendertage ab heute: eine lange
//! Pause oder ein Uhrsprung löscht deshalb nie alle älteren Stände, und das
//! jüngste Backup bleibt immer erhalten.
//!
//! `retention_tag` (`daily`/`monthly`/`yearly`, nach dem Erstellungsdatum)
//! ist nur noch informativ. `manual` — manuelle Backups, `pre_restore` und
//! nach einem Restore übernommene Dateien — wird **nie** gelöscht und zählt
//! für keinen Eimer.
//!
//! **Tier-Modell (G1-BKP.4, ADR 0034):** Es gibt zwei physische Ablagen mit
//! getrennter Aufbewahrung — der **Floor** (immer lokal, `paths.backups_dir`,
//! kurz) und die **Off-Site**-Spiegelung (Ordner-Ziele, lang). Ein Eintrag
//! wird **am `target_path`** zugeordnet: liegt er unter `floor_dir` → Floor,
//! sonst → Off-Site (je Zielordner eine Gruppe). Ist kein Off-Site-Ziel
//! zuletzt erfolgreich, ist der Floor die einzige Kopie und erbt die **lange**
//! Policy.
//!
//! **SFTP (Lauf F, D-06):** Klein.Buch löscht auf SFTP-Zielen nichts — eine
//! kompromittierte Instanz soll die Off-Site-Kopien nicht vernichten können.
//! SFTP-Einträge bleiben deshalb in der Historie (sie behaupten keine Löschung,
//! die nicht stattfand); rotiert wird auf dem SFTP-Server (Handbuch
//! „Backup-Ziel einrichten“, „Speicherbedarf“).
//!
//! Die Pruning-Entscheidung [`plan_deletions`]/[`plan_tiered_deletions`] ist
//! **pure** (testbar mit injizierten Einträgen). [`run`] ist die Imperative
//! Shell, die Dateien löscht und `backup_history` bereinigt.

use crate::error::Result;
use chrono::Datelike;
use sqlx::{Row, SqlitePool};
use std::collections::BTreeSet;
use std::path::Path;

/// Minimaler View auf einen Backup-History-Eintrag für die Pruning-Logik.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRef {
    pub id: String,
    /// UTC (`YYYY-MM-DD HH:MM:SS` oder ISO-8601 mit `T`/`Z`).
    pub created_at: String,
    pub retention_tag: String,
    pub target_path: String,
}

/// Wie viele Kalender-Eimer je Klasse behalten werden.
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
    /// diese Policy, solange **kein** Off-Site-Ziel zuletzt erfolgreich war.
    pub fn offsite() -> Self {
        Self {
            keep_daily: 30,
            keep_monthly: 12,
            keep_yearly: 7,
        }
    }
}

impl Default for RetentionPolicy {
    /// Default = die lange (Off-Site-)Policy.
    fn default() -> Self {
        Self::offsite()
    }
}

/// Bestimmt den (informativen) `retention_tag` eines neuen Backups aus
/// Erstellungsdatum + Trigger-Grund. `manual` schützt vor der Rotation.
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

/// So weit darf ein Eintrag in der Zukunft liegen und trotzdem als Eimer
/// zählen (F1d; wie die Uhr-Toleranz der Pause).
pub const FUTURE_TOLERANCE: chrono::Duration = chrono::Duration::hours(1);

/// Pure: ermittelt, welche Einträge gelöscht werden sollen (Kalender-Eimer,
/// Europe/Berlin). `manual` und Einträge mit unlesbarem Zeitstempel werden
/// nie gelöscht. Behalten wird je Klasse das jüngste Backup der `keep`
/// jüngsten Eimer (Tag, Monat, Jahr); alles andere wird gelöscht.
///
/// Einträge, die mehr als [`FUTURE_TOLERANCE`] nach `now` datiert sind (die
/// Uhr lief beim Erstellen vor), bleiben erhalten, belegen aber keinen Eimer —
/// sonst verdrängten sie nach der Uhrkorrektur echte Stände (F1d). Sobald die
/// echte Zeit sie eingeholt hat, zählen sie normal.
pub fn plan_deletions(
    entries: &[BackupRef],
    policy: &RetentionPolicy,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<BackupRef> {
    let horizon = now + FUTURE_TOLERANCE;
    // (Berliner Zeitpunkt, Eintrag) der rotierbaren Einträge, jüngste zuerst.
    let mut auto: Vec<(chrono::NaiveDateTime, &BackupRef)> = entries
        .iter()
        .filter(|e| e.retention_tag != "manual")
        .filter(|e| crate::clock::db_utc(&e.created_at).is_some_and(|t| t <= horizon))
        .filter_map(|e| crate::clock::db_utc_to_berlin(&e.created_at).map(|b| (b.naive_local(), e)))
        .collect();
    auto.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.id.cmp(&a.1.id)));

    let mut keep: BTreeSet<&str> = BTreeSet::new();
    keep_newest_per_bucket(&auto, policy.keep_daily, &mut keep, |t| {
        (t.year(), t.month(), t.day())
    });
    keep_newest_per_bucket(&auto, policy.keep_monthly, &mut keep, |t| {
        (t.year(), t.month(), 0)
    });
    keep_newest_per_bucket(&auto, policy.keep_yearly, &mut keep, |t| (t.year(), 0, 0));

    auto.iter()
        .filter(|(_, e)| !keep.contains(e.id.as_str()))
        .map(|(_, e)| (*e).clone())
        .collect()
}

/// Behält aus `sorted` (jüngste zuerst) je Eimer den ersten (= jüngsten)
/// Eintrag, für die `limit` jüngsten Eimer.
fn keep_newest_per_bucket<'a>(
    sorted: &[(chrono::NaiveDateTime, &'a BackupRef)],
    limit: usize,
    keep: &mut BTreeSet<&'a str>,
    bucket: impl Fn(&chrono::NaiveDateTime) -> (i32, u32, u32),
) {
    let mut seen: Vec<(i32, u32, u32)> = Vec::new();
    for (t, e) in sorted {
        let b = bucket(t);
        if seen.contains(&b) {
            continue;
        }
        if seen.len() >= limit {
            break;
        }
        seen.push(b);
        keep.insert(e.id.as_str());
    }
}

/// Pure: ist `target_path` eine Datei im lokalen Floor-Ordner? `sftp://`-URIs und
/// andere Verzeichnisse zählen als Off-Site. Komponenten-basiert (`Path::starts_with`),
/// also separator-robust.
pub fn is_floor_path(target_path: &str, floor_dir: &Path) -> bool {
    Path::new(target_path).starts_with(floor_dir)
}

/// Pure: getierte Pruning-Entscheidung. Partitioniert die Einträge nach
/// `target_path` in Floor vs. Off-Site; Off-Site wird je Ziel (enthaltender
/// Ordner, [`crate::backup::target::group_key`]) gruppiert und je Gruppe mit
/// der Off-Site-Policy rotiert (Plan W7, mehrere Ziele). SFTP-Einträge werden
/// nie gelöscht (D-06).
pub fn plan_tiered_deletions(
    entries: &[BackupRef],
    floor_dir: &Path,
    floor_policy: &RetentionPolicy,
    offsite_policy: &RetentionPolicy,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<BackupRef> {
    let (floor, offsite): (Vec<BackupRef>, Vec<BackupRef>) = entries
        .iter()
        .filter(|e| !e.target_path.starts_with("sftp://"))
        .cloned()
        .partition(|e| is_floor_path(&e.target_path, floor_dir));
    let mut out = plan_deletions(&floor, floor_policy, now);
    let mut groups: std::collections::BTreeMap<String, Vec<BackupRef>> = Default::default();
    for e in offsite {
        groups
            .entry(crate::backup::target::group_key(&e.target_path))
            .or_default()
            .push(e);
    }
    for group in groups.values() {
        out.extend(plan_deletions(group, offsite_policy, now));
    }
    out
}

/// Imperative Shell: lädt `backup_history`, plant getierte Löschungen, entfernt
/// die Backup-Dateien und die zugehörigen History-Zeilen. Liefert Anzahl
/// gelöschter Backups. Fehlende Dateien sind kein Fehler (best-effort).
///
/// `floor_dir` = lokaler Floor (`paths.backups_dir`). Ist ein Off-Site-Ziel
/// zuletzt erfolgreich, gilt für den Floor die **kurze** Policy, für Off-Site
/// die **lange**; sonst erbt der Floor die lange Policy (einzige Kopie).
/// SFTP-Einträge plant [`plan_tiered_deletions`] nie ein (D-06).
///
/// Während einer Pause der automatischen Abläufe ruft
/// [`crate::backup::create_with`] die Rotation nicht auf (Lauf F). `now`
/// kommt aus der injizierten Uhr des Backups (zukünftig datierte Einträge,
/// F1d).
pub async fn run(
    pool: &SqlitePool,
    floor_dir: &Path,
    targets_root: &Path,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<usize> {
    // Kurze Floor-Aufbewahrung nur, wenn mindestens ein Ziel zuletzt
    // erfolgreich war — sonst ist der Floor die einzige Kopie
    // (Gegenprüfung W7).
    let targets = crate::backup::target::list_targets(pool, targets_root).await?;
    let results = crate::backup::target::load_results(pool).await?;
    let offsite_configured = targets
        .iter()
        .any(|t| results.get(&t.id).is_some_and(|r| r.ok));
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

    let deletions = plan_tiered_deletions(&entries, floor_dir, &floor_policy, &offsite_policy, now);
    let count = deletions.len();

    for d in &deletions {
        // Lokale Datei (Floor oder Ordner-Ziel) best-effort löschen.
        let _ = std::fs::remove_file(&d.target_path);
        sqlx::query("DELETE FROM backup_history WHERE id = ?")
            .bind(&d.id)
            .execute(pool)
            .await?;
        crate::db::repo::audit_log::append(
            pool,
            "backup.rotation.prune",
            "backup",
            &d.id,
            Some(&serde_json::json!({ "tag": d.retention_tag }).to_string()),
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

    fn ids(v: &[BackupRef]) -> Vec<&str> {
        v.iter().map(|e| e.id.as_str()).collect()
    }

    #[test]
    fn plan_keeps_newest_per_day_month_year() {
        let policy = RetentionPolicy {
            keep_daily: 2,
            keep_monthly: 2,
            keep_yearly: 1,
        };
        let entries = vec![
            mkref("d1", "2026-05-10T02:00:00Z", "daily"),
            mkref("d2", "2026-05-11T02:00:00Z", "daily"),
            mkref("d3", "2026-05-12T02:00:00Z", "daily"), // jüngstes
            mkref("m1", "2026-04-01T02:00:00Z", "monthly"),
            mkref("m0", "2026-03-01T02:00:00Z", "monthly"),
            mkref("y1", "2025-01-01T02:00:00Z", "yearly"),
            mkref("man", "2020-01-02T02:00:00Z", "manual"), // nie löschen
        ];
        let del = plan_deletions(&entries, &policy, far());
        let del_ids = ids(&del);
        // Tage: d3, d2 behalten; Monate: Mai (d3) + April (m1); Jahr: 2026 (d3).
        assert_eq!(del_ids, vec!["d1", "m0", "y1"], "{del_ids:?}");
        assert!(!del_ids.contains(&"man"));
    }

    #[test]
    fn plan_empty_when_within_limits() {
        let entries = vec![
            mkref("d1", "2026-05-10T02:00:00Z", "daily"),
            mkref("m1", "2026-05-01T02:00:00Z", "monthly"),
        ];
        assert!(plan_deletions(&entries, &RetentionPolicy::default(), far()).is_empty());
    }

    /// D-05: Ein Tag mit 20 Lock-Backups verdrängt die Vortage nicht — es
    /// bleiben 7 verschiedene Tage (je das jüngste Backup).
    #[test]
    fn busy_day_does_not_evict_previous_days() {
        let mut entries = Vec::new();
        for day in 1..=10 {
            entries.push(mkref(
                &format!("tag{day:02}"),
                &format!("2026-05-{day:02} 01:00:00"),
                "daily",
            ));
        }
        for i in 0..20 {
            entries.push(mkref(
                &format!("lock{i:02}"),
                &format!("2026-05-11 {:02}:{:02}:00", 6 + i / 4, (i % 4) * 10),
                "daily",
            ));
        }
        let del = plan_deletions(&entries, &RetentionPolicy::floor(), far());
        let kept: Vec<&str> = entries
            .iter()
            .filter(|e| !del.iter().any(|x| x.id == e.id))
            .map(|e| e.id.as_str())
            .collect();
        let days: BTreeSet<String> = entries
            .iter()
            .filter(|e| kept.contains(&e.id.as_str()))
            .map(|e| e.created_at[..10].to_string())
            .collect();
        assert_eq!(days.len(), 7, "7 verschiedene Tage, behalten: {kept:?}");
        // Vom vollen Tag nur das jüngste Lock-Backup.
        assert!(kept.contains(&"lock19"));
        assert_eq!(kept.iter().filter(|k| k.starts_with("lock")).count(), 1);
    }

    /// D-05: Ein Monat ohne Backup am Monatsersten behält trotzdem einen Stand.
    #[test]
    fn month_without_backup_on_first_keeps_one() {
        let entries = vec![
            mkref("mai", "2026-05-20 03:00:00", "daily"),
            mkref("april-a", "2026-04-10 03:00:00", "daily"),
            mkref("april-b", "2026-04-17 03:00:00", "daily"),
            mkref("maerz", "2026-03-05 03:00:00", "daily"),
        ];
        let policy = RetentionPolicy {
            keep_daily: 1,
            keep_monthly: 3,
            keep_yearly: 1,
        };
        let del = plan_deletions(&entries, &policy, far());
        assert_eq!(ids(&del), vec!["april-a"], "je Monat das jüngste");
    }

    /// Eimer zählen in Europe/Berlin: 23:30 UTC am 31.05. ist der 01.06.
    #[test]
    fn buckets_use_berlin_dates() {
        let entries = vec![
            mkref("juni", "2026-05-31 23:30:00", "daily"),
            mkref("mai", "2026-05-31 20:00:00", "daily"),
        ];
        let policy = RetentionPolicy {
            keep_daily: 2,
            keep_monthly: 0,
            keep_yearly: 0,
        };
        // Zwei verschiedene Berliner Tage → beide bleiben.
        assert!(plan_deletions(&entries, &policy, far()).is_empty());
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
        let d = RetentionPolicy::default();
        assert_eq!(d.keep_daily, o.keep_daily);
        assert_eq!(d.keep_yearly, o.keep_yearly);
    }

    #[test]
    fn is_floor_path_classifies() {
        let floor = Path::new("/data/backups");
        assert!(is_floor_path("/data/backups/klein-buch-x.kbk", floor));
        assert!(!is_floor_path("/mnt/onedrive/kb/klein-buch-x.kbk", floor));
        assert!(!is_floor_path(
            "sftp://kb@nas:22/backups/klein-buch-x.kbk",
            floor
        ));
    }

    #[test]
    fn tiered_applies_per_tier_policy_and_never_sftp() {
        let floor = Path::new("/data/backups");
        let floor_pol = RetentionPolicy {
            keep_daily: 1,
            keep_monthly: 0,
            keep_yearly: 0,
        };
        let offsite_pol = RetentionPolicy {
            keep_daily: 2,
            keep_monthly: 0,
            keep_yearly: 0,
        };
        let entries = vec![
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
            mkref_at("o1", "2026-05-10T02:00:00Z", "daily", "/mnt/od/o1.kbk"),
            mkref_at("o2", "2026-05-11T02:00:00Z", "daily", "/mnt/od/o2.kbk"),
            mkref_at("o3", "2026-05-12T02:00:00Z", "daily", "/mnt/od/o3.kbk"),
            // D-06: SFTP wird nie gelöscht, auch weit über der Grenze.
            mkref_at(
                "s1",
                "2026-05-09T02:00:00Z",
                "daily",
                "sftp://kb@nas:22/s1.kbk",
            ),
            mkref_at(
                "s2",
                "2026-05-10T02:00:00Z",
                "daily",
                "sftp://kb@nas:22/s2.kbk",
            ),
            mkref_at(
                "s3",
                "2026-05-11T02:00:00Z",
                "daily",
                "sftp://kb@nas:22/s3.kbk",
            ),
        ];
        let mut del: Vec<String> =
            plan_tiered_deletions(&entries, floor, &floor_pol, &offsite_pol, far())
                .iter()
                .map(|e| e.id.clone())
                .collect();
        del.sort();
        assert_eq!(del, vec!["f1", "f2", "o1"]);
    }

    /// G1-HARDEN.5: Das **global neueste** Backup darf NIE gelöscht werden.
    #[test]
    fn never_deletes_global_newest() {
        let floor = Path::new("/data/backups");
        let mut entries = Vec::new();
        for day in 1..=28 {
            entries.push(mkref_at(
                &format!("f{day}"),
                &format!("2026-04-{day:02}T02:00:00Z"),
                "daily",
                &format!("/data/backups/f{day}.kbk"),
            ));
        }
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
            far(),
        );
        let del_ids = ids(&del);
        assert!(!del_ids.contains(&"newest"), "{del_ids:?}");
        let kept = entries.len() - del.len();
        assert!(kept >= RetentionPolicy::floor().keep_daily, "{kept}");
    }

    /// Eine lange Pause (Eimer mit Backups zählen, nicht Tage ab heute) lässt
    /// die letzten Stände stehen; unlesbare Zeitstempel werden nie gelöscht.
    #[test]
    fn long_gap_keeps_last_states_and_unparsable_stays() {
        let mut entries = Vec::new();
        for day in 1..=10 {
            entries.push(mkref(
                &format!("alt{day:02}"),
                &format!("2024-02-{day:02} 02:30:00"),
                "daily",
            ));
        }
        entries.push(mkref("kaputt", "gestern", "daily"));
        let del = plan_deletions(&entries, &RetentionPolicy::floor(), far());
        assert_eq!(entries.len() - del.len(), 8, "7 Tage + unlesbarer Eintrag");
        assert!(!ids(&del).contains(&"kaputt"));
        assert!(!ids(&del).contains(&"alt10"));
    }

    /// F1d: Backups aus einer Zeit, in der die Uhr vorlief, bleiben erhalten,
    /// belegen aber keine Eimer — die echten Tagesstände bleiben vollständig.
    #[test]
    fn future_dated_backups_do_not_take_buckets() {
        let mut entries = Vec::new();
        for day in 1..=10 {
            entries.push(mkref(
                &format!("echt{day:02}"),
                &format!("2026-10-{day:02} 02:30:00"),
                "daily",
            ));
        }
        for day in 1..=5 {
            entries.push(mkref(
                &format!("zukunft{day}"),
                &format!("2027-03-{day:02} 02:30:00"),
                "daily",
            ));
        }
        let now = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 10, 11, 12, 0, 0).unwrap();
        let del = plan_deletions(&entries, &RetentionPolicy::floor(), now);
        let del_ids = ids(&del);
        assert!(
            !del_ids.iter().any(|i| i.starts_with("zukunft")),
            "zukünftige bleiben: {del_ids:?}"
        );
        // 7 echte Tage bleiben (04.–10.), 01.–03. gehen.
        assert_eq!(del_ids, vec!["echt03", "echt02", "echt01"], "{del_ids:?}");
        // Hat die echte Zeit sie eingeholt, zählen sie normal.
        let later = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2027, 3, 6, 12, 0, 0).unwrap();
        let del = plan_deletions(&entries, &RetentionPolicy::floor(), later);
        assert!(!ids(&del).contains(&"zukunft5"));
        assert!(ids(&del).contains(&"echt01"));
    }

    /// Weit in der Zukunft: Für die übrigen Tests spielt der Horizont keine
    /// Rolle.
    fn far() -> chrono::DateTime<chrono::Utc> {
        chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2100, 1, 1, 0, 0, 0).unwrap()
    }
}
