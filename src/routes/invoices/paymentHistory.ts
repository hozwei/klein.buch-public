// Zahlungen einer Rechnung (U4, Warteliste (7); seit V2 das Zahlungsbuch,
// ADR 0042 Nr. 7–9): reine Hilfen für Detail-, Zahlungs- und Storno-Seite.
// Quelle ist `InvoiceDetail.payments` (Tabelle `invoice_payments`, Migration
// 0034): Zahlungen (`payment`, Betrag > 0) und Gegenbuchungen (`reversal`,
// „Zahlung stornieren“, Betrag < 0, Verweis und Grund). `paymentHistoryJson`
// ist seit 0034 eingefroren (Altbestand) und wird nicht mehr gelesen. Nur
// Anzeige — gebucht und storniert wird ausschließlich über den Server.

import type { Invoice, InvoicePayment, PaymentAccount } from "#lib/types.ts";
import { iban } from "#lib/format.ts";

/** Mindestlänge des Grundes einer Gegenbuchung (wie `clean_reason` im Server). */
export const REVERSE_REASON_MIN_CHARS = 10;

/** Höchstlänge des Grundes (CHECK in Migration 0034). */
export const REVERSE_REASON_MAX_CHARS = 500;

/**
 * Konto einer Buchung wie in der EÜR (`db::repo::euer`): das Konto der
 * Buchung, sonst das Eingangskonto der Rechnung (Altbestand ohne Konto).
 */
export function paymentAccountId(
  entry: Pick<InvoicePayment, "accountId">,
  invoice: Pick<Invoice, "paidToAccountId">,
): string | null {
  return entry.accountId || invoice.paidToAccountId || null;
}

/** IBAN gekürzt auf Ländercode, Prüfziffer und die letzten vier Stellen („DE89 … 3000“). */
export function shortIban(value: string | null | undefined): string {
  const s = (value ?? "").replace(/\s+/g, "").toUpperCase();
  if (!s) return "";
  if (s.length <= 8) return iban(s);
  return `${s.slice(0, 4)} … ${s.slice(-4)}`;
}

/** Anzeige eines Zahlungskontos: „Geschäftskonto (DE89 … 3000)“. */
export function accountText(id: string | null, accounts: readonly PaymentAccount[]): string {
  if (!id) return "nicht angegeben";
  const a = accounts.find((x) => x.id === id);
  if (!a) return "unbekanntes Konto";
  const short = shortIban(a.iban);
  const base = short ? `${a.label} (${short})` : a.label;
  return a.active === 0 ? `${base}, deaktiviert` : base;
}

/**
 * Zahlungen, die nicht durch eine Gegenbuchung storniert sind (wie
 * `invoice_payments::effective_payments`). Stützt sich auf die Verweise der
 * Gegenbuchungen, nicht auf `reversedById`, damit es auch ohne Lesemodell gilt.
 */
export function effectivePayments(payments: readonly InvoicePayment[]): InvoicePayment[] {
  const reversed = new Set(
    payments.filter((p) => p.kind === "reversal" && p.reversesPaymentId).map((p) => p.reversesPaymentId),
  );
  return payments.filter((p) => p.kind === "payment" && !reversed.has(p.id));
}

/** Jüngstes Zahldatum (`JJJJ-MM-TT`) der nicht stornierten Zahlungen, sonst `null`. */
export function lastEffectivePaidDate(payments: readonly InvoicePayment[]): string | null {
  let max: string | null = null;
  for (const p of effectivePayments(payments)) {
    const d = p.paidDate.slice(0, 10);
    if (max === null || d > max) max = d;
  }
  return max;
}

/**
 * Frühestes Storno-Datum (Lauf F8, § 11 EStG; ADR 0042 Nr. 8): das spätere von
 * Rechnungsdatum und letztem Zahlungseingang. Stornierte Zahlungspaare zählen
 * nicht — eine nie geflossene Zahlung verschiebt keine Erstattung (der Server
 * prüft dasselbe, `last_payment_date`).
 */
export function earliestStornoDate(invoiceDate: string, payments: readonly InvoicePayment[]): string {
  const min = invoiceDate.slice(0, 10);
  const last = lastEffectivePaidDate(payments);
  return last && last > min ? last : min;
}

/** Eine Zeile der Zahlungshistorie mit aufgelösten Verweisen. */
export interface PaymentLine {
  entry: InvoicePayment;
  /** Gegenbuchung: die stornierte Zahlung (fehlt sie, bleibt der Verweis leer). */
  reverses: InvoicePayment | null;
  /** Zahlung: ihre Gegenbuchung, falls storniert. */
  reversedBy: InvoicePayment | null;
}

/**
 * Historie in der Reihenfolge der Erfassung (`seq`) mit den Verweisen zwischen
 * Zahlung und Gegenbuchung. Die Zeilen bleiben vollständig — auch eine
 * stornierte Zahlung steht weiter da (GoBD: nachvollziehbar, nichts verschwindet).
 */
export function paymentLines(payments: readonly InvoicePayment[]): PaymentLine[] {
  const byId = new Map(payments.map((p) => [p.id, p]));
  const reversalOf = new Map<string, InvoicePayment>();
  for (const p of payments) {
    if (p.kind === "reversal" && p.reversesPaymentId) reversalOf.set(p.reversesPaymentId, p);
  }
  return [...payments]
    .sort((a, b) => a.seq - b.seq)
    .map((entry) => ({
      entry,
      reverses:
        entry.kind === "reversal" && entry.reversesPaymentId ? (byId.get(entry.reversesPaymentId) ?? null) : null,
      reversedBy:
        entry.kind === "payment"
          ? (reversalOf.get(entry.id) ?? (entry.reversedById ? (byId.get(entry.reversedById) ?? null) : null))
          : null,
    }));
}

/** Bereinigt wie der Server: getrimmt, ohne Steuerzeichen. */
export function cleanReverseReason(reason: string): string {
  return reason
    .trim()
    .replace(/[\u0000-\u001f\u007f-\u009f]/g, "")
    .trim();
}

/** Fehlertext am Feld „Grund“ der Gegenbuchung, sonst `null`. */
export function reverseReasonError(reason: string): string | null {
  return [...cleanReverseReason(reason)].length < REVERSE_REASON_MIN_CHARS
    ? `Bitte einen Grund angeben (mindestens ${REVERSE_REASON_MIN_CHARS} Zeichen) — er steht an der Gegenbuchung und im Protokoll.`
    : null;
}

/**
 * Ablehnung des Servers, die zum Feld „Grund“ gehört (Mindestlänge), statt ins
 * Banner (ADR 0041: Fehler am Feld).
 */
export function isReasonRejection(message: string): boolean {
  return /^Bitte einen Grund angeben/.test(message.trim());
}
