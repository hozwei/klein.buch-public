// Seitentitel (U4, WCAG 2.4.2; ADR 0041 Nr. 160).
//
// Jede Seite hat einen eigenen Titel „<Seite> – Klein.Buch“. Das Root-Layout setzt
// ihn in `<svelte:head>`; die Quelle ist, in dieser Reihenfolge:
//   1. die Anmelde-Maske (`gate`, solange die App gesperrt ist),
//   2. eine Fehlerseite („Seite nicht gefunden“),
//   3. der Titel der PageBar (`page`, setzt `PageBar.svelte` selbst),
//   4. der Kapiteltitel einer Handbuchseite (`page.data.title`),
//   5. ein Ersatztitel je Route (Tabelle unten), solange eine Detailseite lädt
//      oder eine Seite keine PageBar hat.
// SvelteKits Routen-Ansage liest `document.title` nach jeder Navigation — ein
// gleichbleibender Titel hieße: Screenreader hören beim Seitenwechsel nichts.

export const APP_NAME = "Klein.Buch";

/**
 * Zustand der Titelquellen. Objekt statt Top-Level-`let` (Mutationen sind über
 * Modulgrenzen reaktiv, Neuzuweisungen nicht; Muster wie toast.svelte.ts).
 */
export const titleState = $state<{ page: string | null; owner: number; gate: string | null }>({
  page: null,
  owner: 0,
  gate: null,
});

let nextOwner = 1;

/** Kennung einer Titelquelle (je PageBar eine), damit ein verschwindender alter
 *  Seitenkopf den Titel der neuen Seite nicht löscht. */
export function claimPageTitle(): number {
  return nextOwner++;
}

export function setPageTitle(owner: number, title: string | null | undefined): void {
  titleState.owner = owner;
  titleState.page = title?.trim() ? title.trim() : null;
}

export function releasePageTitle(owner: number): void {
  if (titleState.owner !== owner) return;
  titleState.owner = 0;
  titleState.page = null;
}

/** Titel der Anmelde-Maske (`null` = keine Maske). */
export function setGateTitle(title: string | null): void {
  titleState.gate = title;
}

/**
 * Ersatztitel je Route (Glossar-Begriffe wie Navigation und Einstellungen-Hub).
 * Detailseiten tragen die Art ohne Nummer; die PageBar verfeinert sie, sobald der
 * Beleg geladen ist („Rechnung RE-2026-0042“).
 */
const ROUTE_TITLES: Record<string, string> = {
  "/": "Start",
  "/contacts": "Kontakte",
  "/contacts/new": "Neuer Kontakt",
  "/contacts/[id]": "Kontakt",
  "/invoices": "Rechnungen",
  "/invoices/new": "Neue Rechnung",
  "/invoices/[id]": "Rechnung",
  "/invoices/[id]/edit": "Entwurf bearbeiten",
  "/invoices/[id]/payment": "Zahlung erfassen",
  "/invoices/[id]/send": "Rechnung senden",
  "/invoices/[id]/cancel": "Rechnung stornieren",
  "/quotes": "Angebote",
  "/quotes/new": "Neues Angebot",
  "/quotes/[id]": "Angebot",
  "/quotes/[id]/send": "Angebot versenden",
  "/quotes/[id]/convert": "Angebot in Rechnung umwandeln",
  "/recurring-invoices": "Abo-Rechnungen",
  "/recurring-invoices/new": "Abo-Vorlage",
  "/packages": "Pakete",
  "/packages/new": "Neues Paket",
  "/packages/[id]": "Paket",
  "/packages/brochure": "Paket-Broschüre",
  "/expenses": "Kosten",
  "/expenses/new": "Kosten erfassen",
  "/expenses/[id]": "Kosten",
  "/expenses/import": "E-Rechnung importieren",
  "/expenses/recurring": "Kosten-Abos",
  "/expenses/recurring/new": "Kosten-Abo",
  "/assets": "Anschaffungen",
  "/assets/new": "Neue Anschaffung",
  "/assets/[id]": "Anschaffung",
  "/assets/[id]/dispose": "Abgang buchen",
  "/private-movements": "Privatbewegungen",
  "/private-movements/new": "Neue Privatbewegung",
  "/private-movements/[id]": "Privatbewegung",
  "/euer": "Steuer-Übersicht (EÜR)",
  "/euer/export": "EÜR-Export",
  "/fiscal-year": "Geschäftsjahr",
  "/notifications": "Hinweise",
  "/notifications/rules": "Hinweis-Regeln",
  "/settings": "Einstellungen",
  "/settings/seller": "Meine Firmendaten",
  "/settings/payment-accounts": "Konten",
  "/settings/travel": "Anfahrtspauschale",
  "/settings/pdf-templates": "Rechnungs-Layout",
  "/settings/legal": "AGB & Datenschutz",
  "/settings/mail": "E-Mail-Versand",
  "/settings/mail-log": "E-Mail-Protokoll",
  "/settings/security": "Sicherheit & Anmeldung",
  "/settings/backup": "Backups",
  "/settings/backup-log": "Backup-Protokoll",
  "/settings/audit-trail": "Protokoll & Datensicherheit",
  "/settings/migration-export": "Daten exportieren",
  "/help": "Handbuch",
  "/help/[slug]": "Handbuch",
  "/design": "Musterseite Design-System",
};

/** Ersatztitel einer Route (`page.route.id`); `null`, wenn unbekannt. */
export function routeTitle(routeId: string | null | undefined): string | null {
  if (!routeId) return null;
  return ROUTE_TITLES[routeId] ?? null;
}

/** „<Seite> – Klein.Buch“; enthält der Teil den App-Namen schon, bleibt er allein. */
export function documentTitle(part: string | null | undefined): string {
  const p = part?.trim();
  if (!p) return APP_NAME;
  return p.includes(APP_NAME) ? p : `${p} – ${APP_NAME}`;
}
