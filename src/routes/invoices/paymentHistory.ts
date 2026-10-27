// Zahlungshistorie einer Rechnung (U4, Warteliste (7)): reine Hilfen für die
// Detail- und die Storno-Seite. Quelle ist `invoice.paymentHistoryJson`
// (`db::repo::invoices::PaymentEntry`, Schlüssel in camelCase). Nur Anzeige —
// gebucht wird ausschließlich über den Server.

import type { Invoice, PaymentAccount } from "#lib/types.ts";
import { iban } from "#lib/format.ts";

export interface PaymentEntry {
  amountCents: number;
  /** Zahldatum `JJJJ-MM-TT` (Zufluss, § 11 EStG). */
  paidDate: string;
  note: string | null;
  recordedAt: string;
  /** Konto des Eingangs (R2-011); fehlt bei älteren Einträgen. */
  accountId?: string | null;
}

/**
 * Liest die Historie. Leer bzw. `null` ergibt `[]`; beschädigtes JSON ergibt
 * `null` (der Server lehnt die nächste Zahlung dann mit klarer Meldung ab).
 */
export function parsePaymentHistory(json: string | null | undefined): PaymentEntry[] | null {
  if (!json) return [];
  try {
    const v: unknown = JSON.parse(json);
    return Array.isArray(v) ? (v as PaymentEntry[]) : null;
  } catch {
    return null;
  }
}

/**
 * Konto einer Zahlung wie in der EÜR (`db::repo::euer`): das Konto des Eintrags,
 * sonst das Eingangskonto der Rechnung.
 */
export function paymentAccountId(
  entry: PaymentEntry,
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
