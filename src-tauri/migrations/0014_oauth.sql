-- Block 16 (Phase 2D) — OAuth Microsoft Exchange Online.
--
-- Erweitert `mail_accounts` um die Metadaten für den OAuth-Auth-Code-Flow mit
-- PKCE (Microsoft Identity Platform v2.0 / Microsoft Graph). Es liegen hier
-- bewusst NUR nicht-geheime Metadaten:
--   * tenant_id / client_id  → die nutzer-eigene Azure-App-Registrierung
--     (kein Secret — Public-Client, PKCE statt client_secret).
--   * account_email          → das verbundene Postfach (aus Graph /me), nur Anzeige.
--   * scopes                 → zuletzt gewährte Delegated-Scopes, nur Anzeige.
--   * token_expires_at       → Ablauf des Access-Tokens (UTC ISO-8601), für die
--     Refresh-Entscheidung ohne Keychain-Zugriff.
--
-- Hard-Rule (Backup-Hardline): Access- UND Refresh-Token leben NIEMALS in der DB
-- oder im Log — sie liegen ausschließlich im OS-Keychain unter
-- `keychain_service_id` (siehe mail::keyring, user-Key "oauth").

ALTER TABLE mail_accounts ADD COLUMN oauth_tenant_id TEXT;
ALTER TABLE mail_accounts ADD COLUMN oauth_client_id TEXT;
ALTER TABLE mail_accounts ADD COLUMN oauth_account_email TEXT;
ALTER TABLE mail_accounts ADD COLUMN oauth_scopes TEXT;
ALTER TABLE mail_accounts ADD COLUMN oauth_token_expires_at TEXT;

UPDATE app_settings SET value = '14' WHERE key = 'schema_version';
