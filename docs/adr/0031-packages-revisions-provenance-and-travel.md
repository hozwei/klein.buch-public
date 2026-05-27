# ADR 0031 — Pakete (versioniert) + Anfahrt

**Status:** Akzeptiert · 2026-05-23 · Phase 3 (Blöcke P1–P4). Migrationen `0016_travel_cost`, `0019_packages`, `0020_package_item_provenance`, `0021_package_item_title`.

## Kontext

Manuels IT-Dienstleistung verkauft wiederkehrende Leistungs-**Pakete** (z. B.
„Wartung Basis", „Cloud-Backup") und berechnet **Anfahrt**. Pakete ändern über
die Zeit Preis und Inhalt, müssen also versionierbar sein — aber eine einmal in
ein Angebot/eine Rechnung übernommene Position darf sich **nie nachträglich**
ändern (GoBD: der ausgestellte Beleg ist fix). Reiche Beschreibungen sollen auf
dem PDF gut aussehen, dürfen aber die EN-16931-Norm des XML nicht brechen.
Anfahrt soll **offline** funktionieren (local-first, kein Karten-Dienst).

## Entscheidung

1. **`package_revisions` sind append-only + unveränderlich** (DB-Trigger,
   `legal_documents`-Muster). „Paket bearbeiten" = **neue Revision**; „Rollback"
   = neue Revision, die eine alte kopiert. Nie Update/Delete einer Revision.
2. **Provenienz statt Live-Bindung:** eine ins Beleg eingefügte Paket-Position
   ist eine **Kopie/Snapshot** + Soft-Zeiger `source_package_id` /
   `source_package_revision`. Bekommt das Paket später eine neue Revision,
   ändert sich die Beleg-Position **nicht**.
3. **„Paket anpassen" = sauberer Bruch:** beim Editieren werden `source_package_*`
   auf NULL gesetzt → reine Custom-Position, kein Rest-Hinweis.
4. **Reiches Markup nur aufs PDF:** `description_markup` (Markdown-Subset) treibt
   den PDF-Block; `to_typst` ist **AST-only mit Escaping** (Typst-Injection-Schutz),
   kein HTML/Typst-Passthrough. Das XRechnung-XML (**BT-154**) bekommt **immer**
   den geplätteten Klartext aus `description`.
5. **Katalog-Broschüre ist KEIN §14-Beleg:** kein Nummernkreis, **kein**
   write-once-Archiv; Versand nur im append-only `email_log` protokolliert.
6. **Anfahrt:** km × Satz erzeugt eine **normale** Beleg-Position; **kein**
   Geocoding/Karten-Dienst — km werden manuell eingegeben.
7. **§19 erbt:** Revisionen + Anfahrt-Position tragen `tax_category_code 'E'` / 0 %,
   solange `is_kleinunternehmer = true` (ADR 0005). Preise sind Netto-Cent.

## Konsequenzen

- Pakete sind versionssicher **und** Belege GoBD-fest — Preisänderungen wirken nur
  vorwärts, nie rückwirkend auf bestehende Belege.
- PDF darf reich/formatiert sein, das XML bleibt EN-16931-normkonform; die
  AST-only-Konvertierung schließt Injection aus.
- Die Broschüre ist frei versendbar ohne Beleg-Overhead (kein Nummernkreis verbraucht).
- Anfahrt funktioniert offline; Preis: km-Eingabe ist manuell statt automatisch.

## Alternativen

| Option | Contra |
|---|---|
| Live-Bindung (Beleg referenziert aktuelle Revision) | Preisänderung würde ausgestellte Belege rückwirkend ändern → GoBD-Verstoß |
| HTML/Typst-Passthrough im Markup | Code-Injection ins PDF; XML-Norm-Bruch |
| Markup auch ins XML (BT-154) | EN-16931 erwartet Klartext → Validierungsfehler |
| Katalog als nummerierter Beleg | verbraucht Nummernkreis, GoBD-Archiv-Overhead für ein reines Marketing-PDF |
| Geocoding/Karten-API für km | bricht local-first/offline, externe Abhängigkeit + Datenschutz |

## Referenzen

`domain::package` (pulldown-cmark, AST→Typst), `domain::travel`,
`db::repo::packages` (`update_as_new_revision`, `rollback`, `package_materialize_item`),
`commands::packages` (`send_catalog_core`, `render_package_catalog`), Frontend
`routes/packages/**`, `lib/{PackageItemAdder,MarkdownEditor,TravelLineAdder}.svelte`;
Migrationen `0016`/`0019`/`0020`/`0021`; ADR 0005 (§19), ADR 0006 (GoBD), ADR 0018
(write-once/legal-documents-Muster). Review-Befunde KB-0046/0047.
