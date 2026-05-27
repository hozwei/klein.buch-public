# ELSTER-EÜR-Formular — Schema

ELSTER ist das deutsche Steuer-Online-Portal. Die EÜR wird elektronisch
als Anlage zur Einkommensteuererklärung übermittelt. Schema-Definitionen
liegen unter:

https://www.elster.de/elsterweb/eric_dev/api-doc

und in den Taxonomy-Files für das jeweilige Steuerjahr (XBRL).

Klein.Buch's Block 14 implementiert einen **CSV-Export**, der ELSTERs
EÜR-Formular-Importer entspricht. Genaues Mapping (Zeile → Klein.Buch-Feld)
wird in Block 14 ausgearbeitet. Aktuelle EÜR-Anlage 2026 hat Zeilen 11–91
für Einnahmen, Ausgaben (nach Kategorien), AfA, Privatentnahmen,
Gewinn/Verlust.

Direkte ERiC-API-Anbindung (XBRL-Submission ohne Manueles Klicken) ist
**Out of Scope** für v0.1.0. Manuel exportiert CSV und importiert in
ELSTER manuell.
