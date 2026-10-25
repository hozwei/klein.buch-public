# ADR 0011 — Mail (Phase 1): SMTP via lettre, Credentials im OS-Keychain

**Status:** Akzeptiert · 2026-05-20 · Block 5. (Decision-Log D-13, D-14)

## Kontext

Rechnungen (ZUGFeRD-PDF) sollen direkt aus der App versendet werden. Phase 1 darf
keinen OAuth-Aufwand erfordern (das kommt mit Exchange Online in Block 16) und muss
universell mit jedem SMTP-Provider funktionieren. SMTP-Passphrasen dürfen niemals
in der DB liegen.

## Entscheidung

- **SMTP via `lettre`** (async, rustls-TLS). TLS-Strategie: `use_tls=false` →
  `builder_dangerous` (lokaler Relay/MailHog), Port 465 → implizites TLS, sonst
  STARTTLS.
- **Multi-Attachment von Anfang an** (`OutgoingMail.attachments: Vec<…>`) —
  Grundlage für das Angebots-Bundle (Angebot + Datenschutz + AGB) in Block 8.
- **Credentials im OS-Keychain** über die `keyring`-Crate; Service-ID-Schema
  `kleinbuch::mail::{account_id}`. `mail_accounts` hält nur Metadaten +
  `keychain_service_id`, **nie** die Passphrase.
- **`keyring`-Feature-Flags Pflicht**: keyring 3 hat **keine** Default-Backends —
  ohne `apple-native`/`windows-native`/`sync-secret-service` fällt es auf einen
  flüchtigen In-Memory-Mock zurück (Passphrase nach Neustart weg). Wir nutzen
  `crypto-rust` (kein openssl, konsistent mit rustls).

## Konsequenzen

- Versand funktioniert mit jedem Provider; OAuth ist additiv (Block 16).
- Passphrase verlässt den Command nie Richtung DB/Logs/Audit (Backup-Hardline).
- Beim Versand wird das ZUGFeRD-PDF vor dem Anhängen per SHA-256 gegen das Archiv
  verifiziert (Tamper-Schutz).
- Tests/CI nutzen den keyring-Mock-Store + MailHog, brauchen also keinen echten
  Schlüsselbund/Secret-Service-Daemon.

## Alternativen

| Option | Contra |
|---|---|
| Nur OAuth/Graph | Provider-spezifisch, zu schwer für Phase 1 |
| Passphrase verschlüsselt in DB | Doppelte Krypto-Verantwortung, Keychain ist Standard |
| `keyring` ohne Feature-Flags | Mock-Store → Passphrase nicht persistent (Bug) |

## Referenzen

`mail::{smtp,keyring,templates}`, `commands::mail`; keyring-Doku
„Credential store features".

## Amendment 2026-10-23 — Web-/Docker-Betrieb (ADR 0039)

**Status:** Akzeptiert · 2026-10-23 · Phase W (ADR 0039, E6/E9; Umsetzung Block W8). Im
Container gibt es keinen OS-Keychain (kein Secret-Service, keine Desktop-Anmeldung); der
`keyring`-Mock wäre flüchtig. Deshalb:

- **Secrets in der SQLCipher-DB:** SMTP-Passwörter und das SFTP-Passwort der Off-Site-Ziele
  (ADR 0034) liegen in einer eigenen STRICT-Tabelle der verschlüsselten DB (Migration `0031`),
  Schlüssel je Mailkonto- bzw. Ziel-ID. Sie sind durch dieselbe Passphrase geschützt wie alle
  Daten (ADR 0035), gehen damit nur verschlüsselt in Backups und weder in den Migrations- noch
  in den DSGVO-Export. Die Regel „nie in der DB“ wird zu „nur in der verschlüsselten DB“; die
  verworfene doppelte Krypto entsteht nicht, weil SQLCipher die ganze Datei verschlüsselt.
- **`keyring` entfällt** samt Feature-Flags und Service-ID-Schema; `mail_accounts` hält weiter
  nur Metadaten. Tests brauchen keinen Keyring-Mock mehr.
- **Zugangsdaten an (Host, Port, Benutzer) gebunden:** Ändert sich eines dieser Felder, löscht
  der Server das gespeicherte Passwort; ohne neues Passwort wird abgelehnt. Test-Verbindungen
  für gespeicherte Konten nur per ID; `use_tls=false` nur ohne Zugangsdaten.
- **Nur SMTP:** OAuth/MS-Graph (ADR 0028) ist im Server-Betrieb ausgeblendet.
- Passwörter bleiben aus Logs und `audit_log` heraus (Backup-Hardline unverändert).

## Amendment 2026-10-25 — Test-Mailserver Mailpit (Lauf F7a)

Der SMTP-E2E-Test (`tests/e2e_test.rs`) läuft in CI gegen Mailpit
(`axllent/mailpit`, per Digest gepinnt) statt MailHog, das seit Jahren keine
Fassung mehr bekommt und ohne Tag und Digest eingebunden war (W14-021). Die
Nennungen von MailHog oben sind historisch. Begründung: ADR 0039 Nr. 142 (1).
