// Gemeinsame TS-Types, die mit Rust-Structs korrespondieren.
// Block 2: Contact + SellerProfile. Wird in Block 3+ ausgebaut.

export type Cents = number;       // i64 in Rust, Number reicht bis 2^53 (~90 Billionen €).
export type Iso8601 = string;     // YYYY-MM-DD oder YYYY-MM-DDTHH:MM:SSZ
export type Uuid = string;        // UUIDv7

export type TaxCategoryCode = "S" | "Z" | "E" | "AE" | "K" | "G" | "O" | "L" | "M";

// ---- Anfahrtspauschale (Block P1) ------------------------------------------

export interface TravelSettings {
  costPerKmCents: Cents;
  roundTripDefault: boolean;
}

/** Eine berechnete Anfahrts-Position (km × Satz), bereit zum Einfügen. */
export interface TravelLine {
  description: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  netAmountCents: Cents;
  taxCategoryCode: TaxCategoryCode;
}

export type InvoiceStatus =
  | "draft"
  | "issued"
  | "sent"
  | "partially_paid"
  | "paid"
  | "canceled";

// ---- Contacts ---------------------------------------------------------------

export type ContactType = "customer" | "vendor" | "both" | "partner";

/** Korrespondiert zu `ContactRow` in Rust (alle Felder camelCase). */
export interface Contact {
  id: Uuid;
  contactType: string;          // ContactType, aber DB liefert es als String.
  name: string;
  legalForm: string | null;
  vatId: string | null;
  taxNumber: string | null;
  street: string | null;
  postalCode: string | null;
  city: string | null;
  countryCode: string;
  email: string | null;
  phone: string | null;
  iban: string | null;
  bic: string | null;
  acceptsEinvoice: number;      // 0|1 — Frontend macht Boolean draus.
  archived: number;             // 0|1
  notes: string | null;
  createdAt: Iso8601;
  updatedAt: Iso8601;
  // DSGVO Art. 17 (Block 19): NULL = aktiv; gesetzt = anonymisiert.
  anonymizedAt: Iso8601 | null;
}

/** Vorab-Prüfung der DSGVO-Anonymisierung (Block 19, `contacts_anonymize_check`). */
export interface AnonymizeCheck {
  canAnonymize: boolean;
  alreadyAnonymized: boolean;
  openInvoiceDrafts: number;
  openQuoteDrafts: number;
  lockedInvoices: number;
  lockedQuotes: number;
  blocker: string | null;
}

/** Eingabe für create/update. Reflektiert `ContactInput` in Rust. */
export interface ContactInput {
  contactType: ContactType;
  name: string;
  legalForm: string | null;
  vatId: string | null;
  taxNumber: string | null;
  street: string;
  postalCode: string;
  city: string;
  countryCode: string;
  email: string | null;
  phone: string | null;
  iban: string | null;
  bic: string | null;
  acceptsEinvoice: boolean;
  notes: string | null;
}

// ---- Seller Profile --------------------------------------------------------

export interface SellerProfile {
  id: number;                          // immer 1 (Singleton).
  name: string;
  legalForm: string | null;
  street: string;
  postalCode: string;
  city: string;
  countryCode: string;
  taxNumber: string | null;
  vatId: string | null;
  email: string;
  phone: string | null;
  iban: string | null;
  bic: string | null;
  logoFilename: string | null;
  isKleinunternehmer: number;          // 0|1, Status heute aus der Historie (ADR 0042); Belege: Status am Belegdatum
  waivedParagraph19Since: string | null;
  defaultPdfTemplate: string;
  defaultCurrency: string;
  updatedAt: Iso8601;
}

export interface SellerProfileInput {
  name: string;
  legalForm: string | null;
  street: string;
  postalCode: string;
  city: string;
  countryCode: string;
  taxNumber: string | null;
  vatId: string | null;
  email: string;
  phone: string | null;
  iban: string | null;
  bic: string | null;
  logoFilename: string | null;
  defaultPdfTemplate: string | null;
  defaultCurrency: string | null;
  // Kein Steuerstatus mehr (ADR 0042 Nr. 1): Wechsel nur über
  // `taxStatusChangeAdd`; ein abweichender `isKleinunternehmer` würde abgelehnt.
}

/** §19-Kompatibilität einer PDF-Vorlage (Block 17a). */
export interface PdfTemplateKlauselStatus {
  hasMarker: boolean;
  usesDataField: boolean;
  isKleinCompatible: boolean;
}

/** Wählbare PDF-Vorlage: eingebettete Built-in oder eigene `inputs/`-Datei. */
export interface PdfTemplateMeta {
  name: string;
  /** Leer (`""`) bei eingebetteten Built-ins. */
  path: string;
  klauselStatus: PdfTemplateKlauselStatus;
  builtin: boolean;
  /** Stellt die Vorlage Stornos dar (nutzt `storno_of`; Lauf 2026.11 U4). */
  supportsStorno: boolean;
  /** Ohne Storno-Darstellung: eingebaute Vorlage, mit der Stornos entstehen
   *  (z. B. "klassisch"); sonst null. */
  stornoFallback: string | null;
  /** Zeigt die Vorlage einen Leistungszeitraum (`service_period_start`/`_end`,
   *  ADR 0042 Nr. 10)? */
  supportsServicePeriod: boolean;
  /** Ohne Leistungszeitraum: eingebaute Vorlage, mit der Rechnungen mit
   *  Zeitraum entstehen; sonst null. */
  servicePeriodFallback: string | null;
}

export interface Paragraph19Info {
  hinweisText: string;
  /** Bei Regelbesteuerung heute: das früheste Rückkehr-Datum (YYYY-MM-DD) aus
   *  der Statushistorie (nach einem Verzicht das Bindungsende, ADR 0042). */
  returnDateAfterWaiver: string | null;
}

// ---- Invoices (Block 3b) ---------------------------------------------------

export type InvoiceDirection = "issued" | "received";
export type ValidationStatusDto = "passed" | "warning" | "failed";

/** Korrespondiert zu `InvoiceRow` in Rust. */
export interface Invoice {
  id: Uuid;
  invoiceNumber: string;
  fiscalYear: number;
  direction: string;
  invoiceDate: Iso8601;
  deliveryDate: Iso8601 | null;
  dueDate: Iso8601 | null;
  contactId: Uuid;
  sellerName: string;
  sellerStreet: string;
  sellerPostalCode: string;
  sellerCity: string;
  sellerTaxNumber: string | null;
  sellerVatId: string | null;
  netAmountCents: Cents;
  taxAmountCents: Cents;
  grossAmountCents: Cents;
  currencyCode: string;
  isKleinunternehmer: number;
  pdfTemplate: string;
  status: InvoiceStatus;
  sentAt: Iso8601 | null;
  /** Cache des Zahlungsbuchs: Summe aller Buchungen (ADR 0042 Nr. 7). */
  paidAmountCents: Cents;
  /** Cache: jüngstes Zahldatum der nicht stornierten Zahlungen. */
  paidAt: Iso8601 | null;
  /** Eingefroren seit Migration 0034 (Altbestand); Anzeige über `InvoiceDetail.payments`. */
  paymentHistoryJson: string | null;
  canceledAt: Iso8601 | null;
  canceledByStornoId: Uuid | null;
  isStornoFor: Uuid | null;
  cancelReason: string | null;
  validationStatus: ValidationStatusDto | null;
  validationReport: string | null;
  validatedAt: Iso8601 | null;
  pdfArchiveId: Uuid | null;
  xmlArchiveId: Uuid | null;
  lockedAt: Iso8601 | null;
  notes: string | null;
  createdAt: Iso8601;
  updatedAt: Iso8601;
  // Buyer-Snapshot (Migration 0004) — Empfänger-Stand zur Rechnungszeit.
  buyerName: string | null;
  buyerStreet: string | null;
  buyerPostalCode: string | null;
  buyerCity: string | null;
  buyerCountryCode: string | null;
  buyerVatId: string | null;
  buyerEmail: string | null;
  /** Bezahlt-/Zahlungshinweis (reiner PDF-Text, Migration 0018). */
  paymentNote: string | null;
  // Ursprungsangebot bei Konvertierung (Block 7, Migration 0005).
  derivedFromQuoteId: Uuid | null;
  // Zahlungs-Eingangs-Konto (Migration 0007, Block 9). Additiv.
  paidToAccountId: Uuid | null;
  /** Verworfener Entwurf (Migration 0032): Zeitpunkt (UTC) und Grund; die Nummer bleibt verbraucht. */
  voidedAt: Iso8601 | null;
  voidReason: string | null;
  /** Leistungszeitraum (BG-14, JJJJ-MM-TT) statt Leistungsdatum (Migration 0035, ADR 0042 Nr. 10). */
  servicePeriodStart: Iso8601 | null;
  servicePeriodEnd: Iso8601 | null;
  /** Leitweg-ID bzw. Käuferreferenz (BT-10), am Entwurf gespeichert; `null` = „N/A“ (ADR 0042 Nr. 12). */
  buyerReference: string | null;
}

