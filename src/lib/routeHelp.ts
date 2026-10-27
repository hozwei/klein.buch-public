// F1 öffnet das passende Handbuch-Kapitel (U4, V1-19; ADR 0041 Nr. 161).
//
// Zuordnung über das Präfix der Route (`page.route.id`), längstes Präfix gewinnt.
// Ohne Treffer öffnet F1 die Startseite des Handbuchs (`/help`). Die Slugs sind die
// Dateinamen unter `src-tauri/resources/handbook/` (Test: `routeHelp.test.ts`).

const ROUTE_HELP: Array<[prefix: string, slug: string]> = [
  ["/contacts", "kontakte"],
  ["/invoices/[id]/payment", "zahlung-erfassen"],
  ["/invoices/[id]/cancel", "storno-statt-loeschung"],
  ["/invoices/new", "erste-rechnung-schreiben"],
  ["/invoices", "rechnungen"],
  ["/quotes", "angebote"],
  ["/recurring-invoices", "abo-rechnungen"],
  ["/packages", "pakete-und-anfahrt"],
  ["/expenses/import", "rechnungseingang-per-upload"],
  ["/expenses/new", "kosten-erfassen"],
  ["/expenses", "kosten"],
  ["/assets", "anlagen-und-afa"],
  ["/private-movements", "privatbewegungen"],
  ["/euer/export", "euer-export"],
  ["/euer", "euer"],
  ["/fiscal-year", "geschaeftsjahr-abschluss"],
  ["/notifications", "hinweise"],
  ["/settings/seller", "verkaeuferprofil-anlegen"],
  ["/settings/backup", "backup-und-wiederherstellen"],
  ["/settings/backup-log", "backup-und-wiederherstellen"],
  ["/settings/mail", "konten-und-mail-versand"],
  ["/settings/mail-log", "konten-und-mail-versand"],
  ["/settings/payment-accounts", "konten-und-mail-versand"],
  ["/settings/security", "totp-einrichten-und-wiederherstellen"],
  ["/settings/travel", "pakete-und-anfahrt"],
  ["/settings", "einstellungen"],
];

/** Handbuch-Slug zur Route; `null` = Startseite des Handbuchs. */
export function helpSlugForRoute(routeId: string | null | undefined): string | null {
  if (!routeId) return null;
  let best: [string, string] | null = null;
  for (const entry of ROUTE_HELP) {
    const [prefix] = entry;
    if (routeId === prefix || routeId.startsWith(prefix + "/")) {
      if (!best || prefix.length > best[0].length) best = entry;
    }
  }
  return best ? best[1] : null;
}

/** Alle verwendeten Slugs (für den Test gegen den Handbuch-Index). */
export const ROUTE_HELP_SLUGS: readonly string[] = [...new Set(ROUTE_HELP.map(([, s]) => s))];
