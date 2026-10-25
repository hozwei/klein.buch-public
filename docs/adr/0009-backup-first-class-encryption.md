# ADR 0009 — Backup als First-Class-Feature (Argon2id + AES-256-GCM)

**Status:** Akzeptiert · 2026-05-20 · Block 4. (Decision-Log D-28, D-29)

## Kontext

10-Jahres-Aufbewahrung (GoBD/AO) ohne verlässliches Backup ist nicht haltbar. Die
DB kann in OneDrive liegen — unverschlüsselte Steuerdaten dort wären fahrlässig.
Ein Restore darf nie unbemerkt Daten zerstören.

## Entscheidung

- **Verschlüsseltes Backup**: Argon2id (m=64 MB, t=3, p=4) leitet den Schlüssel
  aus der Passphrase ab; Body mit **AES-256-GCM**, frische Salt/Nonce pro Backup.
- **Passphrase-Setup im Onboarding erzwungen** (vor der ersten Rechnung).
- **Auto-Backup** bei jedem `lock`-Event (Rechnung, Storno) + täglich beim Start.
- **Rotation** GVS (30 daily / 12 monthly / 7 yearly); `manual`/`pre_restore` nie
  geprunt.
- **Pre-Restore-Backup Pflicht**; Restore zweiphasig (Staging + Swap beim Start,
  Windows-File-Lock). `inputs/` bleibt beim Restore tabu.
- **Passphrase niemals** in DB/Logs/Audit; Setup/Unlock über verschlüsselten
  Verifier (kein Klartext/Hash).

## Konsequenzen

- Steuerdaten sind auch in der Cloud-Sync-Ablage verschlüsselt.
- Verlust der Passphrase = Verlust der Backups (bewusster Trade-off; im UI klar
  kommuniziert).
- Restore ist sicher gegen Teil-/Fehlanwendung (Pre-Restore-Snapshot + Atomic-Swap).

## Referenzen

`backup::{manifest,encrypt,snapshot,rotation,restore}`; Block-4-Notes.

## Amendment 2026-10-23 — Web-/Docker-Betrieb (ADR 0039)

**Status:** Akzeptiert · 2026-10-23 · Phase W (ADR 0039; Blöcke W1, W2, W7). Krypto
(Argon2id + AES-256-GCM), Rotation und Pre-Restore-Pflicht bleiben unverändert.

- **Backup-`FORMAT_VERSION` 2:** Der Content enthält `branding/` aus `data_dir` (hochgeladenes
  Logo, Unterschrift); Format 1 sicherte stattdessen `inputs/branding/`. Aus `inputs/` wird
  nichts mehr gesichert: `/inputs` ist im Container ein eigenes Volume mit menschen-gepflegten
  Vorlagen, das der Betreiber selbst sichert. Restore nimmt Format 1 und 2 an; bei Format 1
  bleibt `data/branding` unverändert, der Restore-Report sagt das.
- **Konsistenter Snapshot:** Die DB wird per `VACUUM INTO` über eine eigene Verbindung kopiert
  (verschlüsselt wie das Original) statt per Checkpoint und Dateikopie; Temp-Dateien liegen
  unter `<data_dir>/tmp`. Das tägliche Backup läuft im Scheduler-Tick, nicht nur beim Start.
- **Restore älterer Backups (Block W7):** Statt exakt gleicher Schema-Version nimmt der
  Restore Backups mit `30 ≤ schema_version ≤` erwarteter Version an und migriert danach
  vorwärts; Backups einer neueren Version bleiben abgelehnt (Down-Migration-Schutz). Die
  Untergrenze 30 begründet ADR 0039.
- **Pflicht-Backup vor jeder Migration** (ADR 0039, E15): Beim Entsperren mit älterem Schema
  und nach dem Restore eines älteren Backups entsteht vorher ein verifiziertes Backup
  (Auslöser `manual`, wird nie rotiert); scheitert es, wird nicht migriert.

## Amendment 2026-10-24 — Rotation nach Kalender-Eimern (Lauf F, ADR 0040)

Die Aufbewahrung 30/12/7 zählt seit Lauf F Kalender-Eimer (jüngstes Backup je
Berliner Tag, Monat, Jahr) statt Backups je Klasse; der Floor behält bei
erfolgreichem Off-Site-Ziel 7/3/1. Das Tages-Backup entsteht einmal je
Kalendertag zu einer festen Uhrzeit. Auf SFTP-Zielen löscht Klein.Buch nichts.
Einzelheiten: ADR 0034 (Amendment 2026-10-24), ADR 0040 Nr. 15 und 16.

## Amendment 2026-10-25 — Backup-Format 3 (Lauf F3a, ADR 0039 Nr. 127)

Argon2id + AES-256-GCM bleiben. Neu ist **Format 3**: Der Klartext-Kopf (Magic und
Manifest) ist als *Associated Data* an AES-GCM gebunden — ein veränderter Kopf, auch das
Herabsetzen auf Format 2, lässt die Entschlüsselung scheitern. Inhalt wie Format 2; der
Restore nimmt Format 1, 2 und 3 an. Ver- und Entschlüsselung laufen im selben Puffer
(Speicherbedarf beim Restore etwa einmal die Backup-Größe statt dreimal). Lock-Backups
entstehen seit Lauf F3a im Hintergrund unmittelbar nach dem Festschreiben (vermerkt, im
Scheduler; ADR 0040 Nr. 50).

## Amendment 2026-10-25 — Pflicht-Backup vor einer Migration im Format der Vorversion (Lauf F3c, ADR 0039 Nr. 129)

Das Backup, das Klein.Buch vor einer Datenbank-Migration schreibt, hat das höchste Format,
das die Version mit dem alten Schema-Stand einspielt — für Schema ≤ 31 (v2026.10) Format 2
ohne AAD, sonst das aktuelle Format. So gelingt der dokumentierte Rückfall auf die
Vorversion mit genau diesem Backup. Alle übrigen Backups bleiben Format 3 mit gebundenem
Kopf. Der Restore gibt den entschlüsselten Inhalt frei, bevor er das Pre-Restore-Backup
anlegt; die Spitze ist das Größere von beidem, nicht die Summe.
