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
  "mail.log_failed": "E-Mail-Protokoll unvollständig",
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
  "seller_profile_return_to_paragraph_19": "Rückkehr zu §19 (früherer Schalter)",
  "seller_profile_waive_paragraph_19": "Wechsel zur Regelbesteuerung (früherer Schalter)",
  "settings.travel.update": "Kilometersatz geändert",
  // ADR 0042 Nr. 1: §19-Statushistorie (Migration 0033).
  "tax_status.changed": "Steuerstatus eingetragen",
  "tax_status.migrated": "Steuerstatus übernommen (Migration)",
  "tax_status.revoked": "Steuerstatus-Eintrag widerrufen",
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

/** Alle bekannten Vorlagen in Anzeige-Reihenfolge (Warteliste (17): zentral statt je Seite). */
export const PDF_TEMPLATE_NAMES: readonly string[] = Object.keys(PDF_TEMPLATE_LABELS);

/**
 * Text der Option „default“ in einer Vorlagen-Auswahl (ADR 0041 Nr. 28): Der Server
 * liest „default“ als Platzhalter für die Standardvorlage aus den Einstellungen.
 * Beispiel: „Wie in den Einstellungen (Modern)“.
 */
export function pdfTemplateDefaultOptionLabel(settingsDefault: string | null | undefined): string {
  return `Wie in den Einstellungen (${pdfTemplateLabel(settingsDefault || "default")})`;
}

// ---- Mengeneinheiten (U4, U1-060) -------------------------------------------

/**
 * Häufige Mengeneinheiten nach UN/ECE Recommendation 20 (Code-Liste der
 * XRechnung) mit deutschem Klartext. Gespeichert und ins XML geschrieben wird
 * immer der Code; die Oberfläche zeigt den Klartext (Auswahl „Stunde“ statt
 * „HUR“). Unbekannte Codes (z. B. aus importierten E-Rechnungen) bleiben
 * erhalten und erscheinen als Code.
 */
export const UNIT_CODES: { value: string; label: string; short: string }[] = [
  { value: "C62", label: "Stück", short: "Stk." },
  { value: "HUR", label: "Stunde", short: "Std." },
  { value: "DAY", label: "Tag", short: "Tag" },
  { value: "WEE", label: "Woche", short: "Wo." },
  { value: "MON", label: "Monat", short: "Mon." },
  { value: "ANN", label: "Jahr", short: "Jahr" },
  { value: "LS", label: "Pauschale", short: "pausch." },
  { value: "KMT", label: "Kilometer", short: "km" },
  { value: "MIN", label: "Minute", short: "Min." },
  { value: "MTR", label: "Meter", short: "m" },
  { value: "MTK", label: "Quadratmeter", short: "m²" },
  { value: "KGM", label: "Kilogramm", short: "kg" },
  { value: "LTR", label: "Liter", short: "l" },
  { value: "SET", label: "Satz", short: "Satz" },
];

/** Klartext einer Einheit („Stunde“); unbekannte Codes unverändert. */
export function unitLabel(code: string | null | undefined): string {
  if (!code) return "—";
  return UNIT_CODES.find((u) => u.value === code)?.label ?? code;
}

/** Kurzform für Tabellen und Karten („Std.“); unbekannte Codes unverändert. */
export function unitShort(code: string | null | undefined): string {
  if (!code) return "—";
  return UNIT_CODES.find((u) => u.value === code)?.short ?? code;
}

/**
 * Optionen einer Einheiten-Auswahl: die bekannten Einheiten, dazu der aktuelle
 * Code, falls er nicht in der Liste steht (sonst verlöre das Feld still seinen Wert).
 */
