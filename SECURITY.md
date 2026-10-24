# Sicherheit

## Sicherheitslücke melden

Bitte **nicht** als öffentliches Issue. Gemeldet wird privat über GitHub: im
Repository [`hozwei/klein.buch-public`](https://github.com/hozwei/klein.buch-public)
unter **Security → Report a vulnerability**
(<https://github.com/hozwei/klein.buch-public/security/advisories/new>). Die Meldung
ist dann nur für dich und den Rechteinhaber sichtbar.

Steht dieser Weg nicht zur Verfügung, geht die Meldung per E-Mail an den
Rechteinhaber laut `COPYRIGHT`.

Hilfreich sind:

- betroffene Version: Image-Tag bzw. `version` und `commit` aus
  `GET /api/public/info` der Instanz (auch im Über-Dialog);
- Beschreibung, Schritte zum Nachstellen, erwartete und tatsächliche Wirkung;
- ob die Lücke ohne Anmeldung, mit Sitzung oder nur mit Zugriff auf Host bzw.
  Volume ausnutzbar ist.

Bitte keine echten Kunden- oder Steuerdaten, keine Passphrase, keine TOTP-Codes
oder -Geheimnisse, keine Setup-Tokens, keine Datenbank- oder Backup-Dateien
mitschicken.

Behoben wird eine Lücke mit einem neuen Release (siehe „Unterstützte
Versionen“).

## Unterstützte Versionen

| Version | Sicherheitskorrekturen |
|---|---|
| neuestes Release der Docker-Fassung (`ghcr.io/hozwei/klein-buch:<Version>`) | ja |
| ältere Releases der Docker-Fassung | nein, bitte aktualisieren |
| Desktop-Releases v2026.5.0 und v2026.5.1 (Windows) | nein, eingestellt (ADR 0039) |
| Entwicklungsstände (Arbeitszweige, Trockenlauf-Artefakte, Builds mit der Anzeige „Entwicklungs-Build, Quellstand unbekannt“) | nein |

Versionen folgen CalVer (`JJJJ.M.P`). Korrekturen erscheinen als **neue**
Version, nie durch Ändern eines bestehenden Tags: Images tragen nur volle
Versionen, es gibt kein `:latest`, und Tags werden nie verschoben oder gelöscht
(ADR 0039, E15). Aktualisiert wird in der eigenen Container-Verwaltung;
`docs/operations-guide.md` §8 beschreibt Update und Rückfall. Die App kann neue
Versionen in der Inbox melden (opt-in unter Einstellungen → Sicherheit &
Anmeldung).

## Image prüfen

Jedes Release-Image wird in GitHub Actions aus diesem Repository gebaut
(`.github/workflows/release-image.yml`, nur aus Tags `v*` auf `main`) und mit
cosign ohne Schlüssel (keyless, OIDC-Identität des Workflows) signiert.

### Signatur

```sh
cosign verify ghcr.io/hozwei/klein-buch:<v> --certificate-identity-regexp '^https://github\.com/hozwei/klein\.buch-public/\.github/workflows/release-image\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$' --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

`<v>` ist die Version ohne führendes `v`, z. B. das Tag aus dem Release. Die
Release-Notizen nennen zusätzlich den Digest und die engere Form mit
`--certificate-identity` (genau der Release-Workflow und das Tag). Wer den Digest
fest eintragen will, nutzt `klein-buch-<v>-image.txt` aus dem Release (Tag und
`@sha256:…`).

### SBOM und Provenance

- Release-Asset `klein-buch-<v>-sbom.spdx.json` (SPDX-JSON, erzeugt mit syft aus
  dem geprüften Image).
- Zusätzlich hängen eine SBOM- und eine Provenance-Attestierung (BuildKit,
  `mode=max`) am Image-Index; `docker buildx imagetools inspect
  ghcr.io/hozwei/klein-buch:<v>` listet sie mit.

### Quellen

- Quelltext von Klein.Buch: das Tag `v<v>` in diesem Repository; die laufende
  Instanz verlinkt ihren Stand auf der Anmeldeseite, im Über-Dialog und in
  `GET /api/public/info` (AGPL-3.0 §13).
- Quelltext der Komponenten Dritter: Release-Asset
  `klein-buch-<v>-quellen-dritter.tar.zst` (mit `.sha256`): OpenJDK-Quellen des
  exakten Temurin-Builds samt jlink-Befehl, Quellpakete aller Debian-Pakete im
  Image, Quell-Archive der Copyleft-Teile der Java-Komponenten, `SHA256SUMS`.
  Ohne Quellarchiv bricht der Release-Lauf vor dem Push ab.
- Lizenzen und Hinweise: `/legal/` der laufenden Instanz (ohne Anmeldung) und
  `/usr/share/doc/klein-buch` im Image.

### Prüfungen vor der Veröffentlichung

Der Release-Lauf baut ohne Cache, prüft das Image mit einem Smoke-Test (u. a.
Nicht-root, read-only Root-FS, Einrichtung, Festschreiben mit echtem
KoSIT-Validator und Mustang), scannt mit trivy (CRITICAL/HIGH mit verfügbarem
Fix brechen ab; begründete Ausnahmen mit Ablaufdatum in `.trivyignore`) und
pusht erst danach. Die CI prüft zusätzlich mit `cargo deny`, `cargo audit`,
`pnpm audit` und gitleaks. Actions sind per Commit-SHA, Basis-Images per Digest
und Werkzeuge per SHA-256 festgelegt. Einzelheiten:
`docs/architecture/build-and-release.md`.

## Sicherheitsmodell

Eine Instanz dient genau einem Nutzer und darf aus dem Internet erreichbar sein.
Anmeldung mit Passphrase und TOTP, die Datenbank ist mit SQLCipher verschlüsselt,
Backups zusätzlich mit Argon2id und AES-256-GCM. Bedrohungsmodell und
Gegenmaßnahmen: `docs/architecture/security.md` und
`docs/adr/0039-web-docker-betrieb.md`. Was der Betreiber beitragen muss
(Reverse-Proxy mit TLS, `KLEIN_BUCH_TRUSTED_PROXIES`, Volumes, Off-Site-Backup,
Updates): `docs/operations-guide.md`.

Nicht abgedeckt sind ein kompromittierter Host oder eine kompromittierte
Container-Runtime, ein kompromittiertes Gerät oder ein kompromittierter Browser
des Nutzers, volumetrische DoS-Angriffe auf Netzebene und der Verlust der
Passphrase (Totalverlust, es gibt keinen Wiederherstellungsweg).