export interface InvoiceItem {
  id: Uuid;
  invoiceId: Uuid;
  position: number;
  description: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  netAmountCents: Cents;
  taxRatePercent: number;
  taxCategoryCode: TaxCategoryCode;
  /** P3, Migration 0020/0021 — null = Alt-Verhalten. */
  descriptionTitle: string | null;
  descriptionMarkup: string | null;
  sourcePackageId: string | null;
  sourcePackageRevision: number | null;
}

export interface InvoiceDetail {
  invoice: Invoice;
  items: InvoiceItem[];
  buyer: Contact | null;
  /** Zahlungsbuch mit Gegenbuchungen (Migration 0034, ADR 0042 Nr. 7–8). */
  payments: InvoicePayment[];
  /** Restore-Vormerkung des offenen Entwurfs (ADR 0042 Nr. 16); sonst `null`. */
  restoreConflict: RestoreConflictState | null;
}

/**
 * Stand eines wiederhergestellten Entwurfs mit vor dem Restore festgeschriebener
 * Nummer: `flagged` = Fertigstellen erst nach Bestätigung, `confirmed` = das
 * nächste Speichern merkt ihn wieder vor (ADR 0042 Nr. 16, Warteliste 8).
 */
export type RestoreConflictState = "flagged" | "confirmed";

/** Eine Buchung des Zahlungsbuchs (`InvoicePaymentView` in Rust). */
export interface InvoicePayment {
  id: Uuid;
  invoiceId: Uuid;
  seq: number;
  /** `payment` (Betrag > 0) oder `reversal` (Gegenbuchung, Betrag < 0). */
  kind: "payment" | "reversal";
  amountCents: Cents;
  paidDate: string;
  accountId: Uuid | null;
  note: string | null;
  /** Gegenbuchung: die stornierte Zahlung. */
  reversesPaymentId: Uuid | null;
  /** Gegenbuchung: Grund. */
  reason: string | null;
  createdAt: string;
  /** Zahlung: ihre Gegenbuchung, falls storniert. */
  reversedById: Uuid | null;
  /** „Zahlung stornieren“ möglich. */
  reversible: boolean;
  /** Warum nicht (Zahlung, die sich nicht stornieren lässt). */
  notReversibleReason: string | null;
}

export interface InvoiceListItem {
  id: Uuid;
  invoiceNumber: string;
  fiscalYear: number;
  invoiceDate: Iso8601;
  dueDate: Iso8601 | null;
  contactId: Uuid;
  contactName: string;
  grossAmountCents: Cents;
  paidAmountCents: Cents;
  currencyCode: string;
  status: InvoiceStatus;
  isStornoFor: Uuid | null;
  /** Verworfener Entwurf (Migration 0032). */
  voidedAt: Iso8601 | null;
}

export interface InvoiceListFilter {
  fiscalYear?: number;
  status?: InvoiceStatus;
  contactId?: Uuid;
  includeCanceled?: boolean;
}

export interface InvoiceItemInput {
  position: number;
  description: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  taxRatePercent: number;
  taxCategoryCode: TaxCategoryCode;
  /** P3: Positions-Titel (PDF-Zeile, nicht im XML). null = einfache Position. */
  descriptionTitle?: string | null;
  /** P3: optionales Markup → treibt nur den PDF-Block (volle Breite). null = schmale Zelle. */
  descriptionMarkup?: string | null;
  /** P3: Soft-Zeiger aufs Quell-Paket (Provenienz); null = Custom/„Paket angepasst". */
  sourcePackageId?: string | null;
  /** P3: Snapshot der Paket-Revisionsnummer beim Einfügen. */
  sourcePackageRevision?: number | null;
}

export interface InvoiceInputDto {
  direction: InvoiceDirection;
  invoiceDate: string;     // YYYY-MM-DD
  deliveryDate: string | null;
  /** Leistungszeitraum statt Leistungsdatum: beide Grenzen oder keine, nie zugleich mit `deliveryDate` (ADR 0042 Nr. 10). */
  servicePeriodStart?: string | null;
  servicePeriodEnd?: string | null;
  /** Leitweg-ID bzw. Käuferreferenz (BT-10); hat Vorrang vor `CreateDraftArgs.buyerReference` (ADR 0042 Nr. 12). */
  buyerReference?: string | null;
  dueDate: string | null;
  currencyCode: string;
  items: InvoiceItemInput[];
  notes: string | null;
  /** Optionaler Bezahlt-/Zahlungshinweis (reiner PDF-Text). */
  paymentNote: string | null;
  pdfTemplate: string;
  isStornoFor: Uuid | null;
  cancelReason: string | null;
}

export interface CreateDraftArgs {
  contactId: Uuid;
  /** Ältere Form der Leitweg-ID; `input.buyerReference` hat Vorrang (ADR 0042 Nr. 12). */
  buyerReference?: string | null;
  input: InvoiceInputDto;
}

export interface LockResponse {
  invoice: Invoice;
  pdfArchiveId: Uuid;
  xmlArchiveId: Uuid;
  validationStatus: ValidationStatusDto;
  validationFindingsCount: number;
  validationWarningsCount: number;
}

export interface ValidationIssueDto {
  code: string;
  message: string;
}

export interface CancelArgs {
  invoiceId: Uuid;
  reason: string;
  stornoDate: string | null;
}

export interface CancelResponse {
  original: Invoice;
  storno: LockResponse;
}

export interface RecordPaymentArgs {
  invoiceId: Uuid;
  amountCents: Cents;
  paidDate: string;
  note: string | null;
  /** Zahlungskonto des Eingangs (DATEV-Geldkonto, R2-011). */
  accountId?: Uuid | null;
}

/** Argumente von `invoices_reverse_payment` (ADR 0042 Nr. 8). */
export type ReversePaymentArgs = {
  invoiceId: Uuid;
  paymentId: Uuid;
  /** Grund, mindestens 10 Zeichen. */
  reason: string;
};

// ---- Mail (Block 5) --------------------------------------------------------

/** Korrespondiert zu `MailAccountRow` in Rust. Geheimnisse stehen NICHT hier. */
export interface MailAccount {
  id: Uuid;
  label: string;
  authType: string;          // "smtp_password"; "oauth_microsoft" nur bei übernommenen Desktop-Konten
  smtpHost: string | null;
  smtpPort: number | null;
  smtpUser: string | null;
  smtpUseTls: number;        // 0|1
  keychainServiceId: string | null;
  fromEmail: string;
  fromName: string;
  isDefault: number;         // 0|1
  lastUsedAt: Iso8601 | null;
  createdAt: Iso8601;
  // OAuth (Migration 0014) — Bestandsdaten der Desktop-Fassung, nur gelesen.
  oauthTenantId: string | null;
  oauthClientId: string | null;
  oauthAccountEmail: string | null;
  oauthScopes: string | null;
  oauthTokenExpiresAt: Iso8601 | null;
}

export interface MailAccountInput {
  label: string;
  authType: string;          // im Server nur "smtp_password" (E9)
  smtpHost: string | null;
  smtpPort: number | null;
  smtpUser: string | null;
  smtpUseTls: boolean;
  fromEmail: string;
  fromName: string;
  isDefault: boolean;
}

/** Ein Eintrag im E-Mail-Versandprotokoll (Block 16b). Korrespondiert zu `EmailLogRow`. */
export interface EmailLogEntry {
  id: Uuid;
  createdAt: Iso8601;
  accountId: string | null;
  accountLabel: string | null;
  channel: string;          // "smtp" | "graph"
  relatedKind: string;      // "invoice" | "quote" | "test"
  relatedId: string | null;
  relatedNumber: string | null;
  fromEmail: string;
  toEmail: string;
  subject: string;
  attachmentCount: number;
  status: string;           // "success" | "failed"
  providerCode: string | null;
  providerMessage: string | null;
  requestId: string | null;
  error: string | null;
}

