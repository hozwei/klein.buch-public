# Release & Code-Signing — Beschaffungs- und Lieferanleitung (Block 17d)

> **ENTSCHEIDUNG 2026-05-22 (Manuel): KEIN Code-Signing.** Klein.Buch wird Open-Source; wir
> liefern **unsignierte** Installer. Wer eine signierte Variante braucht, kompiliert selbst.
> `release.yml` (Block 17d) baut daher unsignierte Cross-OS-Bundles **+ SHA-256-Checksums** als
> Draft-Release — **keine Secrets, kein Azure, kein Apple-Account nötig.**
>
> Die Signing-Abschnitte §3–§9 unten sind ab jetzt **nur noch Referenz/Anhang**, falls du dich später
> doch für Signierung entscheidest. Für v0.1.0 ist nichts davon zu tun.

**Stand:** 2026-05-22 · Quellen: offizielle Tauri-2-Docs (Win zuletzt 26.09.2025, macOS zuletzt 17.05.2026), siehe §13.
**Zweck (Anhang-Modus):** Referenz, *was* nötig WÄRE, falls Signing später gewünscht ist. Aktiv relevant ist nur §10 (unsignierter Release-Workflow) + §6 (Cross-OS-Sidecar, ADR 0001).

Klein.Buch: `productName = Klein.Buch`, `version = 0.1.0`, `identifier = de.wildbach.kleinbuch`, Publisher = Wildbach Computerhilfe.

---

## 0. TL;DR — was du JETZT anstößt

| Priorität | Aktion | Warum jetzt | Vorlaufzeit |
|---|---|---|---|
| **1 (Pflicht)** | Windows-Signing entscheiden: **Azure Trusted Signing** (empfohlen) ODER OV/EV-Zertifikat | Zielgruppe ist ~Windows-only; ohne Signatur SmartScreen-Warnung beim Download | Azure-Identitätsprüfung: Tage bis ~1 Woche; CA-Zertifikat: Tage |
| 2 (optional) | Apple Developer Program ($99/Jahr) **nur falls** macOS-Build gewünscht | Enrollment + ggf. D-U-N-S dauern | Individual: 24–48 h; Firma mit D-U-N-S: 1–3 Wochen |
| 3 (kann warten) | Linux: nichts beschaffen — nur SHA-256-Checksums | Kein Signing-Zwang | — |

