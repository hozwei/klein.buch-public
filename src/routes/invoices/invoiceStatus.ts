// Status einer Rechnung in der Oberfläche (U4, V2-03/V2-14/V1-03): eine Quelle
// für Liste und Detailseite. Reine Funktionen ohne Importe aus `#lib`, damit sie
// ohne Browser testbar sind. Gebucht und geprüft wird ausschließlich im Server.
//
// - „Überfällig“ (danger) wie auf der Startseite: ausgestellt, kein Storno-Beleg,
//   nicht storniert, offener Betrag > 0 und Fälligkeit vor dem heutigen Berliner
//   Tag (`todayIso` aus `todayBerlin(dayClock.now)`).
// - Wörter und Töne nach design.md §4.9: Entwurf neutral, Ausgestellt primary,
//   Versendet info, Teilbezahlt warning, Bezahlt success, Überfällig danger,
//   Storniert/Verworfen neutral und durchgestrichen.

export type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

export interface StatusView {
  label: string;
  tone: Tone;
  strike: boolean;
}

/** Die Felder, die Liste (`InvoiceListItem`) und Detail (`Invoice`) gemeinsam haben. */
export interface InvoiceStatusInput {
  status: string;
  voidedAt?: string | null;
  isStornoFor?: string | null;
  dueDate?: string | null;
  grossAmountCents: number;
  paidAmountCents: number;
}

/** Ausgestellt, offen und die Fälligkeit liegt vor heute (Berliner Kalendertag). */
export function isOverdue(inv: InvoiceStatusInput, todayIso: string): boolean {
  if (inv.voidedAt || inv.isStornoFor) return false;
  if (inv.status !== "issued" && inv.status !== "sent" && inv.status !== "partially_paid") return false;
  if (inv.paidAmountCents >= inv.grossAmountCents) return false;
  return !!inv.dueDate && inv.dueDate.slice(0, 10) < todayIso.slice(0, 10);
}

/** Badge-Text und -Ton einer Rechnung; „Überfällig“ geht vor Ausgestellt/Versendet/Teilbezahlt. */
export function invoiceStatusView(inv: InvoiceStatusInput, todayIso: string): StatusView {
  if (inv.voidedAt) return { label: "Verworfen", tone: "neutral", strike: true };
  if (isOverdue(inv, todayIso)) return { label: "Überfällig", tone: "danger", strike: false };
  return statusOf(inv.status);
}

/** Wort und Ton zu einem gespeicherten Status (auch für die Filter-Auswahl). */
export function statusOf(status: string): StatusView {
  switch (status) {
    case "draft":          return { label: "Entwurf", tone: "neutral", strike: false };
    case "issued":         return { label: "Ausgestellt", tone: "primary", strike: false };
    case "sent":           return { label: "Versendet", tone: "info", strike: false };
    case "partially_paid": return { label: "Teilbezahlt", tone: "warning", strike: false };
    case "paid":           return { label: "Bezahlt", tone: "success", strike: false };
    case "canceled":       return { label: "Storniert", tone: "neutral", strike: true };
    default:               return { label: status, tone: "neutral", strike: false };
  }
}

/** Art des Belegs im Titel: „Storno-Beleg ST-…“ bzw. „Rechnung RE-…“ (V2-14). */
export function docKind(inv: { isStornoFor?: string | null }): "Storno-Beleg" | "Rechnung" {
  return inv.isStornoFor ? "Storno-Beleg" : "Rechnung";
}

// ---- Ablehnungen beim Ausstellen/Fertigstellen (V1-03) ----

// Präfixe der Prüf-Ablehnung im Server: alt „Validation failed (2 Fehler): “, neu
// „Die Rechnung kann nicht ausgestellt werden: “ bzw. „Das Angebot kann nicht
// fertiggestellt werden: “ (Gründe jeweils mit „; “ getrennt).
const ISSUE_PREFIX =
  /^(?:Validation failed(?: \(\d+ Fehler\))?|(?:Die Rechnung|Der Storno-Beleg|Das Angebot) kann nicht (?:ausgestellt|fertiggestellt) werden(?: \(\d+ Fehler\))?)\s*:\s*/;

/**
 * Zerlegt eine Prüf-Ablehnung des Servers in einzelne Gründe. `null`, wenn die
 * Meldung keine Prüf-Ablehnung ist (dann bleibt sie ein einzelner Grund bzw. ein
 * technischer Fehler).
 */
export function issueReasons(message: string): string[] | null {
  const m = ISSUE_PREFIX.exec(message.trim());
  if (!m) return null;
  return message
    .trim()
    .slice(m[0].length)
    .split("; ")
    .map((s) => s.trim())
    .filter(Boolean);
}

/** Technischer Fehler (Netz, Server) statt fachlicher Ablehnung: bleibt ein Toast. */
export function isTechnicalError(message: string): boolean {
  return /^(Server nicht erreichbar|Fehler \d{3}$)/.test(message);
}

export type ReasonTarget = "seller" | "buyer" | "draft";

const SELLER_CODES = new Set([
  "SellerNameMissing",
  "SellerAddressIncomplete",
  "SellerMissingTaxIdAndVatId",
  "XRechnungSellerPhoneMissing",
  "XRechnungSellerIdMissing",
]);
const BUYER_CODES = new Set(["BuyerNameMissing", "BuyerAddressIncomplete"]);

/**
 * Wo sich ein Grund beheben lässt: Firmendaten, Kontakt des Empfängers oder der
 * Entwurf selbst. Mit Regel-Code (Prüfliste) eindeutig, sonst am Text.
 */
export function reasonTarget(text: string, code?: string | null): ReasonTarget {
  if (code) {
    if (SELLER_CODES.has(code)) return "seller";
    if (BUYER_CODES.has(code)) return "buyer";
    return "draft";
  }
  if (/Firmendaten|Stammdaten|Verkäufer|Telefonnummer|Steuernummer oder USt-IdNr/.test(text)) return "seller";
  if (/Empfänger/.test(text)) return "buyer";
  return "draft";
}

/** Die Ziele, die eine Liste von Gründen braucht, in fester Reihenfolge. */
export function reasonTargets(reasons: Array<{ text: string; code?: string | null }>): ReasonTarget[] {
  const set = new Set(reasons.map((r) => reasonTarget(r.text, r.code)));
  return (["seller", "buyer", "draft"] as const).filter((t) => set.has(t));
}

/** Jüngster erfolgreicher Versand aus dem E-Mail-Protokoll (Zeitstempel) oder `null`. */
export function lastSentAt(log: Array<{ status: string; createdAt: string }>): string | null {
  let last: string | null = null;
  for (const e of log) {
    if (e.status === "success" && (last === null || e.createdAt > last)) last = e.createdAt;
  }
  return last;
}
