# App-Icon-Vorlage

`logo-1024.svg` und `logo-1024.png` sind die Vorlage des Klein.Buch-Logos (Petrol `#0d4150`,
1024 × 1024 px). Daraus entstehen `static/favicon.svg` und `static/favicon.png` sowie künftige
App-Icons der Webanwendung; die PDF-Tests (`pdf::snapshot`, `sidecar_real_test`) nutzen die PNG
als Beispiel-Logo. Bis Lauf F6 lagen die Dateien als `icon-source.*` unter `src-tauri/icons/`;
die daraus erzeugten Desktop-Icons der Tauri-Fassung sind entfallen.

Der Ordner heißt bewusst nicht `logo`: Das Veröffentlichen lässt jeden Ordner dieses Namens aus
(Betreiber-Logos), die Tests im öffentlichen CI fänden die Datei sonst nicht (Test
`core_cut_test::files_read_by_tests_survive_publication`).
