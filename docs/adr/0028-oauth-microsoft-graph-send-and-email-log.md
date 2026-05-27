# ADR 0028 — Versand via Microsoft Graph (OAuth/PKCE) + append-only E-Mail-Protokoll

**Status:** Akzeptiert · 2026-05-22 · Blöcke 16 + 16b. Migrationen `0014_oauth`
/ `0015_email_log` (Schema v15).

## Kontext

Microsoft hat Basic-Auth (SMTP-Passwort) für Exchange Online / M365 abgekündigt;
Versand braucht **OAuth 2.0**. Daraus folgen drei Entscheidungen: über welchen
Kanal gesendet wird, wie die App-Registrierung läuft und wie der **Refresh-Token**
sicher gespeichert wird. Zusätzlich braucht es einen prüfbaren **Versand-Nachweis**
(was wurde wann an wen mit welcher Provider-Antwort verschickt).

## Entscheidung

1. **Versand über Microsoft Graph `/me/sendMail`** (nicht SMTP-XOAUTH2). Scopes
   `offline_access Mail.Send User.Read`.
2. **Nutzer-eigene Azure-App pro Konto** (tenant_id + client_id in der UI),
   Redirect = **Loopback** `http://localhost` (Port egal), **Public-Client +
   PKCE (S256)**. Kein Client-Secret in der App.
3. **Token-Haltung:** im OS-Keychain liegt **nur der Refresh-Token**, und zwar
   **gechunkt** (≤1024 Zeichen je Eintrag + Header). Grund: der Windows
   Credential Manager limitiert auf 2560 Zeichen, MS-Token (2–4 KB) sprengen das
   (real beim Smoke aufgetreten). Der Access-Token wird **bei jedem Versand frisch
   geholt** und **nie persistiert**. `0014` ergänzt 5 **nicht-geheime**
   OAuth-Spalten auf `mail_accounts`.
4. **`email_log`** (`0015`) ist **append-only** (no-update/no-delete-Trigger) und
   protokolliert **jeden Versuch** (Erfolg UND Fehler) mit Provider-Antwort
   (SMTP-Code bzw. Graph-`request-id`), Empfänger-/Beleg-/Konto-Snapshot und
   Anhang-Anzahl. Serverseitige Suche + Zeitfenster + Filter.

## Konsequenzen

- Kein Client-Secret in der App (Public-Client + PKCE) → nichts Geheimes zu
  schützen außer dem Refresh-Token im OS-Keychain.
- Das Chunking umgeht das Windows-2560-Zeichen-Limit zuverlässig.
- Der append-only Log gibt einen prüfbaren Versand-Nachweis, der nicht stillschweigend
  manipulierbar ist (konsistent mit der GoBD-Audit-Linie, ADR 0006).
- SMTP (mit Keychain-Passwort, Block 5) bleibt parallel verfügbar; `dispatch_send`
  wählt SMTP vs. Graph je Konto.

## Alternativen

| Option | Contra |
|---|---|
| SMTP-XOAUTH2 statt Graph | Microsoft baut SMTP-AUTH zurück; Graph ist der strategische, besser dokumentierte Pfad |
| Zentrale (mitgelieferte) App-Registrierung | bräuchte ein gehütetes Secret/Backend; widerspricht local-first + Open-Source |
| Access-Token persistieren | unnötiges Geheimnis auf Platte; frisches Holen je Versand ist sicherer |
| Refresh-Token ungechunkt speichern | scheitert real am Windows-Credential-Manager-Limit |

## Referenzen

`mail::oauth_ms` (PKCE, exchange/refresh, `graph_send`, Loopback-Capture,
gechunkte Keychain-Ablage), `mail::dispatch_send`, `db::repo::email_log`,
Commands `mail_oauth_{status,connect,disconnect}` + `email_log_*`, Migrationen
`0014`/`0015`; ADR 0006 (Audit/Append-only), Block 5 (SMTP/Keychain). Commit
`block-16`.
