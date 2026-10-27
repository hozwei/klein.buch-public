# Mitwirken

## Beiträge Dritter: vorerst keine Code-Beiträge

Code-Beiträge — Pull Requests, Patches und Code in Issues — werden **nicht
angenommen und nicht übernommen**. Klein.Buch hat einen einzigen Urheber und
Rechteinhaber; so bleiben spätere Lizenzentscheidungen, auch eine zusätzliche
kommerzielle Lizenz, ohne Zustimmung Dritter möglich (ADR 0014, Amendment vom
2026-10-24).

Fehlerberichte, Funktionswünsche und Sicherheitsmeldungen sind willkommen. Bitte das
Problem oder die Idee beschreiben, ohne längeren Code beizulegen — umgesetzt
wird im Entwicklungsstand des Rechteinhabers, von dort kommen die Änderungen in
dieses Repository. Auf GitHub wird nichts gemergt, auch keine Pull Requests von
Dependabot; die gelten nur als Hinweis (übernommen werden Versionsnummern, kein
fremder Code).

## Fehler und Wünsche

Fehler und Funktionswünsche bitte als
[Issue](https://github.com/hozwei/klein.buch-public/issues) melden. Hilfreich
sind:

- Version (Über-Dialog oder `version`/`commit` aus `GET /api/public/info`);
- Schritte zum Nachstellen, erwartetes und tatsächliches Verhalten;
- die passende Fehlermeldung bzw. Zeilen aus dem Container-Log.

Nicht ins Issue gehören echte Kunden- oder Steuerdaten, Passphrase, TOTP-Codes,
Einmal-Codes, Setup-Tokens sowie Datenbank- und Backup-Dateien.

**Sicherheitslücken nie als Issue**, sondern privat nach `SECURITY.md`.

## Forks

Forks sind unter der **AGPL-3.0-or-later** willkommen (`LICENSE`, Urheberhinweis
in `COPYRIGHT`, Komponenten Dritter in `THIRD-PARTY-NOTICES.md` und unter
`/legal/` der laufenden Instanz).

Wer eine veränderte Fassung als Netzdienst betreibt, muss ihren Nutzern den
Quelltext genau dieser Fassung anbieten (AGPL-3.0 §13). Klein.Buch zeigt dazu auf
der Anmeldeseite, im Über-Dialog und in `GET /api/public/info` einen Link auf den
laufenden Quellstand:

- **Laufzeit:** `KLEIN_BUCH_SOURCE_URL` auf das eigene Repository setzen (nur
  `http(s)`, ohne Benutzerangaben, Query oder Fragment; sonst Warnung im Log, und
  der Wert wird ignoriert).
- **Image-Build:** die Build-Argumente `KLEINBUCH_SOURCE_URL`,
  `KLEINBUCH_BUILD_COMMIT` und `KLEINBUCH_BUILD_TAG` setzen (siehe Kopf von
  `Dockerfile`), damit der Link auf den exakten Commit bzw. das Release zeigt.
  Ohne Commit zeigt die Oberfläche „Entwicklungs-Build, Quellstand unbekannt“.
- Die opt-in Versionsprüfung fragt die Releases der Quell-URL ab; abweichend
  davon setzt `KLEIN_BUCH_UPDATE_URL` die Abfrage-URL.

Das Handbuch-Kapitel `lizenz-und-quellcode` (in der App unter Hilfe, im Repository
unter `src-tauri/resources/handbook/`) erklärt das für Betreiber.

Für eigene Änderungen: Entwicklungsumgebung und Tests im `README.md` (Abschnitt
„Entwicklung“), Architektur in `docs/ARCHITECTURE.md`, Entscheidungen in
`docs/adr/`. Die GoBD-, §19- und Backup-Regeln im `README.md` sind keine
Stilfragen: Sie sichern Aufbewahrungspflicht und Kleinunternehmer-Status der
Nutzer.