/** Filter für die Protokoll-Suche (Block 16b). Alle Felder optional. */
export interface EmailLogFilter {
  search?: string | null;
  dateFrom?: string | null;   // "YYYY-MM-DD", lokale Zeit
  dateTo?: string | null;     // "YYYY-MM-DD", lokale Zeit
  status?: string | null;     // "success" | "failed"
  kind?: string | null;       // "invoice" | "quote" | "test"
  channel?: string | null;    // "smtp" | "graph"
  limit?: number | null;
}

export interface TestConnectionArgs {
  smtpHost: string;
  smtpPort: number;
  smtpUseTls: boolean;
  smtpUser: string | null;
  /** Klartext nur für den Test — wird nicht gespeichert. */
  password: string | null;
}

export interface SendInvoiceArgs {
  accountId: Uuid;
  invoiceId: Uuid;
  to: string | null;
  subject: string | null;
  body: string | null;
}

export interface SendResult {
  invoiceId: Uuid;
  to: string;
  subject: string;
  attachmentCount: number;
  /** Versandprotokoll unvollständig (R4-S2-EMAILLOG). */
  warning?: string | null;
}

export interface RenderedMail {
  subject: string;
  body: string;
}

// ---- Quotes / Angebote (Block 6) -------------------------------------------

export type QuoteStatus =
  | "draft"
  | "sent"
  | "accepted"
  | "rejected"
  | "canceled"
  | "converted";

/** Korrespondiert zu `QuoteRow` in Rust. */
export interface Quote {
  id: Uuid;
  quoteNumber: string;
  fiscalYear: number;
  quoteDate: Iso8601;
  validUntil: Iso8601;
  contactId: Uuid;
  sellerName: string;
  sellerStreet: string;
  sellerPostalCode: string;
  sellerCity: string;
  sellerTaxNumber: string | null;
  sellerVatId: string | null;
  netAmountCents: Cents;
  taxAmountCents: Cents;
  grossAmountCents: Cents;
  currencyCode: string;
  isKleinunternehmer: number;
  pdfTemplate: string;
  status: QuoteStatus;
  sentAt: Iso8601 | null;
  acceptedAt: Iso8601 | null;
  rejectedAt: Iso8601 | null;
  canceledAt: Iso8601 | null;
  canceledReason: string | null;
  convertedAt: Iso8601 | null;
  convertedInvoiceId: Uuid | null;
  pdfArchiveId: Uuid | null;
  lockedAt: Iso8601 | null;
  notes: string | null;
  createdAt: Iso8601;
  updatedAt: Iso8601;
  // Buyer-Snapshot (Migration 0023, Block 19) — Empfänger-Stand zur Angebotszeit.
  buyerName: string | null;
  buyerStreet: string | null;
  buyerPostalCode: string | null;
  buyerCity: string | null;
  buyerCountryCode: string | null;
  buyerVatId: string | null;
  buyerEmail: string | null;
}

export interface QuoteItem {
  id: Uuid;
  quoteId: Uuid;
  position: number;
  description: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  netAmountCents: Cents;
  taxRatePercent: number;
  taxCategoryCode: TaxCategoryCode;
  /** P3, Migration 0020/0021 — null = Alt-Verhalten. */
  descriptionTitle: string | null;
  descriptionMarkup: string | null;
  sourcePackageId: string | null;
  sourcePackageRevision: number | null;
}

/** `attachments` JOIN `archive_entries` (Anzeige-Sicht). */
export interface AttachmentView {
  id: Uuid;
  parentType: string;
  parentId: Uuid;
  archiveEntryId: Uuid;
  label: string | null;
  sortOrder: number;
  createdAt: Iso8601;
  fileName: string;
  fileSizeBytes: number;
  mimeType: string;
}

export interface QuoteDetail {
  quote: Quote;
  items: QuoteItem[];
  buyer: Contact | null;
  attachments: AttachmentView[];
  /** Restore-Vormerkung des offenen Entwurfs (ADR 0042 Nr. 16); sonst `null`. */
  restoreConflict: RestoreConflictState | null;
}

export interface QuoteListItem {
  id: Uuid;
  quoteNumber: string;
  fiscalYear: number;
  quoteDate: Iso8601;
  validUntil: Iso8601;
  contactId: Uuid;
  contactName: string;
  grossAmountCents: Cents;
  currencyCode: string;
  status: QuoteStatus;
  /** Festgeschrieben am (DB-Zeitstempel UTC); null = nie festgeschrieben.
   *  `status === "canceled"` ohne `lockedAt` = verworfener Entwurf, mit
   *  `lockedAt` = zurückgezogenes Angebot (Lauf 2026.11 U4). */
  lockedAt: Iso8601 | null;
}

export interface QuoteListFilter {
  fiscalYear?: number;
  status?: QuoteStatus;
  contactId?: Uuid;
  includeInactive?: boolean;
}

export interface QuoteItemInput {
  position: number;
  description: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  taxRatePercent: number;
  taxCategoryCode: TaxCategoryCode;
  /** P3: Positions-Titel (PDF-Zeile, nicht im XML). null = einfache Position. */
  descriptionTitle?: string | null;
  /** P3: optionales Markup → treibt nur den PDF-Block (volle Breite). null = schmale Zelle. */
  descriptionMarkup?: string | null;
  /** P3: Soft-Zeiger aufs Quell-Paket (Provenienz); null = Custom/„Paket angepasst". */
  sourcePackageId?: string | null;
  /** P3: Snapshot der Paket-Revisionsnummer beim Einfügen. */
  sourcePackageRevision?: number | null;
}

export interface QuoteInputDto {
  quoteDate: string;     // YYYY-MM-DD
  validUntil: string;    // YYYY-MM-DD
  currencyCode: string;
  items: QuoteItemInput[];
  notes: string | null;
  pdfTemplate: string;
}

export interface QuoteCreateDraftArgs {
  contactId: Uuid;
  input: QuoteInputDto;
}

export interface QuoteAcceptArgs {
  quoteId: Uuid;
  /** Fachliches Annahmedatum (YYYY-MM-DD). null → heute (Backend). */
  acceptedDate: string | null;
  /** Upload (Art `vertrag`) des unterschriebenen Vertrags. null → ohne Doc. */
  signedContractUploadId: string | null;
  attachmentLabel: string | null;
}

export interface QuoteRejectArgs {
  quoteId: Uuid;
  reason: string | null;
}

export interface QuoteCancelArgs {
  quoteId: Uuid;
  reason: string;
}

/** Args für quotes_convert_to_invoice (Block 7). */
export interface QuoteConvertArgs {
  quoteId: Uuid;
  /** Rechnungsdatum (YYYY-MM-DD). Bestimmt auch das Geschäftsjahr. */
  invoiceDate: string;
  deliveryDate: string | null;
  /** Leistungszeitraum statt Leistungsdatum (ADR 0042 Nr. 10), beide oder keiner. */
  servicePeriodStart?: string | null;
  servicePeriodEnd?: string | null;
  /** Leitweg-ID bzw. Käuferreferenz (BT-10) der Rechnung (ADR 0042 Nr. 12). */
  buyerReference?: string | null;
  dueDate: string | null;
  /** null → Template des Angebots übernehmen. */
  pdfTemplate: string | null;
  /** null → Notiz des Angebots übernehmen. */
  notes: string | null;
  /** Optionaler Bezahlt-/Zahlungshinweis (reiner PDF-Text). */
  paymentNote: string | null;
  /** null → Positionen 1:1 aus dem Angebot übernehmen. */
  items: QuoteItemInput[] | null;
}

// ---- Pakete (Block P2, Migration 0019) -------------------------------------

export interface PackageCategory {
  id: Uuid;
  name: string;
  sortOrder: number;
  createdAt: Iso8601;
  updatedAt: Iso8601;
}

export interface Package {
  id: Uuid;
  categoryId: Uuid | null;
  name: string;
  status: "active" | "archived";
  currentRevision: number | null;
  sortOrder: number;
  createdAt: Iso8601;
  updatedAt: Iso8601;
}

export interface PackageRevision {
  id: Uuid;
  packageId: Uuid;
  revision: number;
  title: string;
  bodyMarkup: string;
  defaultUnitPriceCents: number;
  unitCode: string;
  taxCategoryCode: string;
  note: string | null;
  createdAt: Iso8601;
}

