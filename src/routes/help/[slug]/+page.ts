// G2-DOC.3.2 + G2-DOC.3.4 — Loader für eine einzelne Handbuch-Seite.
//
// Holt den Eintrag aus dem Build-Zeit-Index (`$lib/handbook`) und rendert
// den Markdown-Body sofort zu HTML + TOC. Beides wird in `data`
// hochgereicht: das `+page.svelte` setzt das HTML mit `{@html}` ein,
// und das `+layout.svelte` liest `data.toc` (`$page.data.toc`), um die
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

import { error } from "@sveltejs/kit";
import { getEntry } from "$lib/handbook";
import { renderHandbookMarkdown, transformAsFaq } from "$lib/handbook/render";

export function load({ params }: { params: { slug: string } }) {
  const entry = getEntry(params.slug);
  if (!entry) {
    throw error(404, `Handbuch-Seite "${params.slug}" nicht gefunden.`);
  }
  const rendered = renderHandbookMarkdown(entry.body);
  if (entry.category === "faq") {
    const faq = transformAsFaq(rendered.html, rendered.toc);
    return { entry, html: faq.html, toc: faq.toc };
  }
  return { entry, html: rendered.html, toc: rendered.toc };
}
