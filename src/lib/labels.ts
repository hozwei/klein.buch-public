// Deutsche Anzeige-Labels für Enums (Block 9/10/12). Reihenfolge = UI-Reihenfolge.
import type {
  DepreciationMethod,
  DisposalType,
  ExpenseCategory,
  Frequency,
  MovementType,
  PaymentAccountType,
} from "./types";

export const EXPENSE_CATEGORIES: { value: ExpenseCategory; label: string }[] = [
  { value: "office", label: "Bürobedarf" },
  { value: "software", label: "Software / Lizenzen" },
  { value: "hardware", label: "Hardware" },
  { value: "travel", label: "Reisekosten" },
  { value: "services", label: "Fremdleistungen" },
  { value: "goods", label: "Wareneinkauf" },
  { value: "communications", label: "Telefon / Internet" },
  { value: "vehicle", label: "Kfz / Fahrzeug" },
  { value: "rent", label: "Miete / Raumkosten" },
  { value: "insurance", label: "Versicherungen / Beiträge" },
  { value: "training", label: "Fortbildung" },
  { value: "fees", label: "Gebühren / Bankspesen" },
  { value: "marketing", label: "Werbung / Marketing" },
  { value: "other", label: "Sonstiges" },
];

export function expenseCategoryLabel(value: string): string {
  return EXPENSE_CATEGORIES.find((c) => c.value === value)?.label ?? value;
}

export const MOVEMENT_TYPES: { value: MovementType; label: string }[] = [
  { value: "entnahme", label: "Privatentnahme" },
  { value: "einlage", label: "Privateinlage" },
];

export function movementTypeLabel(value: string): string {
  return MOVEMENT_TYPES.find((m) => m.value === value)?.label ?? value;
}

export const PAYMENT_ACCOUNT_TYPES: { value: PaymentAccountType; label: string }[] = [
  { value: "bank", label: "Bankkonto" },
  { value: "cash", label: "Bargeld-Kasse" },
  { value: "paypal", label: "PayPal" },
  { value: "stripe", label: "Stripe" },
  { value: "other", label: "Sonstiges" },
];

export const FREQUENCIES: { value: Frequency; label: string }[] = [
  { value: "monthly", label: "monatlich" },
  { value: "quarterly", label: "vierteljährlich" },
  { value: "semiannually", label: "halbjährlich" },
  { value: "annually", label: "jährlich" },
];

export function frequencyLabel(value: string): string {
  return FREQUENCIES.find((f) => f.value === value)?.label ?? value;
}

// ---- Anlagen / AfA (Block 12) ----------------------------------------------

export const DEPRECIATION_METHODS: { value: DepreciationMethod; label: string }[] = [
  { value: "linear", label: "Lineare Abschreibung (über die Nutzungsdauer)" },
  { value: "gwg_sofort", label: "Sofortabschreibung (geringwertig, ≤ 800 € netto)" },
  { value: "computer_special_2021", label: "Computer/Software (1 Jahr, faktisch sofort)" },
];

export function depreciationMethodLabel(value: string): string {
  return DEPRECIATION_METHODS.find((m) => m.value === value)?.label ?? value;
}

/** Kurzform für Tabellen/Badges. */
export function depreciationMethodShort(value: string): string {
  switch (value) {
    case "linear":
      return "linear";
    case "gwg_sofort":
      return "Sofort (GWG)";
    case "computer_special_2021":
      return "Sofort (Computer)";
    default:
      return value;
  }
}

export const DISPOSAL_TYPES: { value: DisposalType; label: string }[] = [
  { value: "sale", label: "Verkauf" },
  { value: "scrap", label: "Verschrottung / Entsorgung" },
  { value: "given_away", label: "Verschenkt / Privatentnahme" },
];

export function disposalTypeLabel(value: string | null): string {
  return DISPOSAL_TYPES.find((d) => d.value === value)?.label ?? (value ?? "—");
}

// ---- E-Rechnung-Empfang (Block 11) -----------------------------------------

/** Lesbarer Name des erkannten E-Rechnungs-Formats. */
export function einvoiceSourceLabel(value: string): string {
  switch (value) {
    case "zugferd":
      return "ZUGFeRD / Factur-X (PDF mit eingebetteter Rechnung)";
    case "xrechnung-cii":
      return "XRechnung (CII-Format)";
    case "xrechnung-ubl":
      return "XRechnung (UBL-Format)";
    default:
      return value;
  }
}

/** Lesbarer Validierungs-Status (KoSIT). */
export function validationStatusLabel(value: string | null): string {
  switch (value) {
    case "passed":
      return "gültig";
    case "warning":
      return "gültig mit Hinweisen";
    case "failed":
      return "formale Mängel";
    default:
      return "nicht geprüft";
  }
}