export interface PackageRevisionInput {
  title: string;
  bodyMarkup: string;
  defaultUnitPriceCents: number;
  unitCode: string;
  taxCategoryCode: string;
  note: string | null;
}

/** Aus einem Paket materialisierte Beleg-Position (Block P3, package_materialize_item). */
export interface MaterializedPackageItem {
  description: string;
  descriptionTitle: string;
  descriptionMarkup: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  taxRatePercent: number;
  taxCategoryCode: TaxCategoryCode;
  sourcePackageId: Uuid;
  sourcePackageRevision: number;
  packageName: string;
}

// ---- Paket-Katalog-Broschüre (Block P4) ------------------------------------

/** Versand-Argumente für die Paket-Broschüre (package_catalog_send). */
export interface SendPackageCatalogArgs {
  accountId: Uuid;
  contactId: Uuid;
  packageIds: Uuid[];
  subject: string | null;
  body: string | null;
}

export interface SendPackageCatalogResult {
  to: string;
  subject: string;
  packageCount: number;
  attachmentCount: number;
  warning?: string | null;
}

// ---- Rechtsdokumente / Legal Documents (Block 8) ---------------------------

export type LegalDocType = "agb" | "privacy";

/** Korrespondiert zu `LegalDocumentRow` in Rust (JOIN archive_entries). */
export interface LegalDocument {
  id: Uuid;
  docType: string; // LegalDocType
  version: number;
  title: string;
  archiveEntryId: Uuid;
  isActive: number; // 0|1
  createdAt: Iso8601;
  activatedAt: Iso8601 | null;
  deactivatedAt: Iso8601 | null;
  fileName: string;
  fileSizeBytes: number;
  mimeType: string;
}

/** Fest mit einem Angebot verknüpfte Legal-Version (`QuoteLegalDocumentView`). */
export interface QuoteLegalBinding {
  id: Uuid;
  quoteId: Uuid;
  legalDocumentId: Uuid;
  docType: string;
  version: number;
  boundAt: Iso8601;
  title: string;
  archiveEntryId: Uuid;
  fileName: string;
}

export interface SendQuoteArgs {
  accountId: Uuid;
  quoteId: Uuid;
  to: string | null;
  subject: string | null;
  body: string | null;
}

export interface SendQuoteResult {
  quoteId: Uuid;
  to: string;
  subject: string;
  attachmentCount: number;
  warning?: string | null;
}

// ---- Payment Accounts (Block 9) --------------------------------------------

export type PaymentAccountType = "bank" | "cash" | "paypal" | "stripe" | "other";

/** Korrespondiert zu `PaymentAccountRow` in Rust (DB-Spalte `type`). */
export interface PaymentAccount {
  id: Uuid;
  label: string;
  accountType: string; // PaymentAccountType
  iban: string | null;
  bic: string | null;
  isDefault: number; // 0|1
  active: number; // 0|1
  showOnInvoice: number; // 0|1 — auf Beleg/Impressum anzeigen
  details: string | null; // Freitext für Nicht-Bank-Konten (z. B. PayPal-Link)
  createdAt: Iso8601;
}

export interface PaymentAccountInput {
  label: string;
  accountType: PaymentAccountType;
  iban: string | null;
  bic: string | null;
  isDefault: boolean;
  showOnInvoice: boolean;
  details: string | null;
}

// ---- Expenses / Kosten (Block 9) -------------------------------------------

export type ExpenseCategory =
  | "office"
  | "software"
  | "hardware"
  | "travel"
  | "services"
  | "goods"
  | "communications"
  | "vehicle"
  | "rent"
  | "insurance"
  | "training"
  | "fees"
  | "marketing"
  | "other";

export type ExpenseStatus = "recorded" | "canceled";

/** Eingabe (korrespondiert zu `ExpenseInput` in Rust). */
export interface ExpenseInputDto {
  expenseDate: string; // YYYY-MM-DD
  paidDate: string | null;
  paidFromAccountId: Uuid | null;
  vendorContactId: Uuid | null;
  vendorName: string;
  vendorInvoiceNumber: string | null;
  category: ExpenseCategory;
  description: string;
  netAmountCents: Cents;
  taxAmountCents: Cents;
  grossAmountCents: Cents;
  currencyCode: string;
  reverseCharge13b: boolean;
  notes: string | null;
}

/** Korrespondiert zu `ExpenseRow` in Rust. */
export interface Expense {
  id: Uuid;
  expenseNumber: string;
  fiscalYear: number;
  expenseDate: Iso8601;
  paidDate: Iso8601 | null;
  paidFromAccountId: Uuid | null;
  vendorContactId: Uuid | null;
  vendorNameSnapshot: string;
  vendorInvoiceNumber: string | null;
  category: string;
  description: string;
  netAmountCents: Cents;
  taxAmountCents: Cents;
  grossAmountCents: Cents;
  currencyCode: string;
  reverseCharge13b: number; // 0|1
  receiptArchiveId: Uuid | null;
  einvoiceValidationStatus: string | null;
  einvoiceValidationReport: string | null;
  recurringSubscriptionId: Uuid | null;
  capitalizedAsAssetId: Uuid | null;
  status: ExpenseStatus;
  canceledAt: Iso8601 | null;
  canceledReason: string | null;
  lockedAt: Iso8601 | null;
  notes: string | null;
  createdAt: Iso8601;
}

export interface ExpenseListItem {
  id: Uuid;
  expenseNumber: string;
  fiscalYear: number;
  expenseDate: Iso8601;
  paidDate: Iso8601 | null;
  vendorNameSnapshot: string;
  vendorInvoiceNumber: string | null;
  category: string;
  description: string;
  grossAmountCents: Cents;
  currencyCode: string;
  status: ExpenseStatus;
  reverseCharge13b: number;
  receiptArchiveId: Uuid | null;
  recurringSubscriptionId: Uuid | null;
}

export interface ExpenseDetail {
  expense: Expense;
  vendor: Contact | null;
  attachments: AttachmentView[];
  /** PV1-A5: erkanntes E-Rechnungs-Format des primären Belegs (`zugferd` | `xrechnung-cii`),
   * oder `null` bei manuell erfassten Belegen. Steuert die Sichtbarkeit des
   * „Roh-XML anzeigen"-Buttons. Die feine CII/UBL-Unterscheidung passiert erst
   * im Viewer-Command. */
  sourceFormat: string | null;
}

/** PV1-A5: Anzeige-Payload für den Roh-XML-Viewer. */
export interface XmlViewerPayload {
  /** Roh-XML — Frontend rendert in `<pre>`. */
  xml: string;
  /** `zugferd` | `xrechnung-cii` | `xrechnung-ubl`. */
  sourceFormat: string;
  /** SHA-256 des archivierten Originals (Hex). */
  sha256Hex: string;
  /** Byte-Größe des archivierten Originals. */
  byteSize: number;
}

export interface ExpenseListFilter {
  fiscalYear?: number;
  status?: ExpenseStatus;
  category?: ExpenseCategory;
  includeCanceled?: boolean;
}

export interface ExpenseCreateArgs {
  input: ExpenseInputDto;
  /** Upload (Art `beleg`, Plan W7). null → ohne Beleg. */
  receiptUploadId: string | null;
}

export interface ExpenseCancelArgs {
  expenseId: Uuid;
  reason: string;
}

export interface ExpenseSetPaymentArgs {
  expenseId: Uuid;
  /** null → wieder auf „offen" setzen. */
  paidDate: string | null;
  paidFromAccountId: Uuid | null;
}

// ---- E-Rechnung-Empfang / Receive (Block 11) -------------------------------

export type EinvoiceSyntax = "cii" | "ubl";

/** Ein Befund aus dem KoSIT-Validator (verdichtet). */
export interface EinvoiceValidationFinding {
  severity: string; // "error" | "warning" | "info"
  ruleId: string | null;
  message: string;
  location: string | null;
}

/** Korrespondiert zu `ValidationSummary` in Rust (ohne Roh-XML). */
export interface ValidationSummary {
  status: ValidationStatusDto;
  errorCount: number;
  warningCount: number;
  findings: EinvoiceValidationFinding[];
}

