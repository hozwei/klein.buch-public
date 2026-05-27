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
