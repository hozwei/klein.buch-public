// Status von Kosten in der Oberfläche (U4, V3-09): eine Quelle für Liste, Detailseite
// und Startseite. Reine Funktion ohne Importe aus `#lib`.
//
// Kosten werden beim Erfassen sofort festgeschrieben (ADR 0019); „erfasst“ trug
// darum keine Information. Der Status sagt, ob bezahlt ist — Wörter und Töne nach
// design.md §4.9: Bezahlt success, Unbezahlt warning (braucht noch eine Zahlung),
// Storniert neutral und durchgestrichen.

import type { StatusView } from "../invoices/invoiceStatus.ts";

export function expenseStatusView(e: { status: string; paidDate?: string | null }): StatusView {
  if (e.status === "canceled") return { label: "Storniert", tone: "neutral", strike: true };
  if (!e.paidDate) return { label: "Unbezahlt", tone: "warning", strike: false };
  return { label: "Bezahlt", tone: "success", strike: false };
}
