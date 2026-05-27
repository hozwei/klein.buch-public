# Klein.Buch — Operations-Guide (v0.1.0)

Betriebs- und Wartungsabläufe zum v0.1.0-Stand (Phasen 1–2D).

## 1. Backup & Restore

- **Backups** liegen verschlüsselt (Argon2id + AES-256-GCM) im konfigurierten Ziel
  (Einstellungen → Backup). Dateiname enthält Zeitstempel + Typ
  (`manual`/`auto_critical`/`auto_daily`/`pre_restore`).
- **Restore**: Einstellungen → Backup → Wiederherstellen → Datei wählen →
  GoBD-Warnung bestätigen → Passphrase eingeben. Klein.Buch erstellt ein
  Pre-Restore-Backup, verifiziert Hash + Schema-Version und tauscht DB + Archiv
  beim nächsten App-Start (Atomic-Swap, wegen Windows-File-Lock).
- **Voraussetzung**: korrekte Backup-Passphrase. Ohne sie ist kein Restore möglich.
- `inputs/` (Vorlagen, AfA-Tabellen, Logos) wird vom Restore **nicht** verändert.

> TODO (spätere Phase): Restore aus CLI/Recovery-Modus, Integritäts-Report.

## 2. Migrations-Export (Steuerberater / Datenportabilität)

- Einstellungen → Migrations-Export erzeugt ein **offenes ZIP**: pro Tabelle ein
  JSON, das `archive/`-Verzeichnis (PDF/XML der Belege), das Schema-SQL (aus dem
  Migrator), eine `erd.md`, ein `manifest.json` und ein eigenständiges
  `read_export.py` zum Sichten ohne Klein.Buch.
- Zweck: Datenherausgabe an den Steuerberater und Schutz vor Vendor-Lock-in.
- Für die laufende Steuer-Abgabe gibt es zusätzlich die gezielten Exporte
  (DATEV-Buchungsstapel, ELSTER-Ausfüllhilfe, Steuerberater-ZIP) unter
  **Steuer (EÜR) → Exportieren** — siehe `user-guide.md` §9.

## 3. Schema-Update-Verfahren

- Schema-Migrationen sind **forward-only**. Beim Start prüft Klein.Buch
  `app_settings.schema_version` gegen `EXPECTED_SCHEMA_VERSION`; bei Mismatch
  startet die App nicht (Schutz vor Down-Migration mit alter Binary).
- Vor einem Update mit Schema-Migration: aktuelles Backup sicherstellen.
- Ein Downgrade der Binary nach einer Migration ist nicht unterstützt — bei Bedarf
  Restore eines Backups vom passenden Schema-Stand.

> TODO (spätere Phase): Migrations-Changelog je Version, Pre-Flight-Check-Report.

## 4. Sidecar-Updates (KoSIT / Mustang)

- KoSIT-Validator und Mustang liegen gebündelt in einem jlink-JRE-Sidecar unter
  `binaries/klein-buch-java-<target-triple>/`.
- Update: neue KoSIT-/Mustang-Version + XRechnung-Konfiguration einspielen, Sidecar
  neu bauen (Block-0-Skript), Health-Checks fahren (`--version` für beide Tools),
  eine Test-Rechnung festschreiben und mit `verapdf` gegen PDF/A-3 prüfen.
- Cross-OS-Bundles entstehen per CI-Matrix (siehe ADR 0001).

> TODO (spätere Phase): automatisierter Sidecar-Versions-/Konformitäts-Smoke in CI.

## 5. Geschäftsjahres-Abschluss (prüfungssicher)

- **Steuer (EÜR) → Geschäftsjahr** schließt ein **abgelaufenes** Jahr ab. Ablauf:
  fällige AfA buchen → Anlagen + Abschreibungen sperren → EÜR-Snapshot ins
  Festschreibungsprotokoll → Audit-Eintrag → Auto-Critical-Backup.
- Der Abschluss ist **unumkehrbar** und verlangt eine **entsperrte
  Backup-Passphrase** (sonst Abbruch). Ein abgeschlossenes Jahr ist DB-seitig
  unveränderlich (no-update/no-delete-Trigger, §146 AO).
- **Korrektur nach Abschluss:** nur über einen **Storno-Beleg** (wirkt im
  Storno-Jahr). Es gibt kein „Wieder-Öffnen" eines Jahres.
- Auto-AfA zum 01.01. ist Default an, abschaltbar (Geschäftsjahr-Seite).

## 6. Credentials & OAuth-Token

- **SMTP-Passwort** und **OAuth-Refresh-Token** liegen ausschließlich im
  OS-Schlüsselbund (Windows Credential Manager / macOS Keychain / Linux Secret
  Service), nie in DB/Logs/Audit. Der OAuth-Refresh-Token wird **gechunkt**
  abgelegt (Windows-Limit von 2560 Zeichen; MS-Token sind größer). Access-Token
  werden bei jedem Versand frisch geholt und nicht persistiert.
- **Postfach trennen:** Einstellungen → Mail → *Trennen* entfernt den
  Refresh-Token aus dem Schlüsselbund; *Löschen* eines SMTP-Kontos entfernt das
  Passwort. Ein Backup-Restore überträgt **keine** Keychain-Geheimnisse — nach
  einem Restore auf einem anderen Rechner sind Konten neu zu verbinden.
- **Backup-Passphrase** ist davon getrennt: sie lebt nur im Sitzungs-Speicher und
  ist nirgends gespeichert. Verlust = Backups unbrauchbar.

## 7. E-Mail-Versandprotokoll

- Jeder Versand (Erfolg **und** Fehler) wird **append-only** protokolliert
  (`email_log`, nicht änder-/löschbar) mit Provider-Antwort (SMTP-Code bzw.
  Microsoft-Graph-`request-id`), Empfänger, Beleg-Nr. und Anhang-Anzahl.
- Einsehbar unter **Einstellungen → E-Mail-Protokoll** (Suche + Zeitfenster +
  Filter) sowie als „Versand-Historie" auf der jeweiligen Rechnung/Angebot.
- Nutzen im Betrieb: Nachweis/Diagnose bei Zustellfragen — eine angenommene Mail
  (2xx/`202`) ist beim Provider eingeliefert, sagt aber nichts über den
  Posteingang des Empfängers (Spam/Filter; siehe `user-guide.md` §1.3).