export function unitOptions(current?: string | null): { value: string; label: string }[] {
  const list = UNIT_CODES.map(({ value, label }) => ({ value, label }));
  if (current && !list.some((u) => u.value === current)) {
    // U7 (R1-03): Ein formal gültiger, aber nicht gelisteter Code bleibt wählbar;
    // der Zusatz zeigt, dass er nicht aus der Liste stammt (Hinweis statt Sperre).
    list.push({ value: current, label: isFormalUnitCode(current) ? `${current} (Code, nicht in der Liste)` : current });
  }
  return list;
}

/**
 * Formal ein Code nach UN/ECE Rec 20/21 (U7, R1-03): zwei bis drei Zeichen aus
 * Großbuchstaben und Ziffern (z. B. „H87“, „E48“, „P1“, „XPP“). Ob die Codeliste der
 * XRechnung ihn kennt, prüft der KoSIT-Validator beim Ausstellen.
 */
export function isFormalUnitCode(code: string | null | undefined): boolean {
  return !!code && /^[A-Z0-9]{2,3}$/.test(code);
}

/** Steht der Code in der Einheiten-Liste (`UNIT_CODES`)? */
export function isKnownUnit(code: string | null | undefined): boolean {
  return !!code && UNIT_CODES.some((u) => u.value === code);
}

// Vergleichsschlüssel für Freitext-Einheiten: klein, ohne Punkte und Leerzeichen,
// Umlaute ausgeschrieben („Stück“ = „stueck“, „Std.“ = „std“).
function unitKey(s: string): string {
  return s
    .toLocaleLowerCase("de")
    .replace(/[.\s]/g, "")
    .replace(/ä/g, "ae")
    .replace(/ö/g, "oe")
    .replace(/ü/g, "ue")
    .replace(/ß/g, "ss");
}

/**
 * Passende Einheit aus der Liste zu einem Freitext oder kleingeschriebenen Code
 * („Pauschal“ → Pauschale, „Std.“ → Stunde, „hur“ → Stunde); sonst `null`.
 * Gleich bei Code, Klartext oder Kurzform; ab drei Zeichen auch, wenn einer der
 * Anfang des anderen ist („Stunden“, „Pauschal“).
 */
export function suggestUnit(text: string | null | undefined): { value: string; label: string } | null {
  const k = unitKey(text ?? "");
  if (!k) return null;
  const exact = UNIT_CODES.find((u) => [u.value, u.label, u.short].some((x) => unitKey(x) === k));
  if (exact) return { value: exact.value, label: exact.label };
  if (k.length < 3) return null;
  const prefix = UNIT_CODES.find((u) => {
    const l = unitKey(u.label);
    return l.startsWith(k) || k.startsWith(l);
  });
  return prefix ? { value: prefix.value, label: prefix.label } : null;
}

/**
 * Meldung am Einheiten-Feld einer Position (U4, Befund E): Die E-Rechnung
 * (XRechnung, BT-130) verlangt einen Code nach UN/ECE Rec 20; Freitext wie
 * „Pauschal“ aus älteren Paketen lehnt die KoSIT-Prüfung beim Ausstellen ab.
 * Leer bei einem Code aus `UNIT_CODES`. `fromPackage`: Position ist noch an ein
 * Paket gebunden — die Wahl löst sie vom Paket (sauberer Bruch, Hardline Pakete).
 * U7 (R1-03): Leer auch bei einem formal gültigen Code außerhalb der Liste (z. B.
 * „H87“ aus Lauf F oder einer importierten E-Rechnung) — er sperrt das Speichern
 * nicht; die Auswahl zeigt ihn als „H87 (Code, nicht in der Liste)“.
 */
export function unitCodeError(code: string | null | undefined, fromPackage = false): string {
  if (isKnownUnit(code) || isFormalUnitCode(code)) return "";
  const tail = fromPackage ? " Das löst die Position vom Paket." : "";
  if (!code || !code.trim()) return `Bitte eine Einheit aus der Liste wählen.${tail}`;
  const s = suggestUnit(code);
  return (
    `„${code}“ ist keine Einheit für die E-Rechnung — bitte aus der Liste wählen` +
    (s ? `, z. B. ${s.label}.` : ".") +
    tail
  );
}