/** Korrespondiert zu `ParsedEInvoice` in Rust. */
export interface ParsedEInvoice {
  syntax: EinvoiceSyntax | null;
  invoiceNumber: string | null;
  typeCode: string | null;
  invoiceDate: Iso8601 | null;
  dueDate: Iso8601 | null;
  /** BG-14 — Rechnungszeitraum auf Kopfebene (ADR 0042 Nr. 10). */
  servicePeriodStart?: Iso8601 | null;
  servicePeriodEnd?: Iso8601 | null;
  currencyCode: string | null;
  sellerName: string | null;
  sellerVatId: string | null;
  sellerTaxNumber: string | null;
  buyerName: string | null;
  buyerVatId: string | null;
  netAmountCents: Cents | null;
  taxAmountCents: Cents | null;
  grossAmountCents: Cents | null;
  lineDescriptions: string[];
  reverseCharge: boolean;
  /** Steuerkategorien laut Rechnung (UNTDID 5305, z. B. "S", "E", "AE", "K"),
   *  je einmal in der Reihenfolge des Auftretens (Lauf 2026.11 U4). */
  taxCategories: string[];
  /** Mindestens eine Kategorie K (innergemeinschaftlich, §1a UStG) — nur ein
   *  Kennzeichen für den Hinweis, Beträge bleiben unberührt (U4). */
  intraCommunity: boolean;
  /** CII-Profil-URN (BT-24); null bei UBL. */
  guidelineId: string | null;
}

/** Ergebnis von expenses_parse_einvoice (Schritt 1, nichts persistiert). */
export interface EInvoiceParseResult {
  input: ExpenseInputDto;
  parsed: ParsedEInvoice;
  validation: ValidationSummary | null;
  sourceFormat: string; // "zugferd" | "xrechnung-cii" | "xrechnung-ubl"
  isPdf: boolean;
}

/** Args für expenses_create_from_einvoice (Schritt 2). */
export interface EInvoiceCreateArgs {
  input: ExpenseInputDto;
  /** Upload des Originals aus Schritt 1 (XML oder ZUGFeRD-PDF) — wird
   *  unverändert archiviert. */
  uploadId: string;
  sourceFormat: string;
  validation: ValidationSummary | null;
}

// ---- Private Movements / Privatbewegungen (Block 9) ------------------------

export type MovementType = "entnahme" | "einlage";

export interface PrivateMovementInputDto {
  movementDate: string; // YYYY-MM-DD
  movementType: MovementType;
  amountCents: Cents;
  accountId: Uuid | null;
  description: string;
  notes: string | null;
}

export interface PrivateMovement {
  id: Uuid;
  movementNumber: string;
  fiscalYear: number;
  movementDate: Iso8601;
  movementType: MovementType;
  amountCents: Cents;
  accountId: Uuid | null;
  description: string;
  receiptArchiveId: Uuid | null;
  lockedAt: Iso8601 | null;
  notes: string | null;
  createdAt: Iso8601;
}

export interface PrivateMovementListItem {
  id: Uuid;
  movementNumber: string;
  fiscalYear: number;
  movementDate: Iso8601;
  movementType: MovementType;
  amountCents: Cents;
  accountId: Uuid | null;
  accountLabel: string | null;
  description: string;
}

export interface PrivateMovementListFilter {
  fiscalYear?: number;
  movementType?: MovementType;
}

export interface PrivateMovementCreateArgs {
  input: PrivateMovementInputDto;
  /** Upload (Art `beleg`, Plan W7). null → ohne Beleg. */
  receiptUploadId: string | null;
}

// ---- Generischer Anhang-Upload (Block 9) -----------------------------------

export interface AddAttachmentArgs {
  parentType: string;
  parentId: Uuid;
  /** Upload der Art `anhang` (Plan W7). */
  uploadId: string;
  label: string | null;
}

// ---- Recurring Subscriptions / Abos (Block 10) -----------------------------

export type Frequency = "monthly" | "quarterly" | "semiannually" | "annually";

/** Eingabe (korrespondiert zu `RecurringInput` in Rust). */
export interface RecurringInputDto {
  label: string;
  vendorContactId: Uuid | null;
  frequency: Frequency;
  dayOfPeriod: number;
  nextDueDate: string; // YYYY-MM-DD
  expectedAmountCents: Cents;
  category: ExpenseCategory;
  descriptionTemplate: string;
  autoCreateExpense: boolean;
  reverseCharge13bDefault: boolean;
}

/** Korrespondiert zu `RecurringSubscriptionRow` in Rust. */
export interface RecurringSubscription {
  id: Uuid;
  label: string;
  vendorContactId: Uuid | null;
  frequency: string; // Frequency
  dayOfPeriod: number;
  nextDueDate: Iso8601;
  expectedAmountCents: Cents;
  category: string;
  descriptionTemplate: string;
  autoCreateExpense: number; // 0|1
  reverseCharge13bDefault: number; // 0|1
  active: number; // 0|1
  lastExecutedAt: Iso8601 | null;
  lastExpenseId: Uuid | null;
  createdAt: Iso8601;
}

/** Korrespondiert zu `ProcessReport` (scheduler::recurring) in Rust. */
export interface RecurringProcessReport {
  skippedLocked: boolean;
  /** Grund einer Pause der automatischen Belegerzeugung (Lauf F), sonst null. */
  skippedHold?: string | null;
  processedSubscriptions: number;
  createdExpenses: number;
  failedSubscriptions?: number;
  /** Abos mit Rückstand über der Grenze — warten auf „Rückstand …“. */
  awaitingDecision?: number;
}

/** Wahl bei verpassten Terminen (Fortsetzen nach Pause, Rückstand). */
export type ResumeMode = "catchUp" | "skip";

/** Pause der automatischen Belegerzeugung (Uhrsprung, Restore; ADR 0040). */
export interface SchedulerHoldStatus {
  held: boolean;
  reason: string | null;
  since: string | null;
  message: string | null;
}

// ---- Wiederkehrende Ausgangsrechnungen / Abo-Rechnungen (Block RI) ----------

export type RecurringInvoiceMode = "draft" | "issue" | "issue_send";

/** Korrespondiert zu `RecurringInvoiceRow` in Rust. */
export interface RecurringInvoiceRow {
  id: Uuid;
  label: string;
  contactId: Uuid;
  frequency: string; // Frequency
  dayOfPeriod: number;
  nextDueDate: Iso8601;
  startDate: string | null;
  endDate: string | null;
  autoMode: string; // RecurringInvoiceMode
  paymentTermsDays: number;
  pdfTemplate: string;
  servicePeriodNote: number; // 0|1
  active: number; // 0|1
  lastExecutedAt: Iso8601 | null;
  lastInvoiceId: Uuid | null;
  notes: string | null;
  createdAt: Iso8601;
  updatedAt: Iso8601;
  /** Leitweg-ID bzw. Käuferreferenz (BT-10) für jede erzeugte Rechnung; `null` = „N/A“ (Migration 0035). */
  buyerReference: string | null;
}

/** Korrespondiert zu `RecurringInvoiceItemRow` in Rust. */
export interface RecurringInvoiceItemRow {
  id: Uuid;
  recurringInvoiceId: Uuid;
  position: number;
  description: string;
  quantity: number;
  unitCode: string;
  unitPriceCents: Cents;
  netAmountCents: Cents;
  taxRatePercent: number;
  taxCategoryCode: string;
  descriptionTitle: string | null;
  descriptionMarkup: string | null;
  sourcePackageId: string | null;
  sourcePackageRevision: number | null;
}

/** Korrespondiert zu `RecurringInvoiceDetail` in Rust. */
export interface RecurringInvoiceDetail {
  template: RecurringInvoiceRow;
  items: RecurringInvoiceItemRow[];
}

/** Eingabe (korrespondiert zu `RecurringInvoiceInput` in Rust). Positionen sind
 *  der kanonische InvoiceItemInput. */
export interface RecurringInvoiceInputDto {
  label: string;
  contactId: Uuid;
  frequency: Frequency;
  dayOfPeriod: number;
  nextDueDate: string; // YYYY-MM-DD
  startDate: string | null;
  endDate: string | null;
  autoMode: RecurringInvoiceMode;
  paymentTermsDays: number;
  pdfTemplate: string;
  /** Nur noch der Klartext in Position 1; den Abrechnungszeitraum (BG-14) trägt jede Abo-Rechnung (ADR 0042 Nr. 10). */
  servicePeriodNote: boolean;
  notes: string | null;
  /** Leitweg-ID bzw. Käuferreferenz (BT-10) für jede erzeugte Rechnung (ADR 0042 Nr. 12). */
  buyerReference?: string | null;
  items: InvoiceItemInput[];
}