**Meine Empfehlung (Push-back gegen „alles signieren"):** Für v0.1.0 **nur Windows signieren**. Deine Nutzer (deutsche §19-Kleinunternehmer) sind quasi vollständig auf Windows. macOS-Signing kostet $99/Jahr + einen Mac + Notarisierungs-Aufwand für eine verschwindend kleine Nutzergruppe; Linux-Signing bringt für Direct-Download nichts. macOS/Linux liefern wir zunächst **unsigniert mit Checksums** und ziehen Signing nach, wenn echte Nachfrage da ist. Wenn du das anders siehst — sag's, dann bauen wir die volle Matrix.

---

## 1. Grundentscheidung: welche Plattformen signieren?

| Plattform | Signing nötig? | Folge ohne Signatur | Empfehlung v0.1.0 |
|---|---|---|---|
| **Windows** | Nicht zum Ausführen, aber **dringend** | SmartScreen: „Der Computer wurde durch Windows geschützt / Unbekannter Herausgeber" beim Download via Browser | **Signieren** |
| **macOS** | Ja, falls du Mac-Nutzer ernst nimmst | Gatekeeper: „App ist beschädigt / nicht verifiziert", Start blockiert | Optional → später |
| **Linux** | Nein | Keine — `.deb`/`.rpm`/`.AppImage` laufen ohne Signatur | Nur Checksums |

---

## 2. Was du NICHT brauchst (bewusst gespart)

- **Updater-Signing (minisign / `TAURI_SIGNING_PRIVATE_KEY`)** — entfällt. Klein.Buch ist local-first ohne Auto-Update (CLAUDE.md: „kein Auto-Update, kein Cloud-Sync"). Den ganzen Updater-Schlüssel-Komplex überspringen wir.
- **App-Store-Distribution** — kein Microsoft Store, kein Apple App Store. Wir liefern Direct-Download über GitHub Releases. Damit brauchst du auf macOS ein **Developer ID Application**-Zertifikat (nicht „Apple Distribution") und auf Windows ein normales Code-Signing-Zertifikat (kein Store-Zertifikat).
- **EV-Zertifikat zwingend** — nett (sofortige SmartScreen-Reputation), aber teurer. OV oder Azure Trusted Signing reicht; die SmartScreen-Reputation baut sich dann über Downloads/Zeit auf.

---

## 3. Windows-Signing

Seit **1. Juni 2023** dürfen OV- und EV-Code-Signing-Schlüssel nicht mehr als einfache `.pfx`-Datei vorliegen — sie müssen auf einem **FIPS-140-2-Hardware-Token oder in einer Cloud-HSM** liegen. Der alte, simple `certificateThumbprint`-Weg in `tauri.conf.json` gilt **nur noch für OV-Zertifikate, die vor diesem Datum ausgestellt wurden**. Für alles Neue brauchst du entweder einen Cloud-Signing-Dienst oder einen Hardware-Token — und für CI praktisch immer Cloud. Daraus folgen zwei realistische Wege:

### Weg A — Azure Trusted Signing (empfohlen)

Microsofts Cloud-Signing-Dienst (früher „Azure Code Signing"). Günstig, kein Hardware-Token, CI-tauglich. Tauri unterstützt es offiziell über `trusted-signing-cli` als `signCommand`.

**Voraussetzungen, die DU einrichtest:**

1. **Azure-Subscription** (Pay-as-you-go reicht).
2. **Trusted Signing Account** + **Certificate Profile** anlegen (Azure Portal → „Trusted Signing").
3. **Identitätsprüfung**: Microsoft validiert die Identität hinter dem Zertifikat.
   - Als **Organisation** (empfohlen für „Wildbach Computerhilfe"): die rechtliche Geschäftsidentität wird geprüft (Handelsregister/Gewerbe). Der geprüfte Name erscheint später als „Herausgeber".
   - Als **Einzelperson**: historisch nur mit ≥ 3 Jahren verifizierbarer Identitätshistorie. → **Verifiziere die aktuellen Eligibility-Regeln vor dem Anlegen**, das ändert Microsoft gelegentlich.
4. **App Registration** (Microsoft Entra ID) für die Maschinen-Authentifizierung der CI anlegen → liefert Client-ID, Tenant-ID, Client-Secret.
5. Der App Registration im Trusted-Signing-Konto die Rolle **„Trusted Signing Certificate Profile Signer"** zuweisen (IAM).

**Was DU als GitHub-Secrets lieferst** (Settings → Secrets and variables → Actions):

| Secret | Wert |
|---|---|
| `AZURE_CLIENT_ID` | Application (client) ID der App Registration |
| `AZURE_CLIENT_SECRET` | Client-Secret-Wert der App Registration |
| `AZURE_TENANT_ID` | Directory (tenant) ID |

**Was DU mir als Klartext-Werte gibst** (kommen in `tauri.conf.json`, nicht geheim):

- Endpoint deiner Region, z. B. `https://weu.codesigning.azure.net` (West Europe) — **bitte deine konkrete Region nennen**
- Account-Name deines Trusted-Signing-Kontos
- Profil-Name (Certificate Profile)

Ich verdrahte dann in `tauri.conf.json`:

```jsonc
{
  "bundle": {
    "windows": {
      "signCommand": "trusted-signing-cli -e https://weu.codesigning.azure.net -a <DEIN_ACCOUNT> -c <DEIN_PROFIL> -d \"Klein.Buch\" %1"
    }
  }
}
```

**Kosten:** Größenordnung ~10 USD/Monat (vor Kauf prüfen — §12). Plus minimale Azure-Subscription-Grundkosten.

### Weg B — klassisches OV/EV-Zertifikat von einer CA

Von Sectigo, DigiCert, SSL.com o. Ä. Kommt nach der Reform entweder auf einem **Hardware-Token** (für CI ungeeignet, weil der Token physisch am Build-Rechner stecken muss) oder als **Cloud-Signing** (DigiCert KeyLocker, SSL.com eSigner). Für unsere GitHub-Actions-Pipeline brauchst du die **Cloud-Variante**; dann signieren wir über deren CLI als `signCommand` (analog Weg A, andere Credentials). Teurer und CA-spezifischer als Azure. Nur nehmen, wenn du ohnehin schon ein Cloud-Cert hast.

> **Nimm Weg A, außer du hast einen konkreten Grund für B.**

---

## 4. macOS-Signing (optional, nur falls Mac-Build gewünscht)

**Hartes Vorab:** Du brauchst (a) ein **bezahltes Apple Developer Program** ($99/Jahr — der freie Plan kann **nicht notarisieren**) und (b) **einmal einen Mac**, um die Certificate Signing Request (CSR) zu erzeugen und das Zertifikat als `.p12` zu exportieren. (Technisch ginge die CSR auch per `openssl` ohne Mac, aber der dokumentierte, schmerzfreie Weg ist ein Mac — auch leihweise genügt, es ist eine Einmal-Aktion.)

**Schritte, die DU machst:**

1. **Apple Developer Program** beitreten. Als **Einzelperson** schnell (24–48 h). Als **Firma** brauchst du eine **D-U-N-S-Nummer** (kostenlos von Dun & Bradstreet, kann 1–3 Wochen dauern) — nur nötig, wenn als Herausgeber „Wildbach Computerhilfe" statt deines Namens stehen soll.
2. Auf einem Mac in **Keychain Access** eine **CSR** erzeugen (Certificate Assistant → Request a Certificate from a CA).
3. Im Apple Developer Portal → Certificates → **„Developer ID Application"** erstellen (nur der Account-Holder kann das), CSR hochladen, `.cer` herunterladen, per Doppelklick in die Keychain importieren.
4. In Keychain Access unter „Meine Zertifikate" den **privaten Schlüssel** mit-exportieren als **`.p12`** (Passwort vergeben → merken!).
5. `.p12` nach base64 konvertieren (siehe §8).
6. **App Store Connect API-Key** für die Notarisierung anlegen (App Store Connect → Users and Access → Integrations → API-Key mit „Developer"-Rolle). Lade den **`.p8`-Private-Key** herunter (geht **nur einmal**!). Notiere **Key ID** und **Issuer ID**.

**Was DU als GitHub-Secrets lieferst:**

| Secret | Wert |
|---|---|
| `APPLE_CERTIFICATE` | base64 des `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Passwort des `.p12`-Exports |
| `APPLE_SIGNING_IDENTITY` | z. B. `Developer ID Application: Manuel Schmid (TEAMID)` — exakt wie aus `security find-identity -v -p codesigning` |
| `KEYCHAIN_PASSWORD` | beliebiges, von dir gesetztes Passwort für die temporäre CI-Keychain |
| `APPLE_API_ISSUER` | Issuer ID aus App Store Connect |
| `APPLE_API_KEY` | Key ID aus App Store Connect |
| `APPLE_API_KEY_PATH` | wird im Workflow gesetzt; **du lieferst stattdessen** den `.p8`-Inhalt als zusätzliches Secret `APPLE_API_KEY_P8` (base64), ich schreibe ihn im Job in eine Datei |

**Kostenfalle CI:** macOS-Runner-Minuten auf GitHub Actions sind bei **privaten** Repos teuer (Faktor ~10 ggü. Linux). Bei öffentlichem Repo (AGPL-3.0 → spricht für public) sind sie frei. Das Public-Repo ist `hozwei/klein.buch-public`.

---

## 5. Linux

**Kein Signing nötig.** Der Workflow erzeugt `.deb`, `.rpm` und `.AppImage` plus **SHA-256-Checksums** — das ist für Direct-Download Standard und ausreichend. Optionales GPG-Signing der AppImage/`.deb` (Secrets: GPG-Private-Key base64 + Passphrase) **verschieben wir** auf v0.2+, sobald du ein eigenes APT/RPM-Repo betreibst. Für v0.1.0: nichts zu beschaffen.

---

## 6. Nicht-Signing-Voraussetzung: Cross-OS-Sidecar (ADR 0001)

Unabhängig vom Signing: der **Java-jlink-JRE-Sidecar** (Mustang + KoSIT-Validator) kann nur für die **Host-Plattform** gebaut werden, von der das JDK stammt. Laut ADR 0001 baut Block 0 nur das Windows-Bundle; die anderen drei Triples entstehen erst in der **Release-Matrix**:

| Target-Triple | Runner |
|---|---|
| `x86_64-pc-windows-msvc` | `windows-latest` |
| `x86_64-apple-darwin` | `macos-13` |
| `aarch64-apple-darwin` | `macos-14` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` |

**Das ist meine Arbeit in 17d** (Portierung `build-sidecar.ps1` → `build-sidecar.sh`, Einbau in die Matrix). Von dir brauche ich dazu nur die Bestätigung der **JDK-Version** (vermutlich Temurin 21, passend zu ADR 0001 / jlink-21). Kein Beschaffungsaufwand.

---

## 7. GitHub-Secrets — Sammeltabelle

Anlegen unter: **Repo → Settings → Secrets and variables → Actions → New repository secret.**

| Secret | Plattform | Wer liefert | Pflicht für v0.1.0 (Empfehlung) |
|---|---|---|---|
| `AZURE_CLIENT_ID` | Windows | du | ✅ |
| `AZURE_CLIENT_SECRET` | Windows | du | ✅ |
| `AZURE_TENANT_ID` | Windows | du | ✅ |
| `APPLE_CERTIFICATE` | macOS | du | optional |
| `APPLE_CERTIFICATE_PASSWORD` | macOS | du | optional |
| `APPLE_SIGNING_IDENTITY` | macOS | du | optional |
| `KEYCHAIN_PASSWORD` | macOS | du (frei wählbar) | optional |
| `APPLE_API_ISSUER` | macOS | du | optional |
| `APPLE_API_KEY` | macOS | du | optional |
| `APPLE_API_KEY_P8` | macOS | du (base64 der `.p8`) | optional |
| `GITHUB_TOKEN` | alle | automatisch von GitHub | ✅ (nichts zu tun) |

Endpoint/Account/Profil (Azure) sind **keine Secrets** — die gibst du mir als Klartext für `tauri.conf.json`.

---

## 8. base64-Encode-Spickzettel

```bash
# macOS .p12  → base64 (auf einem Mac)
openssl base64 -A -in certificate.p12 -out certificate-base64.txt

# macOS App-Store-Connect .p8 → base64 (auf einem Mac/Linux)
openssl base64 -A -in AuthKey_XXXXXX.p8 -out apikey-base64.txt
```

```powershell
# Windows: beliebige Datei → base64 (PowerShell)
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certificate.pfx")) | Set-Content cert-base64.txt
# oder klassisch:
certutil -encode certificate.pfx base64cert.txt
```

Inhalt der jeweiligen `*-base64.txt` **komplett** als Secret-Wert einfügen.

---

## 9. Kosten- und Vorlaufzeit-Übersicht

> Preise = Stand der Recherche, **vor Kauf bestätigen** (§12). Größenordnungen, keine Zusagen.

| Posten | Kosten | Vorlaufzeit | Pflicht? |
|---|---|---|---|
| Azure Trusted Signing | ~10 USD/Monat + Azure-Grundgebühr | Identitätsprüfung Tage–1 Woche | Windows: ja |
| Azure-Subscription | gering (Pay-as-you-go) | sofort | Windows: ja |
| Apple Developer Program | 99 USD/Jahr | Individual 24–48 h | macOS: nur wenn gewünscht |
| D-U-N-S-Nummer (nur Firmen-Enrollment) | kostenlos | 1–3 Wochen | nur bei Firmen-Apple-Account |
| Mac für CSR/Export | — (leihweise ok) | einmalig | macOS: ja |
| Klassisches OV/EV-Cert (Weg B) | ~200–600 EUR/Jahr | Tage | nur falls statt Azure |
| Linux-Signing | 0 | 0 | nein |

---

## 10. Was ICH in 17d baue (sobald Secrets stehen)

`.github/workflows/release.yml` — Auslöser: Tag `v*`. Struktur (an eure CI-Konventionen angelehnt: `checkout@v4`, `pnpm` v9, Node 20, working-directory `klein-buch`):

```yaml
name: release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - { os: windows-latest, target: x86_64-pc-windows-msvc }
          - { os: macos-13,       target: x86_64-apple-darwin }
          - { os: macos-14,       target: aarch64-apple-darwin }
          - { os: ubuntu-latest,  target: x86_64-unknown-linux-gnu }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      # ... Toolchain, Sidecar-Build (build-sidecar.sh / .ps1), pnpm install ...
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # Windows (Azure Trusted Signing):
          AZURE_CLIENT_ID:     ${{ secrets.AZURE_CLIENT_ID }}
          AZURE_CLIENT_SECRET: ${{ secrets.AZURE_CLIENT_SECRET }}
          AZURE_TENANT_ID:     ${{ secrets.AZURE_TENANT_ID }}
          # macOS (nur falls aktiviert):
          APPLE_CERTIFICATE:          ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY:     ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_API_ISSUER:           ${{ secrets.APPLE_API_ISSUER }}
          APPLE_API_KEY:              ${{ secrets.APPLE_API_KEY }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'Klein.Buch ${{ github.ref_name }}'
          releaseDraft: true        # erst Draft, du gibst manuell frei
          prerelease: false
```

**Reihenfolge der Umsetzung (deine Wahl in der Rückfrage):**

1. **Zuerst unsigniert als Draft** — `release.yml` baut Bundles für alle vier Triples ohne Signatur-Env, erzeugt einen **Draft-Release** mit Checksums. Damit ist die Pipeline verifizierbar grün, ohne dass du auf Zertifikate wartest.
2. **Danach Windows-Signing scharf** — sobald die drei `AZURE_*`-Secrets + Endpoint/Account/Profil da sind, hänge ich den `signCommand` ein.
3. **macOS-Signing** nur, wenn du den Apple-Weg gehst.

---

## 11. Empfohlene Reihenfolge für dich (Checkliste)

- [ ] Entscheiden: **nur Windows** (meine Empfehlung) oder **Windows + macOS**?
- [ ] Azure-Subscription + Trusted Signing Account + Certificate Profile anlegen
- [ ] Identitätsprüfung als **Organisation** (Wildbach) starten — **Eligibility vorher prüfen**
- [ ] App Registration anlegen, Rolle „Certificate Profile Signer" zuweisen
- [ ] `AZURE_CLIENT_ID/SECRET/TENANT_ID` als GitHub-Secrets setzen
- [ ] Endpoint/Account/Profil-Namen an mich (Klartext)
- [ ] (optional macOS) Apple Developer Program + Developer ID Application + API-Key, Secrets setzen
- [ ] Repo public/private klären (macOS-Runner-Kosten)
- [ ] Mir „Sidecar-JDK = Temurin 21?" bestätigen

---

## 12. Verifikations-Hinweis

Die **Mechanik** (Config-Keys, Secret-Namen, Env-Variablen, Workflow) ist gegen die offiziellen Tauri-2-Docs geprüft. **Preise, Azure-Eligibility-Regeln und Apple-Enrollment-Details ändern sich** — bestätige sie unmittelbar vor Kauf/Anmeldung an der jeweiligen Quelle. Markiere für mich, falls bei der Einrichtung etwas von dieser Anleitung abweicht; dann ziehe ich `release.yml` / `tauri.conf.json` nach.

## 13. Quellen

- Tauri 2 — Windows Code Signing: https://v2.tauri.app/distribute/sign/windows/
- Tauri 2 — macOS Code Signing: https://v2.tauri.app/distribute/sign/macos/
- Tauri 2 — Linux Signing: https://v2.tauri.app/distribute/sign/linux/
- Tauri 2 — GitHub Pipeline: https://v2.tauri.app/distribute/pipelines/github/
- tauri-action: https://github.com/tauri-apps/tauri-action
- trusted-signing-cli: https://github.com/Levminer/trusted-signing-cli
- Azure Trusted Signing Quickstart: https://learn.microsoft.com/en-us/azure/trusted-signing/quickstart
- ADR 0001 (Cross-OS-Sidecar in Block 17): `docs/adr/0001-cross-os-sidecar-bundles-in-block-17.md`
