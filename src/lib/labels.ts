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
  { value: "given_away", label: "Privatentnahme / Geschenk" },
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

// ---- Audit-Protokoll (Lauf 2026.11 U1) -------------------------------------

/**
 * Anzeige-Labels für Audit-Aktionen, die der Server schreibt, die aber in der
 * Label-Tabelle der Seite Einstellungen → Audit-Protokoll
 * (`routes/settings/audit-trail/+page.svelte`, `ACTION_LABELS`) fehlen —
 * Stand 26.10.2026, ermittelt aus den Aufrufen von `audit_log::append(_in)`.
 * Die Seite übernimmt sie mit `{ ...ACTION_LABELS, ...AUDIT_ACTION_LABELS_ADDITIONAL }`
 * (Übergabe an den Eigentümer der Seite). Die Labels `archive.read`,
 * `archive.integrity_missing` und `archive.integrity_unreadable` (Übergabe
 * Lauf F, W13-HASH-UI-LABELS) stehen dort bereits.
 */
export const AUDIT_ACTION_LABELS_ADDITIONAL: Record<string, string> = {
  "asset.update": "Anlage geändert",
  "attachment.add": "Anhang hinzugefügt",
  "backup.daily_time_set": "Uhrzeit des Tages-Backups geändert",
  "backup.history_rescanned": "Backup-Verlauf neu eingelesen",
  "backup.mirror.ok": "Backup extern gespiegelt",
  "backup.mirror.failed": "Externe Spiegelung fehlgeschlagen",
  "backup.mirror.skipped": "Externe Spiegelung ausgesetzt",
  "backup.passphrase.setup": "Daten-Passwort eingerichtet",
  "backup.requested": "Backup angefordert",
  "backup.rotation.prune": "Alte Backups aufgeräumt",
  "backup.skipped_locked": "Backup übersprungen (gesperrt)",
  "backup.targets.set": "Backup-Ziele geändert",
  "backup.unlock.ok": "Datenbank entsperrt",
  "depreciation.disposal_partial": "Abschreibung im Abgangsjahr gebucht",
  "depreciation.reset_for_disposal": "Abschreibung für Abgang zurückgesetzt",
  "expense.payment_set": "Zahlung der Kosten erfasst",
  "fiscal_year.lock_restored_after_restore": "Jahresabschluss nach Wiederherstellung übernommen",
  "invoice.cancel_pair_completed": "Storno abgeschlossen",
  "invoice.lock.backup_pending": "Festgeschrieben, Backup ausstehend",
  "legal_document.activate": "Rechtstext aktiviert",
  "legal_document.deactivate": "Rechtstext deaktiviert",
  "legal_document.upload": "Rechtstext hochgeladen",
  "mail.log_failed": "Versandprotokoll unvollständig",
  "mail_account.create": "Postfach angelegt",
  "mail_account.delete": "Postfach gelöscht",
  "mail_account.test_send": "Test-Mail gesendet",
  "mail_account.update": "Postfach geändert",
  "migration.export.tamper_detected": "Migrations-Export: Manipulation erkannt",
  "numbering.raised_after_restore": "Nummernkreis nach Wiederherstellung angehoben",
  "numbering.restored_draft_confirmed": "Vorgemerkter Entwurf bestätigt",
  "numbering.restored_draft_flagged": "Entwurf nach Wiederherstellung vorgemerkt",
  "numbering.restored_draft_reflagged": "Entwurf erneut vorgemerkt",
  "package.archive": "Paket archiviert",
  "package.create": "Paket angelegt",
  "package.new_revision": "Neue Paket-Version",
  "package.reactivate": "Paket reaktiviert",
  "package.rollback": "Paket auf ältere Version zurückgesetzt",
  "package_catalog.render": "Paket-Broschüre erstellt",
  "package_catalog.sent": "Paket-Broschüre versendet",
  "package_category.create": "Paket-Kategorie angelegt",
  "package_category.update": "Paket-Kategorie geändert",
  "payment_account.activate": "Zahlungskonto aktiviert",
  "payment_account.create": "Zahlungskonto angelegt",
  "payment_account.deactivate": "Zahlungskonto deaktiviert",
  "payment_account.seed_defaults": "Standard-Zahlungskonten angelegt",
  "payment_account.update": "Zahlungskonto geändert",
  "quote.attachment_added": "Anhang zum Angebot hinzugefügt",
  "quote.bundle_opened": "Angebots-Paket geöffnet",
  "quote.conversion_reverted": "Umwandlung in Rechnung zurückgenommen",
  "quote.pdf_generated": "Angebots-PDF erzeugt",
  "recurring.activate": "Kosten-Abo aktiviert",
  "recurring.backlog_confirmed": "Kosten-Abo: Rückstand nachgebucht",
  "recurring.backlog_skipped": "Kosten-Abo: Rückstand übersprungen",
  "recurring.create": "Kosten-Abo angelegt",
  "recurring.deactivate": "Kosten-Abo pausiert",
  "recurring.markers_cleared_after_restore": "Abo-Termine nach Wiederherstellung zurückgesetzt",
  "recurring.period_skipped": "Kosten-Abo: Termin übersprungen",
  "recurring.update": "Kosten-Abo geändert",
  "recurring_invoice.activate": "Abo-Vorlage aktiviert",
  "recurring_invoice.backlog_confirmed": "Abo-Rechnungen: Rückstand erstellt",
  "recurring_invoice.backlog_skipped": "Abo-Rechnungen: Rückstand übersprungen",
  "recurring_invoice.create": "Abo-Vorlage angelegt",
  "recurring_invoice.deactivate": "Abo-Vorlage pausiert",
  "recurring_invoice.run": "Abo-Rechnung erstellt",
  "recurring_invoice.update": "Abo-Vorlage geändert",
  "settings.travel.update": "Kilometersatz geändert",
};

/** Anzeigename einer PDF-Vorlage (U1: Detailseiten zeigten „default“). */
const PDF_TEMPLATE_LABELS: Record<string, string> = {
  default: "Standard",
  modern: "Modern",
  klassisch: "Klassisch",
  minimal: "Minimal",
};
export function pdfTemplateLabel(name: string | null | undefined): string {
  if (!name) return "—";
  return PDF_TEMPLATE_LABELS[name] ?? name;
}