/** Korrespondiert zu `ProcessReport` (scheduler::recurring_invoice) in Rust. */
export interface RecurringInvoiceProcessReport {
  skippedLocked: boolean;
  skippedHold?: string | null;
  processedTemplates: number;
  createdInvoices: number;
  failedTemplates?: number;
  awaitingDecision?: number;
  resumed?: number;
}

// ---- Anlagen / Assets + AfA (Block 12) -------------------------------------

export type DepreciationMethod = "gwg_sofort" | "linear" | "computer_special_2021";
export type DisposalType = "sale" | "scrap" | "given_away";

/** Eingabe (korrespondiert zu `AssetInput` in Rust). */
export interface AssetInputDto {
  label: string;
  acquisitionDate: string; // YYYY-MM-DD
  /** Netto-Anschaffungskosten in Cent (voll, vor Privatanteil). */
  acquisitionCostCents: Cents;
  expenseId: Uuid | null;
  vendorContactId: Uuid | null;
  depreciationMethod: DepreciationMethod;
  /** Nur bei 'linear' Pflicht; sonst null. */
  usefulLifeYears: number | null;
  afaCategory: string | null;
  /** Betrieblicher Anteil 0–100. */
  businessSharePercent: number;
  notes: string | null;
}

/** Korrespondiert zu `AssetRow` in Rust. */
export interface Asset {
  id: Uuid;
  assetNumber: string;
  label: string;
  acquisitionDate: Iso8601;
  acquisitionCostCents: Cents;
  acquisitionFiscalYear: number;
  expenseId: Uuid | null;
  vendorContactId: Uuid | null;
  depreciationMethod: string; // DepreciationMethod
  usefulLifeYears: number | null;
  afaCategory: string | null;
  businessSharePercent: number;
  bookValueCents: Cents;
  lastDepreciationYear: number | null;
  disposed: number; // 0|1
  disposalDate: Iso8601 | null;
  disposalType: string | null; // DisposalType
  disposalProceedsCents: Cents | null;
  disposalResidualBookValueCents: Cents | null;
  lockedAt: Iso8601 | null;
  notes: string | null;
  createdAt: Iso8601;
  updatedAt: Iso8601;
}

export interface AssetListItem {
  id: Uuid;
  assetNumber: string;
  label: string;
  acquisitionDate: Iso8601;
  acquisitionFiscalYear: number;
  acquisitionCostCents: Cents;
  depreciationMethod: string;
  businessSharePercent: number;
  bookValueCents: Cents;
  lastDepreciationYear: number | null;
  disposed: number; // 0|1
  disposalDate: Iso8601 | null;
  lockedAt: Iso8601 | null;
}

/** Korrespondiert zu `DepreciationEntryRow` in Rust. */
export interface DepreciationEntry {
  id: Uuid;
  assetId: Uuid;
  fiscalYear: number;
  depreciationAmountCents: Cents;
  monthsInYear: number;
  bookValueBeforeCents: Cents;
  bookValueAfterCents: Cents;
  isFullWriteoff: number; // 0|1
  computedAt: Iso8601;
  lockedAt: Iso8601 | null;
}

export interface AssetDetail {
  asset: Asset;
  vendor: Contact | null;
  depreciationEntries: DepreciationEntry[];
  sourceExpenseNumber: string | null;
}

export interface AssetListFilter {
  fiscalYear?: number;
  /** true → nur veräußerte, false → nur aktive, weglassen → alle. */
  disposed?: boolean;
  afaCategory?: string;
}

export interface AssetDisposeArgs {
  assetId: Uuid;
  disposalDate: string; // YYYY-MM-DD
  disposalType: DisposalType;
  proceedsCents: Cents;
}

/** Vorschau eines Abgangs (`commands::assets::AssetDisposalPreview`, Lauf 2026.11 U4):
 *  was `assets_dispose` zum Datum buchen würde. Gewinn bzw. Verlust =
 *  Erlös − `residualBookValueCents`. */
export interface AssetDisposalPreview {
  assetId: Uuid;
  disposalDate: string; // YYYY-MM-DD
  /** Aktueller Restbuchwert (Stand vor dem Abgang). */
  currentBookValueCents: Cents;
  /** Offene AfA ab dem Abgangsjahr, die der Abgang ersetzt. */
  replacedDepreciationCents: Cents;
  replacedYears: number[];
  /** AfA der Vorjahre ohne Buchung, die der Abgang nachholt. */
  catchUpDepreciationCents: Cents;
  catchUpYears: number[];
  /** Restbuchwert zu Beginn des Abgangsjahres. */
  bookValueBeforeCents: Cents;
  /** Anteilige AfA im Abgangsjahr bis einschließlich Abgangsmonat. */
  disposalYearDepreciationCents: Cents;
  disposalYearMonths: number;
  /** Restbuchwert zum Abgang (Ausgabe in der EÜR). */
  residualBookValueCents: Cents;
  /** AfA insgesamt von der Anschaffung bis zum Abgang. */
  depreciationToDateCents: Cents;
  /** Grund, aus dem der Abgang zu diesem Datum abgelehnt würde; null = möglich. */
  blockedReason: string | null;
}

/** Korrespondiert zu `afa_tabellen::AfaCategory` in Rust. */
export interface AfaCategory {
  code: string;
  label: string;
  usefulLifeYears: number;
  specialRule: string | null;
  appliesTo: string[];
}

/** Korrespondiert zu `afa_tabellen::AfaTabellen` in Rust. */
export interface AfaTabellen {
  version: string;
  sourceUrl: string | null;
  categories: AfaCategory[];
  gwgThresholdCents: Cents;
}

/** Korrespondiert zu `asset::MethodSuggestion` in Rust. */
export interface MethodSuggestion {
  method: DepreciationMethod;
  usefulLifeYears: number | null;
  afaCategory: string | null;
  reason: string;
}

/** Korrespondiert zu `commands::assets::AssetSourceDefaults` (Lauf F8). */
export interface AssetSourceDefaults {
  /** Belegdatum der Quell-Kosten (YYYY-MM-DD). */
  acquisitionDate: string;
  /** Brutto bei §19, netto bei Regelbesteuerung (§ 9b Abs. 1 EStG; Status bei Leistungsbezug = Belegdatum, Lauf F8d). */
  acquisitionCostCents: number;
  /** Betrag ohne Umsatzsteuer — Grundlage der GWG-Grenze. */
  gwgBasisCents: number;
  isKleinunternehmer: boolean;
  /** Kosten mit §13b-Schalter (Lauf F8c): bei §19 die selbst geschuldete Steuer als eigene Kosten + Anlage. */
  reverseCharge13b: boolean;
}

/** Korrespondiert zu `commands::assets::AssetSuggestion` (Lauf F8). */
export interface AssetSuggestion extends MethodSuggestion {
  source: AssetSourceDefaults | null;
}

/** Korrespondiert zu `accrue_yearly::AccrueReport` in Rust. */
export interface AccrueReport {
  skippedLocked: boolean;
  fiscalYear: number;
  processedAssets: number;
  bookedEntries: number;
  totalDepreciationCents: Cents;
}

// ---- EÜR / Steuer-Auswertung (Block 13) ------------------------------------

/** Korrespondiert zu `euer::aggregate::CategoryExpense` in Rust. */
export interface EuerCategoryExpense {
  category: string; // ExpenseCategory-Code → Label via expenseCategoryLabel()
  amountCents: Cents;
}

/** Korrespondiert zu `euer::aggregate::EuerReport` in Rust (Cash-Basis). */
export interface EuerReport {
  fiscalYear: number;
  // Betriebseinnahmen
  invoiceIncomeCents: Cents;
  stornoRefundsCents: Cents;
  disposalProceedsCents: Cents;
  totalIncomeCents: Cents;
  // Betriebsausgaben
  expensesByCategory: EuerCategoryExpense[];
  expensesTotalCents: Cents;
  depreciationTotalCents: Cents;
  disposalBookValueCents: Cents;
  totalExpensesCents: Cents;
  // Abgeleitet
  disposalGainLossCents: Cents;
  // Ergebnis
  surplusCents: Cents;
  /** §19 das ganze Jahr (wie die Exporte); false = Regelbesteuerung oder
   *  Wechsel im Jahr. Lauf 2026.11 U4, ADR 0042. */
  isKleinunternehmer: boolean;
  /** Status des Jahres dreiwertig (ADR 0042 Nr. 3). */
  yearStatus: YearStatus;
  /** Wechsel im Jahr als Text; null = einheitlicher Status. */
  statusChange: string | null;
  /** Abgleich mit dem Festschreibungs-Snapshot; null = Jahr offen (U4). */
  snapshotCheck: SnapshotCheck | null;
}

