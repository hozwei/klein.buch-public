# ADR 0036 — Factory Reset (vollständiges Zurücksetzen)

**Status:** Akzeptiert · 2026-05-24 · v1.0 (Umsetzung im Security-Block G1).

## Kontext

Nutzer brauchen in den Optionen ein **vollständiges Zurücksetzen** — alle Daten
löschen, zurück auf den Onboarding-Start (z. B. Gerät verkaufen/weitergeben,
echter Neuanfang, Testdaten verwerfen). Das steht in **Spannung zur GoBD-Hardline**
(„kein Löschen-UI", Storno statt Löschung, 10-Jahre-Aufbewahrung, Archive write-once).

Auflösung: GoBD verbietet das **selektive, stille Verändern/Löschen einzelner
Belege** (Manipulation der Bücher). Es verbietet nicht, dass der Nutzer seine
**gesamte lokale Installation** bewusst vernichtet — solange seine
Aufbewahrungspflicht anderweitig erfüllbar bleibt. Ein Factory Reset ist genau das:
ein **Total-Nuke der lokalen Instanz**, kein Beleg-Editier-Werkzeug.

## Entscheidung

1. **Factory Reset = die EINE sanktionierte Total-Löschung.** Selektives Löschen
   einzelner Belege bleibt verboten (Storno statt Löschung). Der Reset löscht die
   **gesamte** lokale Instanz: DB, Archiv, lokale Backups, App-Settings → zurück
   auf den Onboarding-Zustand (Passphrase-Setup).
2. **Mehrstufige Absicherung (prüfungssicherer Default):**
   - (a) Warn-Dialog mit **GoBD-Hinweis** (10-Jahre-Aufbewahrungspflicht).
   - (b) **Export-First prominent angeboten** (Steuerberater-ZIP / `migration_export`),
     damit die Aufbewahrung off-app erfüllt werden kann.
   - (c) **Tipp-Bestätigung** (Nutzer tippt z. B. `LÖSCHEN`).
   - (d) **Passphrase-Eingabe** (die Master-Passphrase, ADR 0035).
   - (e) finaler `confirmDialog`.
   - **Gating:** Existieren **festgeschriebene Belege** (issued/locked), ist der
     Reset nur nach **Export** ODER nach expliziter, getippter GoBD-Quittung
     („Ich habe meine Aufbewahrungspflicht erfüllt") möglich. Leere/Testinstanz:
     direkter Reset nach (c)–(e).
3. **Scope = nur lokal.** Off-Site-Backups (Cloud-Ordner/SFTP) bleiben
   **unangetastet** — die App löscht keine Remote-Daten. Hinweis an den Nutzer,
   diese bei Bedarf selbst zu entfernen.
4. **Kein Audit-Eintrag überlebt** — der `audit_log` ist Teil der DB und wird
   mitgelöscht. Inhärent bei einem Total-Reset; bewusst akzeptiert (es ist kein
   Beleg-Eingriff, sondern Instanz-Vernichtung).

## Konsequenzen

- Nutzer kann sein lokales Gerät rechtssauber „auf null" setzen, ohne die
  GoBD-Linie für laufende Bücher zu untergraben (Export-First + Warnung + Quittung).
- Kein Selektiv-Lösch-Schlupfloch — die GoBD-Hardline bleibt für den Normalbetrieb intakt.
- Bewusste GoBD-Ausnahme; in CLAUDE.md GoBD-Hardline als die eine sanktionierte
  Total-Löschung dokumentiert (damit künftige Blöcke nicht fälschlich STOP/ASK auslösen).

## Alternativen

| Option | Contra |
|---|---|
| Kein Reset (Nutzer löscht App + `%APPDATA%` manuell) | schlechte/fehleranfällige UX; Daten + Schlüssel bleiben evtl. liegen |
| Selektives Löschen einzelner Belege | verboten (GoBD, Storno statt Löschung) |
| Reset ohne Export-First/Quittung | GoBD-Retention-Risiko, kein prüfungssicherer Default |
| Auch Remote-Backups mitlöschen | gefährlich (kann fremde/geteilte Ziele treffen), oft kein Zugriff — bewusst draußen |

## Umsetzung (G1-RESET, 2026-05-25)

- **Zweiphasig, marker-getrieben (race-frei, wie der Restore).** Der Live-Command
  schließt den Pool **nicht** und löscht **nichts** — sonst liefen parallele
  Commands in „attempted to acquire a connection on a closed pool" (im Smoke-Test
  beobachtet; der erste Entwurf, der im Command schloss+löschte, hat genau das
  ausgelöst und nichts gelöscht). Stattdessen:
  - **Phase A** (`commands::factory_reset::factory_reset_request`): Passphrase +
    Gating prüfen, einen Marker `FACTORY_RESET_PENDING.json` ins `data_dir`
    schreiben (mit den zu löschenden Keychain-Service-IDs der Mail-Konten),
    `AppHandle::restart()`.
  - **Phase B** (`backup::factory_reset::apply_pending`, aus
    `db::prepare_filesystem` **vor** dem Pool-Open): `data_dir` force-rekursiv
    leeren, Kern-Verzeichnisse leer neu, Keychain-Secrets löschen. Idempotent
    (Marker wird mitgelöscht); crash-sicher (ein Absturz nach Phase A wird beim
    nächsten Start nachgeholt).
- **Datei-Nuke = gesamter `data_dir`-Inhalt**, nicht nur DB/Archiv/Backups:
  zusätzlich Branding, Exporte, Vorschau, Restore-Staging — alles
  maschinen-verwaltete, kundendaten-tragende Material unter `data_dir`. `inputs/`
  liegt außerhalb von `data_dir` und bleibt unberührt (Hardline).
- **Keychain-Erweiterung (über den ursprünglichen Pt.-1-Wortlaut hinaus):** Für
  die in der Motivation genannte Geräte-Weitergabe werden zusätzlich die
  app-eigenen OS-Keychain-Geheimnisse best-effort gelöscht — SMTP-Passwörter und
  OAuth-Refresh-Tokens je Mail-Konto (`mail::keyring`) sowie das SFTP-Backup-
  Passwort (`backup::sftp`). Sonst behielte ein weitergegebenes Gerät
  E-Mail-Zugriffstokens. Fehler hier sind nicht fatal (Daten sind bereits weg).
- **Echter Prozess-Neustart** (`AppHandle::restart()`, in Tauri-2-Docs verifiziert,
  `-> !`): garantiert frischen Tauri-State. Bewusst **kein** Webview-Reload — ein
  erneutes `app.manage::<SqlitePool>(…)` beim Re-Onboarding wäre ein No-op (Tauri
  ersetzt einen bereits gemanagten State-Typ nicht). `restart()` divergiert; der
  Erfolgspfad kehrt nicht zum Frontend zurück (Off-Site-Hinweis wird vorher im UI
  gezeigt).
- **Passphrase-Gate** nutzt `backup::verify_passphrase` (Argon2id-Verifier in
  `app_settings`) — unabhängig vom SQLCipher-Key, server-seitig erzwungen.
- **GoBD-Gating** (Pt. 2) ist serverseitig in `domain::factory_reset` (pure):
  Tipp-Wort `LÖSCHEN` + bei festgeschriebenen Belegen Export-Flag **oder** exakt
  getippte Quittung. Belege-Zählung über alle `locked_at`-Tabellen (invoices,
  quotes, expenses, depreciation_entries, private_movements).

## Referenzen

`commands::factory_reset` (Phase A: `factory_reset`, `factory_reset_request`,
`factory_reset_check`), `backup::factory_reset` (Phase B: `request`/`apply_pending`,
aufgerufen aus `db::prepare_filesystem`), `domain::factory_reset` (pure Gating),
`migration_export` (Export-First), `backup` (verify/target/sftp/restore::clear_dir_force),
`mail::keyring`, Onboarding/`BackupGate.svelte`, Frontend `routes/settings/reset`;
ADR 0035 (Passphrase), ADR 0009/0034 (Backup), GoBD-Hardline in `CLAUDE.md`.
RELEASE-1.0-GUIDE §G1-RESET.
