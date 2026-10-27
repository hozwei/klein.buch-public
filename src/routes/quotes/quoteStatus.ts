// Status eines Angebots in der Oberfläche (U4, V1-01): eine Quelle für Liste und
// Detailseite. Reine Funktion ohne Importe aus `#lib`.
//
// Der Server setzt beim Fertigstellen `status = 'sent'` (Festschreiben, ohne
// Versand). Die Oberfläche nennt das „Fertiggestellt“; ob das Angebot wirklich
// verschickt wurde, steht im E-Mail-Protokoll („Versendet am …“ auf der
// Detailseite). Keine Migration, keine Server-Änderung (ADR 0041 Nr. 170).
//
// ADR 0041 Nr. 27/83: ein nie fertiggestelltes Angebot (ohne `lockedAt`) wurde
// verworfen, ein fertiggestelltes storniert.

import type { StatusView } from "../invoices/invoiceStatus.ts";

export function quoteStatusView(q: { status: string; lockedAt?: string | null }): StatusView {
  switch (q.status) {
    case "draft":     return { label: "Entwurf", tone: "neutral", strike: false };
    case "sent":      return { label: "Fertiggestellt", tone: "primary", strike: false };
    case "accepted":  return { label: "Angenommen", tone: "success", strike: false };
    case "rejected":  return { label: "Abgelehnt", tone: "danger", strike: false };
    case "canceled":  return { label: q.lockedAt ? "Storniert" : "Verworfen", tone: "neutral", strike: true };
    case "converted": return { label: "Umgewandelt", tone: "primary", strike: false };
    default:          return { label: q.status, tone: "neutral", strike: false };
  }
}