// ---- ELSTER-Ausfüllhilfe / Anlage EÜR (Block 14a) --------------------------

/** Eine Zeile der ELSTER-Ausfüllhilfe (`euer::elster_csv::ElsterLine`). */
export interface ElsterLine {
  /** Zeilennummer im amtlichen Vordruck des Jahres (siehe `formYear`). */
  zeile: number;
  /** Amtliche Kennzahl (Kz), über die Jahre gleich. */
  kennzahl: number | null;
  bezeichnung: string;
  amountCents: Cents;
  /** true = im ELSTER-Formular einzutragen; false = ELSTER berechnet selbst. */
  isEntry: boolean;
}

/** ELSTER-Ausfüllhilfe eines Geschäftsjahres (`euer::elster_csv::ElsterForm`). */
export interface ElsterForm {
  fiscalYear: number;
  /** §19 das ganze Jahr; false bei Regelbesteuerung oder Wechsel im Jahr. */
  isKleinunternehmer: boolean;
  /** Wechsel des Steuerstatus im Jahr als Text (ADR 0042); fehlt sonst. */
  statusChange?: string | null;
  /** Jahr des Vordrucks, dem die Zeilennummern folgen. */
  formYear: number;
  /** true = Zeilennummern am Vordruck genau dieses Jahres geprüft. */
  layoutVerified: boolean;
  lines: ElsterLine[];
  incomeTotalCents: Cents;
  expenseTotalCents: Cents;
  surplusCents: Cents;
  /** Hinweise der Ausfüllhilfe (Vordruck, Regelbesteuerung, Storno-Überhang). */
  notes: string[];
}

/** Download-Ticket (Plan W7, `downloads::DownloadTicket`): die Datei liegt kurz
 *  auf dem Server und wird über `GET /files/download/{downloadId}` geholt. */
export interface DownloadTicket {
  downloadId: string;
  fileName: string;
  sizeBytes: number;
}

/** Ergebnis des CSV-Exports (`commands::euer::ElsterExportResult`). */
export interface ElsterExportResult {
  download: DownloadTicket;
  lineCount: number;
  entryCount: number;
  snapshotCheck: SnapshotCheck | null;
}

// ---- Einzelaufstellung + Anlageverzeichnis (Block 14a) ---------------------

/** Ein Zahlungseingang (`euer::detail::IncomeItem`). */
export interface EuerIncomeItem {
  paidDate: string;
  invoiceNumber: string;
  customer: string;
  description: string;
  /** Mit Vorzeichen: eine Gegenbuchung ist negativ (ADR 0042 Nr. 9). */
  amountCents: Cents;
  /** Gegenbuchung einer storniert erfassten Zahlung (am ursprünglichen Zahldatum). */
  isReversal?: boolean;
  /** Diese Zahlung ist durch eine Gegenbuchung storniert. */
  reversed?: boolean;
}

/** Eine Storno-Erstattung (`euer::detail::StornoItem`). */
export interface EuerStornoItem {
  stornoDate: string;
  stornoNumber: string;
  originalNumber: string;
  refundedCents: Cents;
}

/** Eine bezahlte Kostenposition (`euer::detail::ExpenseItem`). */
export interface EuerExpenseItem {
  paidDate: string;
  expenseNumber: string;
  vendor: string;
  category: string; // ExpenseCategory-Code → Label via expenseCategoryLabel()
  description: string;
  grossCents: Cents;
  /** §19-Status bei Leistungsbezug (Jahr des Belegdatums, Lauf F8d). */
  isKleinunternehmer: boolean;
}

/** Eine Anlagen-Veräußerung (`euer::detail::DisposalItem`). */
export interface EuerDisposalItem {
  disposalDate: string;
  assetNumber: string;
  label: string;
  proceedsCents: Cents;
  residualBookValueCents: Cents;
  gainLossCents: Cents;
}

/** Eine Zeile des Anlageverzeichnisses (`euer::detail::AveeurItem`). */
export interface AveeurItem {
  assetNumber: string;
  label: string;
  acquisitionDate: string;
  acquisitionCostCents: Cents;
  depreciationMethod: string;
  usefulLifeYears: number | null;
  businessSharePercent: number;
  afaYearCents: Cents;
  bookValueStartCents: Cents;
  bookValueEndCents: Cents;
  disposedInYear: boolean;
  disposalDate: string | null;
  disposalProceedsCents: Cents | null;
}

/** EÜR-Eckwerte in Cent (`fiscal_year::snapshot::Figures`). */
export interface SnapshotFigures {
  incomeCents: Cents;
  expenseCents: Cents;
  afaCents: Cents;
  surplusCents: Cents;
}

/**
 * Abgleich eines abgeschlossenen Jahres mit dem Festschreibungs-Snapshot
 * (`fiscal_year::snapshot::SnapshotCheck`, Lauf F, KB-0031).
 */
export interface SnapshotCheck {
  fiscalYear: number;
  closedAt: Iso8601;
  closedWithVersion: string;
  snapshot: SnapshotFigures;
  live: SnapshotFigures;
  matches: boolean;
  /** `income` | `expenses` | `afa` | `surplus` */
  differences: string[];
}

/** Komplettes EÜR-Paket eines GJ (`commands::euer::EuerPackage`). */
export interface EuerPackage {
  form: ElsterForm;
  income: EuerIncomeItem[];
  storno: EuerStornoItem[];
  expenses: EuerExpenseItem[];
  disposals: EuerDisposalItem[];
  assets: AveeurItem[];
  /** Nur abgeschlossene Jahre (Lauf F). */
  snapshotCheck: SnapshotCheck | null;
}

/** Ergebnis des Einzelaufstellung-ZIP-Exports (`commands::euer::EuerDetailExport`). */
export interface EuerDetailExport {
  download: DownloadTicket;
  incomeCount: number;
  expenseCount: number;
  assetCount: number;
  snapshotCheck: SnapshotCheck | null;
}

/** Offene AfA-Buchungen eines GJ (`commands::euer::AfaPending`). */
export interface AfaPending {
  fiscalYear: number;
  pendingCount: number;
}

/** Ergebnis des PDF-Exports „Anlage EÜR" (`commands::euer::EuerPdfExport`). */
export interface EuerPdfExport {
  download: DownloadTicket;
  sizeBytes: number;
  snapshotCheck: SnapshotCheck | null;
}

/** Optionale Kopf-Nummern des DATEV-Stapels (Feld 11/12): Beraternummer
 *  1001–9999999, Mandantennummer 1–99999 (Lauf F5, Durchreichen U4). */
export interface DatevHeaderNumbers {
  consultantNumber?: number | null;
  clientNumber?: number | null;
}

/** Ergebnis des DATEV-Buchungsstapel-Exports (`commands::euer::DatevExport`). */
export interface DatevExport {
  download: DownloadTicket;
  bookingCount: number;
  snapshotCheck: SnapshotCheck | null;
}

/** Ergebnis des Steuerberater-Paket-Exports (`commands::euer::StbExport`). */
export interface StbExport {
  download: DownloadTicket;
  fileCount: number;
  snapshotCheck: SnapshotCheck | null;
}

// ---- Block 15: Notifications / Hinweise ------------------------------------

export type NotificationSeverity = "info" | "warning" | "urgent";

/** Korrespondiert zu `notify::store::Notification` in Rust. */
export interface AppNotification {
  id: Uuid;
  ruleId: string | null;
  title: string;
  body: string;
  severity: string; // NotificationSeverity
  relatedEntityType: string | null;
  relatedEntityId: string | null;
  triggeredAt: Iso8601;
  dismissedAt: Iso8601 | null;
  actionUrl: string | null;
  dedupKey: string | null;
}

/** Korrespondiert zu `notify::rules::NotificationRule` in Rust. */
export interface NotificationRule {
  id: string;
  ruleType: string;
  label: string;
  enabled: number; // 0|1
  configJson: string;
  deliverInApp: number; // 0|1
  deliverOsNative: number; // 0|1 — Spalte der Desktop-Fassung, ohne Wirkung (Lauf F)
  createdAt: Iso8601;
}

