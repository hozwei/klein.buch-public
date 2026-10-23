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
| jlink-Module | siehe `versions.json` (`jlink_modules`) im selben Ordner |

Ab der Docker-Fassung (Phase W) ersetzt eine unter Linux gebaute Laufzeit diese
Windows-Laufzeit; deren Herkunft und Quelltext nennt dann `sidecar/versions.json` und die
Lizenzseite des Images.
