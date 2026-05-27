# EN 16931 — relevante Felder für Klein.Buch

Europäische Norm für elektronische Rechnungen. Definiert das semantische
Datenmodell (BT = Business Term, BG = Business Group) und Cardinality.
Klein.Buch generiert XRechnung 3.0.x (deutsche Profilierung) und liest
auch ZUGFeRD 2.4.0 (EN16931-Profil).

Quelle: https://standards.cen.eu/dyn/www/f?p=204:110:0::::FSP_PROJECT,FSP_ORG_ID:60602,2480

Konkrete Mapping-Tabelle (Klein.Buch-Felder → BT-Codes) wird in Block 3
in `einvoice::generator` ausgebaut. Pflichtfelder: BT-1 (Invoice Number),
BT-2 (Invoice Issue Date), BT-22 (Invoice Note, §19-Klausel), BT-31
(Seller VAT identifier oder BT-32 Seller Tax registration identifier),
BT-44 (Buyer Name), BT-50–55 (Buyer Postal Address), BT-106 (Invoice
total amount without VAT), BT-109 (Invoice total amount), BT-110
(Invoice total VAT amount, bei §19 = 0).
