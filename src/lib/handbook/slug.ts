// Slugifier für Heading-IDs (ADR 0042 Nr. 16, U7-A02): eigenes Modul, damit
// `HelpAnchor` im Start-Bundle ihn ohne `render.ts` (mit `marked`) nutzen kann.
// `render.ts` exportiert ihn weiter.

/**
 * Deutsche-aware Slugifier für Heading-IDs.
 *
 * Ä/Ö/Ü → ae/oe/ue, ß → ss, alles übrige Nicht-Wort-Zeichen wird zu
 * Space normalisiert und mit `-` zusammengezogen. Bewusst lossy, aber
 * deterministisch — gleiches Heading liefert immer denselben Slug.
 */
export function slugifyHeading(text: string): string {
  return text
    .toLowerCase()
    .replace(/ä/g, "ae")
    .replace(/ö/g, "oe")
    .replace(/ü/g, "ue")
    .replace(/ß/g, "ss")
    .replace(/[^a-z0-9\s-]+/g, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean)
    .join("-");
}
