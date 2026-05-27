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