// ---- Re-Auth: Name der Aktion (U4, V1-09; ADR 0041 Nr. 163) -----------------

/**
 * Verständlicher Name je Aktion, die eine frische Bestätigung verlangt (Re-Auth).
 * Schlüssel: Command-Name (`/api/cmd/<name>`) bzw. Auth-Pfad. Die Liste folgt
 * `auth_of` in `src-tauri/src/commands/registry.rs` (Test `labels.test.ts`); die
 * Wörter folgen den Knöpfen und Bereichen der Oberfläche.
 */
const REAUTH_ACTIONS: Record<string, string> = {
  seller_profile_upsert: "Bankverbindung ändern",
  mail_send_invoice: "Rechnung an eine andere Adresse senden",
  mail_send_quote: "Angebot an eine andere Adresse senden",
  backup_target_add_directory: "Backup-Ziel (Verzeichnis) hinzufügen",
  backup_target_add_sftp: "Backup-Ziel (SFTP) hinzufügen",
  backup_target_remove: "Backup-Ziel entfernen",
  backup_test_sftp: "SFTP-Verbindung testen",
  backup_test_sftp_saved: "SFTP-Verbindung testen",
  backup_restore_apply: "Backup wiederherstellen",
  backup_download: "Backup herunterladen",
  mail_account_create: "Postfach anlegen",
  mail_account_update: "Postfach ändern",
  mail_account_delete: "Postfach löschen",
  mail_account_test_connection: "Postfach-Verbindung testen",
  mail_account_test_saved: "Postfach-Verbindung testen",
  mail_send_test: "Test-Mail senden",
  migration_export_run: "Daten exportieren",
  dsgvo_export: "DSGVO-Auskunft erstellen",
  contacts_anonymize: "Kontakt anonymisieren",
  euer_export_pdf: "EÜR als PDF herunterladen",
  euer_export_elster: "ELSTER-Ausfüllhilfe (CSV) herunterladen",
  euer_export_detail_zip: "Einzelaufstellung (ZIP) herunterladen",
  euer_export_datev: "DATEV-Buchungsstapel herunterladen",
  euer_export_stb_zip: "Steuerberater-Paket herunterladen",
  payment_accounts_create: "Konto anlegen",
  payment_accounts_update: "Konto ändern",
  payment_accounts_set_active: "Konto aktivieren oder deaktivieren",
  fiscal_year_close: "Geschäftsjahr abschließen",
  scheduler_release_hold: "Automatische Abo-Läufe freigeben",
  numbering_raise: "Nummernkreis anheben",
  numbering_restored_draft_confirm: "Abgleich des Entwurfs bestätigen",
  invoices_void_draft: "Entwurf verwerfen",
  tax_status_change_add: "Steuerstatus ändern",
  tax_status_change_revoke: "Steuerstatus-Eintrag widerrufen",
  "/api/auth/sessions/others/revoke": "Andere Sitzungen abmelden",
  "/api/auth/totp/renew": "Authenticator-App neu verbinden",
  "/api/auth/recovery/renew": "Neue Einmal-Codes erzeugen",
  "/api/auth/passphrase": "Daten-Passwort ändern",
};

/** Name der Aktion zur Adresse einer Anfrage; `null`, wenn unbekannt. */
export function reauthActionLabel(url: string): string | null {
  const path = url.split("?")[0];
  const cmd = /^\/api\/cmd\/([^/]+)$/.exec(path);
  const key = cmd ? decodeURIComponent(cmd[1]) : path;
  return REAUTH_ACTIONS[key] ?? null;
}

/** Schlüssel der Tabelle (für den Abgleich mit der Registry im Test). */
export const REAUTH_ACTION_KEYS: readonly string[] = Object.keys(REAUTH_ACTIONS);
