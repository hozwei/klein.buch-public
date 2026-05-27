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

// ---- Drop-Folder fuer eingehende E-Rechnungen (Block PV1-DROP) -------------

/** Konfiguration des Watched-Folders fuer eingehende E-Rechnungen. */
export interface DropFolderSettings {
  /** Periodischer Sync (5-min-Tick + App-Start-Sweep) aktiv? */
  enabled: boolean;
  /** Absoluter Ordner-Pfad. Leer = noch nicht gewaehlt. */
  path: string;
}

/** Ergebnis-Zaehler eines manuellen `Jetzt synchronisieren`-Laufs. */
export interface DropFolderSyncResult {
  /** `true`, wenn der Lauf uebersprungen wurde (Toggle off oder Pfad leer/ungueltig). */
  skippedDisabled: boolean;
  imported: number;
  failed: number;
  ignoredHidden: number;
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
  isKleinunternehmer: number;          // 0|1
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
  isKleinunternehmer: boolean;
  defaultPdfTemplate: string | null;
  defaultCurrency: string | null;
  /** Pflicht-`true` beim Wechsel §19 → Regelbesteuerung (5-Jahres-Bindung). */
  confirmWaiveParagraph19: boolean | null;
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
}

export interface Paragraph19Info {
  hinweisText: string;
  /** Wenn aktuell verzichtet wurde: das früheste Rückkehr-Datum (YYYY-MM-DD). */
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
  paidAmountCents: Cents;
  paidAt: Iso8601 | null;
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
  // Ursprungsangebot bei Konvertierung (Block 7, Migration 0005).
  derivedFromQuoteId: Uuid | null;
  // Zahlungs-Eingangs-Konto (Migration 0007, Block 9). Additiv.
  paidToAccountId: Uuid | null;
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
  fiscalYear: number;
  buyerReference: string | null;
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
}

// ---- Mail (Block 5) --------------------------------------------------------

/** Korrespondiert zu `MailAccountRow` in Rust. Geheimnisse stehen NICHT hier. */
export interface MailAccount {
  id: Uuid;
  label: string;
  authType: string;          // "smtp_password" | "oauth_microsoft" (Block 16)
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
  // OAuth (Block 16) — nicht-geheime Metadaten.
  oauthTenantId: string | null;
  oauthClientId: string | null;
  oauthAccountEmail: string | null;
  oauthScopes: string | null;
  oauthTokenExpiresAt: Iso8601 | null;
}

export interface MailAccountInput {
  label: string;
  authType: string;          // "smtp_password" | "oauth_microsoft"
  smtpHost: string | null;
  smtpPort: number | null;
  smtpUser: string | null;
  smtpUseTls: boolean;
  fromEmail: string;
  fromName: string;
  isDefault: boolean;
  // OAuth (Block 16): nutzer-eigene Azure-App. Kein Secret.
  oauthTenantId: string | null;
  oauthClientId: string | null;
}

/** OAuth-Verbindungsstatus eines Microsoft-Kontos (Block 16). */
export interface OauthStatus {
  accountId: Uuid;
  connected: boolean;
  accountEmail: string | null;
  scopes: string | null;
  tokenExpiresAt: Iso8601 | null;
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
  fiscalYear: number;
  input: QuoteInputDto;
}

export interface QuoteAcceptArgs {
  quoteId: Uuid;
  /** Fachliches Annahmedatum (YYYY-MM-DD). null → heute (Backend). */
  acceptedDate: string | null;
  /** Roh-Bytes des hochgeladenen unterschriebenen Vertrags. null → ohne Doc. */
  signedContractBytes: number[] | null;
  /** Original-Dateiname des Uploads (für MIME/Anhang-Name). */
  signedContractFilename: string | null;
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
  fiscalYear: number | null;
  receiptBytes: number[] | null;
  receiptFilename: string | null;
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
  fiscalYear: number | null;
  /** Original-Datei (XML oder ZUGFeRD-PDF) — wird unverändert archiviert. */
  originalBytes: number[];
  originalFileName: string;
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
  fiscalYear: number | null;
  receiptBytes: number[] | null;
  receiptFilename: string | null;
}

