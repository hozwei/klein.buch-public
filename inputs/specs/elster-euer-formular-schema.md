# ELSTER — Anlage EÜR

Die Gewinnermittlung nach § 4 Abs. 3 EStG wird nach amtlich
vorgeschriebenem Datensatz durch Datenfernübertragung übermittelt
(Anlage EÜR; § 60 Abs. 4 EStDV, § 25 Abs. 4 EStG). Die Vordrucke und die
Anleitung veröffentlicht das BMF je Jahr per BMF-Schreiben; die
Kennzahlen (Kz) bleiben über die Jahre gleich, die Zeilennummern nicht
(2026: ab Kz 100 eine Zeile tiefer als 2024/2025).

Quellen: § 60 Abs. 4 EStDV —
https://www.gesetze-im-internet.de/estdv_1955/__60.html ; BMF-Schreiben zur
Anlage EÜR des jeweiligen Jahres (Liste und Links in
`docs/reference/elster-euer-formular-schema.md`).

**ELSTER hat keinen CSV-Import** für die Anlage EÜR. Eine direkte
Übermittlung ginge nur über die ERiC-Schnittstelle (Registrierung als
Softwarehersteller, Datensatz im ELSTER-XML-Format; XBRL gilt für die
E-Bilanz nach § 5b EStG, nicht für die EÜR). Klein.Buch erzeugt deshalb eine
**Ausfüllhilfe** je Jahr nach Zeile und Kennzahl (CSV und PDF „Anlage EÜR“),
die der Nutzer in „Mein ELSTER“ überträgt — keine Einreichung. Zuordnung,
Kennzahlen und Abgrenzungen: `docs/reference/elster-euer-formular-schema.md`
(ADR 0026, ADR 0039 Nr. 135).

Für den Steuerstatus nach § 19 UStG gilt laut Anleitung: Betriebseinnahmen
als Kleinunternehmer brutto in Kz 111; nach einem Wechsel netto in Kz 112
mit vereinnahmter Umsatzsteuer in Kz 140; maßgeblich ist der Status des
einzelnen Umsatzes (ADR 0042 Nr. 3). Gegenbuchungen stornierter Zahlungen
mindern die Einnahmen im Jahr der ursprünglichen Zahlung (ADR 0042 Nr. 8, 9).

Direkte ERiC-Anbindung bleibt außerhalb des Umfangs.
