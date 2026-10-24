# Hinweise zu Komponenten Dritter

Klein.Buch selbst steht unter der AGPL-3.0-or-later (`LICENSE`, Urheberhinweis in
`COPYRIGHT`). Diese Datei nennt mitgelieferte Komponenten Dritter, deren Lizenz eine
Quellenangabe verlangt. Die vollständige Lizenzübersicht der Bibliotheken erzeugt der
Build (`third-party-licenses.html`).

## Java-Laufzeit der Windows-Releases v2026.5.0 und v2026.5.1

Die Windows-Installer der Releases **v2026.5.0** und **v2026.5.1** und der Ordner
`src-tauri/binaries/klein-buch-java-x86_64-pc-windows-msvc/runtime/` im Quellstand dieser
Releases enthalten eine mit `jlink` verkleinerte Java-Laufzeit:

| Angabe | Wert |
|---|---|
| Hersteller | Microsoft (Microsoft Build of OpenJDK) |
| Version | 21.0.11+10-LTS (`runtime/release`: `JAVA_VERSION="21.0.11"`) |
| Lizenz | GNU General Public License, Version 2, mit Classpath Exception (`GPL-2.0-only WITH Classpath-exception-2.0`); die Lizenz- und Hinweistexte der Module liegen in `runtime/legal/` |
| Quelltext | <https://github.com/microsoft/openjdk-jdk21u/tree/jdk-21.0.11%2B10> (Tag `jdk-21.0.11+10`) |
| jlink-Module | siehe `versions.json` (`jlink_modules`) im selben Ordner; seit Phase W in `src-tauri/sidecar/versions.json` unter `desktop_releases_2026_5` |

Ab der Docker-Fassung (Phase W) ersetzt eine im Image-Build per `jlink` erzeugte Linux-Laufzeit
(Eclipse Temurin) diese Windows-Laufzeit; der Ordner `runtime/` ist aus dem Quellstand
entfernt. Herkunft, Version, Image-Digest und Quelltext der neuen Laufzeit sowie Quelle und
SHA-256 von KoSIT-Validator, Mustang und XRechnung-Konfiguration nennt
`src-tauri/sidecar/versions.json`; die Lizenztexte der Laufzeit liegen im Image unter
`<jre>/legal/`.