// ---- Generischer Anhang-Upload (Block 9) -----------------------------------

export interface AddAttachmentArgs {
  parentType: string;
  parentId: Uuid;
  fileBytes: number[];
  fileName: string;
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
  processedSubscriptions: number;
  createdExpenses: number;
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
  servicePeriodNote: boolean;
  notes: string | null;
  items: InvoiceItemInput[];
}

/** Korrespondiert zu `ProcessReport` (scheduler::recurring_invoice) in Rust. */
export interface RecurringInvoiceProcessReport {
  skippedLocked: boolean;
  processedTemplates: number;
  createdInvoices: number;
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
}

// ---- ELSTER-Ausfüllhilfe / Anlage EÜR (Block 14a) --------------------------

/** Eine Zeile der ELSTER-Ausfüllhilfe (`euer::elster_csv::ElsterLine`). */
export interface ElsterLine {
  /** Offizielle Zeilennummer der Anlage EÜR; 0 = reine Kontrollsumme. */
  zeile: number;
  bezeichnung: string;
  amountCents: Cents;
  /** true = im ELSTER-Formular einzutragen; false = ELSTER berechnet selbst. */
  isEntry: boolean;
}

/** ELSTER-Ausfüllhilfe eines Geschäftsjahres (`euer::elster_csv::ElsterForm`). */
export interface ElsterForm {
  fiscalYear: number;
  isKleinunternehmer: boolean;
  lines: ElsterLine[];
  incomeTotalCents: Cents;
  expenseTotalCents: Cents;
  surplusCents: Cents;
}

/** Ergebnis des CSV-Exports (`commands::euer::ElsterExportResult`). */
export interface ElsterExportResult {
  csvPath: string;
  lineCount: number;
  entryCount: number;
}

// ---- Einzelaufstellung + Anlageverzeichnis (Block 14a) ---------------------

/** Ein Zahlungseingang (`euer::detail::IncomeItem`). */
export interface EuerIncomeItem {
  paidDate: string;
  invoiceNumber: string;
  customer: string;
  description: string;
  amountCents: Cents;
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

/** Komplettes EÜR-Paket eines GJ (`commands::euer::EuerPackage`). */
export interface EuerPackage {
  form: ElsterForm;
  income: EuerIncomeItem[];
  storno: EuerStornoItem[];
  expenses: EuerExpenseItem[];
  disposals: EuerDisposalItem[];
  assets: AveeurItem[];
}

/** Ergebnis des Einzelaufstellung-ZIP-Exports (`commands::euer::EuerDetailExport`). */
export interface EuerDetailExport {
  zipPath: string;
  incomeCount: number;
  expenseCount: number;
  assetCount: number;
}

/** Offene AfA-Buchungen eines GJ (`commands::euer::AfaPending`). */
export interface AfaPending {
  fiscalYear: number;
  pendingCount: number;
}

/** Ergebnis des PDF-Exports „Anlage EÜR" (`commands::euer::EuerPdfExport`). */
export interface EuerPdfExport {
  pdfPath: string;
  sizeBytes: number;
}

/** Ergebnis des DATEV-Buchungsstapel-Exports (`commands::euer::DatevExport`). */
export interface DatevExport {
  csvPath: string;
  bookingCount: number;
}

/** Ergebnis des Steuerberater-Paket-Exports (`commands::euer::StbExport`). */
export interface StbExport {
  zipPath: string;
  fileCount: number;
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
  deliverOsNative: number; // 0|1
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
}

/** Statische App-Metadaten für den „Über"-Dialog (`commands::system::AppInfo`). */
export interface AppInfo {
  appVersion: string;
  schemaVersion: number;
  identifier: string;
  licenseSpdx: string;
  repositoryUrl: string;
  /** Kurzer Commit-Hash vom Release-CI bzw. `"dev"` im lokalen Build. */
  buildCommit: string;
}
