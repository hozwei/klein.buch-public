// Status einer Anschaffung in der Oberfläche (U4, V3-09): eine Quelle für Liste und
// Detailseite. Reine Funktion ohne Importe aus `#lib`.
//
// Töne nach design.md §4.9: laufende Abschreibung ist der aktive Zustand (success),
// festgeschrieben primary; Anfang und Ende ohne offene Aufgabe neutral — „Noch keine
// AfA“ (vorher orange „neu“, obwohl nichts zu tun ist), „Abgeschrieben“, „Veräußert“.

import type { StatusView } from "../invoices/invoiceStatus.ts";

export interface AssetStatusInput {
  disposed: number;
  bookValueCents: number;
  lockedAt?: string | null;
  lastDepreciationYear?: number | null;
}

export function assetStatusView(a: AssetStatusInput): StatusView {
  if (a.disposed === 1) return { label: "Veräußert", tone: "neutral", strike: false };
  if (a.bookValueCents <= 0) return { label: "Abgeschrieben", tone: "neutral", strike: false };
  if (a.lockedAt) return { label: "Festgeschrieben", tone: "primary", strike: false };
  if (a.lastDepreciationYear != null) return { label: "In Abschreibung", tone: "success", strike: false };
  return { label: "Noch keine AfA", tone: "neutral", strike: false };
}
