// G2-DOC.3.2 + G2-DOC.3.4 — Loader für eine einzelne Handbuch-Seite.
//
// Holt den Eintrag aus dem Build-Zeit-Index (`#lib/handbook/index.ts`) und rendert
// den Markdown-Body sofort zu HTML + TOC. Beides wird in `data`
// hochgereicht: das `+page.svelte` setzt das HTML mit `{@html}` ein,
// und das `+layout.svelte` liest `data.toc` (`page.data.toc` aus `$app/state`), um die
// rechte Spalte zu füllen.
//
// 404 statt Redirect: ein toter Anker (z. B. später aus `<HelpAnchor>`
// in G2-DOC.4) soll als Fehler sichtbar werden, nicht stillschweigend
// auf willkommen umgeleitet werden.
//
// 3.4: FAQ-Seiten (Kategorie `faq`) werden nach dem Marked-Render durch
// `transformAsFaq` in eine `<details>`-Accordion-Ansicht überführt; die
// TOC-Anker werden auf `faq-<slug>` umgeschrieben, sodass Deeplinks auf
// einzelne Fragen aus dem Glossar/Recht-Kapitel funktionieren.
//
// U4 (U3-041): Jede Handbuch-Seite beginnt mit `# Titel`. Die Seite zeigt den Titel
// schon im Kopf (`h1`); gerendert stand er darunter ein zweites Mal als Überschrift
// („Angebote“ / „Angebote“). Die führende H1 wird deshalb vor dem Rendern
// abgetrennt und ersetzt den Titel aus dem Front-Matter im Kopf (sie ist teils
// genauer, z. B. „Kleinunternehmer-Regelung (§19 UStG)“). Die Markdown-Dateien
// bleiben unverändert; Suche und Index lesen weiter den ganzen Text.

import { error } from "@sveltejs/kit";
import { getEntry } from "#lib/handbook/index.ts";
import { renderHandbookMarkdown, transformAsFaq } from "#lib/handbook/render.ts";

/**
 * Trennt eine führende ATX-Überschrift erster Ordnung (`# Titel`) ab. Nur, wenn sie
 * vor jedem anderen Inhalt steht (Leerzeilen davor erlaubt); sonst bleibt der Text
 * unverändert und `heading` ist `null`.
 */
function splitLeadingH1(body: string): { heading: string | null; body: string } {
  const m = /^(?:[ \t]*\r?\n)*[ \t]{0,3}#[ \t]+(.+?)(?:[ \t]+#+)?[ \t]*(?:\r?\n|$)/.exec(body);
  if (!m) return { heading: null, body };
  return { heading: m[1].trim(), body: body.slice(m[0].length) };
}

export function load({ params }: { params: { slug: string } }) {
  const entry = getEntry(params.slug);
  if (!entry) {
    // U7 (R2-04): Slug nicht ausgeben — sonst ließe sich über die Adresse beliebiger
    // Text im Handbuch-Layout der eigenen Instanz anzeigen (Inhalts-Spoofing).
    throw error(404, "Diese Handbuch-Seite gibt es nicht.");
  }
  const { heading, body } = splitLeadingH1(entry.body);
  const title = heading || entry.title;
  const rendered = renderHandbookMarkdown(body);
  if (entry.category === "faq") {
    const faq = transformAsFaq(rendered.html, rendered.toc);
    return { entry, title, html: faq.html, toc: faq.toc };
  }
  return { entry, title, html: rendered.html, toc: rendered.toc };
}