// ---- Block 15: Geschäftsjahr-Abschluss (GJ-Lock) ---------------------------

/** Status eines Geschäftsjahres in der Übersicht (`commands::fiscal_year`). */
export interface FiscalYearStatus {
  fiscalYear: number;
  closed: boolean;
  closedAt: Iso8601 | null;
  incomeTotalCents: Cents;
  expenseTotalCents: Cents;
  afaTotalCents: Cents;
  surplusCents: Cents;
  closable: boolean;
  afaPending: number;
  /** Offene Entwürfe des Jahres (Lauf F): Warnhinweis vor dem Abschluss. */
  openDrafts: OpenDrafts;
  /** Nur abgeschlossene Jahre: Snapshot und Live-Rechnung stimmen überein. */
  snapshotMatches: boolean | null;
}

/** Offene Entwürfe eines Jahres (`fiscal_year::lock::OpenDrafts`). */
export interface OpenDrafts {
  invoices: number;
  quotes: number;
}

/** Offene Forderung (Carry-over, `fiscal_year::transition::OpenReceivable`). */
export interface OpenReceivable {
  id: Uuid;
  invoiceNumber: string;
  invoiceDate: Iso8601;
  dueDate: Iso8601 | null;
  fiscalYear: number;
  outstandingCents: Cents;
}

/** GJ-Übersicht (`commands::fiscal_year::FiscalYearOverview`). */
export interface FiscalYearOverview {
  currentYear: number;
  autoYearClose: boolean;
  years: FiscalYearStatus[];
  openReceivables: OpenReceivable[];
}

/** Festschreibungsprotokoll (`fiscal_year::lock::FiscalYearLock`). */
export interface FiscalYearLock {
  fiscalYear: number;
  closedAt: Iso8601;
  incomeTotalCents: Cents;
  expenseTotalCents: Cents;
  afaTotalCents: Cents;
  surplusCents: Cents;
  assetsLocked: number;
  depreciationEntriesLocked: number;
  appVersion: string;
  schemaVersion: number;
  notes: string | null;
  /** Nur in der Antwort des Abschlusses (Lauf F). */
  openDrafts?: OpenDrafts;
}

// ---- Block 15: System / Audit-Trail / Integrity ----------------------------

/** Korrespondiert zu `db::repo::audit_log::AuditEntry` in Rust. */
export interface AuditEntry {
  id: number;
  timestampUtc: Iso8601;
  actor: string;
  action: string;
  entityType: string | null;
  entityId: string | null;
  detailsJson: string | null;
}

/** Ergebnis eines manuellen Integrity-Scans (`archive::IntegrityCheckSummary`). */
export interface IntegrityCheckSummary {
  checkId: number;
  filesChecked: number;
  filesPassed: number;
  filesFailed: number;
  failedArchiveIds: string[];
  /** Vorhanden, aber nicht lesbar (z. B. Rechte) — kein Manipulationsbefund (W2). */
  filesUnreadable: number;
  unreadableArchiveIds: string[];
}

/** Eine Zeile der Integrity-Check-Historie (`commands::system::IntegrityCheckRow`). */
export interface IntegrityCheckRow {
  id: number;
  startedAt: Iso8601;
  finishedAt: Iso8601 | null;
  filesChecked: number;
  filesPassed: number;
  filesFailed: number;
  failedArchiveIdsJson: string | null;
  /** Teilmenge von filesFailed: fehlende Dateien; null = Lauf vor Migration 0032. */
  filesMissing: number | null;
  /** Vorhanden, aber nicht lesbar (kein Manipulationsbefund); null = vor 0032. */
  filesUnreadable: number | null;
  missingArchiveIdsJson: string | null;
  unreadableArchiveIdsJson: string | null;
}

/** Statische App-Metadaten für den „Über"-Dialog (`commands::system::AppInfo`). */
export interface AppInfo {
  appVersion: string;
  schemaVersion: number;
  identifier: string;
  licenseSpdx: string;
  /** Wirksame Quell-URL (AGPL §13; `KLEIN_BUCH_SOURCE_URL`, sonst Build-Wert). */
  repositoryUrl: string;
  /** Voller Commit-SHA des Builds; `null` im Entwicklungs-Build (Plan W12). */
  buildCommit: string | null;
  /** Release-Tag des Builds; `null` außerhalb von Release-Builds. */
  buildTag: string | null;
  /** Link auf den exakten Quellstand; `null` ohne bekannten Commit. */
  sourceLink: string | null;
}

/** Opt-in Versionsprüfung (`commands::settings::UpdateCheckSettings`, Plan W12). */
export interface UpdateCheckSettings {
  enabled: boolean;
  /** Letzte Abfrage (RFC 3339, UTC) oder `null`. */
  lastCheckAt: string | null;
}

// ---- Steuerstatus nach § 19 UStG (ADR 0042 Nr. 1–6) ------------------------

export type TaxStatus = "kleinunternehmer" | "regelbesteuerung";
export type TaxStatusReason =
  | "initial"
  | "verzicht"
  | "wegfall_vorjahr"
  | "wegfall_laufend"
  | "wegfall_gruendungsjahr"
  | "rueckkehr"
  | "legacy";
/** Status eines Geschäftsjahres; `wechsel` = Wechsel im Jahr. */
export type YearStatus = "kleinunternehmer" | "regelbesteuerung" | "wechsel";

/** Eintrag der Statushistorie (append-only, Widerruf statt Löschen). */
export interface TaxStatusChange {
  id: string;
  /** YYYY-MM-DD; „0001-01-01“ = seit Beginn. */
  effectiveFrom: string;
  status: TaxStatus;
  reason: TaxStatusReason;
  /** Nach einem Verzicht: erster Tag der möglichen Rückkehr. */
  bindingUntil: string | null;
  triggerDocument: string | null;
  note: string | null;
  createdAt: string;
  revokedAt: string | null;
  revokeReason: string | null;
}

/** Hinweis zu einer Umsatzgrenze (§ 19 Abs. 1/2 UStG). */
export interface TaxThresholdHint {
  kind: "vorjahr" | "laufend" | "gruendungsjahr";
  year: number;
  turnoverCents: Cents;
  limitCents: Cents;
  /** true = überschritten, false = genau erreicht. */
  exceeded: boolean;
  triggerDocument: string | null;
  suggestedEffectiveFrom: string | null;
  suggestedReason: TaxStatusReason;
  /** true = Gründungsjahr nur vermutet. */
  presumed: boolean;
}

export interface TaxStatusOverview {
  entries: TaxStatusChange[];
  today: string;
  currentStatus: TaxStatus;
  bindingUntil: string | null;
  earliestReturn: string | null;
  /** Nur der jüngste gültige Eintrag lässt sich widerrufen. */
  revocableId: string | null;
  turnover: {
    previousYear: number;
    previousYearCents: Cents;
    currentYear: number;
    currentYearCents: Cents;
  };
  thresholdHints: TaxThresholdHint[];
  hinweisText: string;
}

export interface TaxStatusAt {
  date: string;
  status: TaxStatus;
  isKleinunternehmer: boolean;
}

export interface TaxStatusChangeInput {
  /** Pflicht außer beim Anfangsstand (`initial`, seit Beginn). */
  effectiveFrom: string | null;
  status: TaxStatus;
  reason: TaxStatusReason;
  triggerDocument?: string | null;
  note?: string | null;
}

export interface TaxStatusAffectedInvoice {
  id: string;
  number: string;
  documentDate: string;
  lockedAsKleinunternehmer: boolean;
  nowKleinunternehmer: boolean;
}

export interface TaxStatusConsequences {
  firstDifference: string | null;
  affectedInvoices: TaxStatusAffectedInvoice[];
  affectedDrafts: string[];
  recurringTemplates: string[];
  capitalizedAssets: string[];
  reexportYears: number[];
}

export interface TaxStatusChangePreview {
  ok: boolean;
  errors: string[];
  /** Hinweise, die den Eintrag nicht sperren (Erklärungsfrist des Verzichts, Vorjahresumsatz vor einer Rückkehr). */
  warnings: string[];
  bindingUntil: string | null;
  consequences: TaxStatusConsequences;
}

export interface TaxStatusChangeResult {
  entry: TaxStatusChange;
  consequences: TaxStatusConsequences;
}
