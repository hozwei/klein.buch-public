# GoBD-Archivierung

Quellen: BMF-Schreiben vom 28.11.2019 (GoBD), geändert durch die
BMF-Schreiben vom 11.03.2024 und vom 14.07.2025 (zweite Änderung, u. a.
Aufbewahrung von E-Rechnungen) —
https://www.bundesfinanzministerium.de (Themen → Steuern → Abgabenordnung);
§ 146, § 147 AO — https://www.gesetze-im-internet.de/ao_1977/__147.html ;
§ 14b UStG.

GoBD verlangen Unveränderbarkeit (§ 146 Abs. 4 AO), Nachvollziehbarkeit,
Vollständigkeit, Ordnung und Verfügbarkeit; sie gelten auch bei der
Gewinnermittlung nach § 4 Abs. 3 EStG.

Aufbewahrungsfristen (§ 147 Abs. 3 AO in der Fassung des Vierten
Bürokratieentlastungsgesetzes, BGBl. I 2024 Nr. 323): **10 Jahre** für
Bücher, Aufzeichnungen, Jahresabschlüsse und Arbeitsanweisungen
(§ 147 Abs. 1 Nr. 1 AO), **8 Jahre** für Buchungsbelege (Nr. 4, auch
Rechnungen nach § 14b Abs. 1 UStG; für Belege, deren Frist am 31.12.2024
noch nicht abgelaufen war), 6 Jahre für Handels- und Geschäftsbriefe und
sonstige Unterlagen (Nr. 2, 3, 5). Die Frist beginnt
mit Schluss des Kalenderjahres der letzten Eintragung bzw. Entstehung und
läuft nicht ab, solange die Festsetzungsfrist nicht abgelaufen ist
(§ 147 Abs. 3 und 4 AO). Klein.Buch bewahrt einheitlich 10 Jahre auf und
bietet kein Löschen an (Hard-Rule), weil Datenbank und Archiv zugleich
Aufzeichnungen sind.

E-Rechnungen: aufzubewahren ist der strukturierte Teil (XML); der
Bildteil eines hybriden Formats (ZUGFeRD) nur, wenn er weitergehende
steuerlich relevante Angaben enthält. Klein.Buch archiviert bei
Ausgangsrechnungen PDF und XML, bei Eingang das Original (GoBD in der
Fassung vom 14.07.2025).

Umsetzung: ADR 0006, `docs/architecture/gobd.md` (Trigger, Archiv
write-once mit SHA-256, Audit-Log und Zahlungsbuch append-only, Storno
statt Löschung).
