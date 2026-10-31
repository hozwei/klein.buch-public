// Backend-Aufrufe. Jede Funktion ruft einen Command-Kern über
// `invoke<T>(name, args)` auf — seit Plan W5 per HTTP (`src/lib/transport.ts`,
// `POST /api/cmd/{name}`), vorher über die Bridge der Desktop-Fassung.
// Block 2: contacts_* + seller_profile_* + paragraph_19_info.
// Block 3b: invoices_* — Draft, Lock-and-Issue, Cancel, Payment.

import { invoke, saveFile } from "./transport";
export { uploadFile } from "./transport";
export type { UploadArt, UploadTicket } from "./transport";

// Anmeldung und Sitzung (Plan W6) laufen über `/api/auth/*`.
export {
  authStatus,
  authSetupOffer,
  authSetupFinish,
  authLogin,
  authReauth,
  authLogout,
  authLock,
  authSecurity,
  authSessions,
  authRevokeOtherSessions,
  authTotpRenewOffer,
  authTotpRenewFinish,
  authRecoveryRenew,
  authChangePassphrase,
  PassphraseChangeUnclear,
} from "./transport";
export type {
  AuthState,
  AuthStatus,
  AuthSession,
  TotpOffer,
  SecondFactor,
  LoginResult,
} from "./transport";
import type {
  AnonymizeCheck,
  DownloadTicket,
  CancelArgs,
  CancelResponse,
  Contact,
  ContactInput,
  CreateDraftArgs,
  Invoice,
  InvoiceDetail,
  InvoiceListFilter,
  InvoiceListItem,
  LockResponse,
  MailAccount,
  MailAccountInput,
  EmailLogEntry,
  EmailLogFilter,
  Paragraph19Info,
  TaxStatusAt,
  TaxStatusChangeInput,
  TaxStatusChangePreview,
  TaxStatusChangeResult,
  TaxStatusOverview,
  PdfTemplateMeta,
  TravelSettings,
  TravelLine,
  RecordPaymentArgs,
  ReversePaymentArgs,
  RenderedMail,
  SellerProfile,
  SellerProfileInput,
  SendInvoiceArgs,
  SendResult,
  TestConnectionArgs,
  ValidationIssueDto,
  AttachmentView,
  Quote,
  QuoteAcceptArgs,
  QuoteCancelArgs,
  QuoteConvertArgs,
  QuoteCreateDraftArgs,
  QuoteDetail,
  QuoteListFilter,
  QuoteListItem,
  QuoteRejectArgs,
  LegalDocument,
  QuoteLegalBinding,
  Package,
  PackageCategory,
  PackageRevision,
  PackageRevisionInput,
  MaterializedPackageItem,
  SendPackageCatalogArgs,
  SendPackageCatalogResult,
  SendQuoteArgs,
  SendQuoteResult,
  PaymentAccount,
  PaymentAccountInput,
  Expense,
  ExpenseDetail,
  ExpenseListItem,
  ExpenseListFilter,
  ExpenseCreateArgs,
  ExpenseCancelArgs,
  ExpenseSetPaymentArgs,
  PrivateMovement,
  PrivateMovementListItem,
  PrivateMovementListFilter,
  PrivateMovementCreateArgs,
  AddAttachmentArgs,
  RecurringSubscription,
  ResumeMode,
  SchedulerHoldStatus,
  RecurringInputDto,
  RecurringProcessReport,
  RecurringInvoiceRow,
  RecurringInvoiceDetail,
  RecurringInvoiceInputDto,
  RecurringInvoiceProcessReport,
  EInvoiceParseResult,
  EInvoiceCreateArgs,
  XmlViewerPayload,
  AssetDetail,
  AssetListItem,
  AssetListFilter,
  AssetInputDto,
  AssetDisposeArgs,
  AssetDisposalPreview,
  AfaTabellen,
  AssetSuggestion,
  DepreciationEntry,
  AccrueReport,
  EuerReport,
  ElsterExportResult,
  EuerPackage,
  EuerDetailExport,
  EuerPdfExport,
  DatevExport,
  DatevHeaderNumbers,
  StbExport,
  AfaPending,
  AppNotification,
  NotificationRule,
  FiscalYearOverview,
  FiscalYearLock,
  AuditEntry,
  IntegrityCheckSummary,
  IntegrityCheckRow,
  AppInfo,
  UpdateCheckSettings,
} from "./types";

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

// ---- Contacts ---------------------------------------------------------------

export function contactsList(includeArchived = false): Promise<Contact[]> {
  return invoke<Contact[]>("contacts_list", { includeArchived });
}

export function contactsGet(id: string): Promise<Contact | null> {
  return invoke<Contact | null>("contacts_get", { id });
}

export function contactsCreate(input: ContactInput): Promise<Contact> {
  return invoke<Contact>("contacts_create", { input });
}

export function contactsUpdate(id: string, input: ContactInput): Promise<Contact> {
  return invoke<Contact>("contacts_update", { id, input });
}

export function contactsArchive(id: string): Promise<void> {
  return invoke<void>("contacts_archive", { id });
}

export function contactsUnarchive(id: string): Promise<void> {
  return invoke<void>("contacts_unarchive", { id });
}

export function contactsSearch(query: string, includeArchived = false): Promise<Contact[]> {
  return invoke<Contact[]>("contacts_search", { query, includeArchived });
}

// ---- DSGVO-Anonymisierung (Art. 17, Block 19) ------------------------------

/** Vorab-Prüfung: darf der Kontakt anonymisiert werden, und was bleibt erhalten? */
export function contactsAnonymizeCheck(id: string): Promise<AnonymizeCheck> {
  return invoke<AnonymizeCheck>("contacts_anonymize_check", { id });
}

/** Anonymisiert den Kontakt (irreversibel). Überschreibt die personenbezogenen
 *  Stammdaten; festgeschriebene Belege bleiben über den Buyer-Snapshot erhalten. */
export function contactsAnonymize(id: string): Promise<Contact> {
  return invoke<Contact>("contacts_anonymize", { id });
}

// ---- DSGVO-Auskunft (Art. 15, Block 18) ------------------------------------

export interface DsgvoExportResult {
  fileName: string;
  /** Abruf über `/files/download/{id}` (Plan W7). */
  download: DownloadTicket | null;
  invoiceCount: number;
  quoteCount: number;
  expenseCount: number;
  documentCount: number;
  bundledDocumentCount: number;
  emailCount: number;
}

/** Erzeugt die DSGVO-Auskunft (Art. 15) zu einem Kontakt als ZIP (PDF + JSON +
 *  Originaldateien) zum Herunterladen. Read-only. */
export function dsgvoExport(contactId: string): Promise<DsgvoExportResult> {
  return invoke<DsgvoExportResult>("dsgvo_export", { contactId });
}

// ---- Seller Profile --------------------------------------------------------

export function sellerProfileGet(): Promise<SellerProfile | null> {
  return invoke<SellerProfile | null>("seller_profile_get");
}

export function sellerProfileUpsert(input: SellerProfileInput): Promise<SellerProfile> {
  return invoke<SellerProfile>("seller_profile_upsert", { input });
}

export function paragraph19Info(): Promise<Paragraph19Info> {
  return invoke<Paragraph19Info>("paragraph_19_info");
}

// ---- Steuerstatus nach § 19 UStG (ADR 0042) ---------------------------------

/** Historie, Status heute, Bindung, Gesamtumsatz und Grenzhinweise. */
export function taxStatusOverview(): Promise<TaxStatusOverview> {
  return invoke<TaxStatusOverview>("tax_status_overview");
}

/** Status an einem Datum (YYYY-MM-DD): Leistungsdatum, sonst Belegdatum. */
export function taxStatusAt(date: string): Promise<TaxStatusAt> {
  return invoke<TaxStatusAt>("tax_status_at", { date });
}

/** Prüft einen geplanten Eintrag und liefert seine Folgen; schreibt nichts. */
export function taxStatusChangePreview(input: TaxStatusChangeInput): Promise<TaxStatusChangePreview> {
  return invoke<TaxStatusChangePreview>("tax_status_change_preview", { input });
}

/** Trägt einen Wechsel ein (frische Re-Auth). */
export function taxStatusChangeAdd(input: TaxStatusChangeInput): Promise<TaxStatusChangeResult> {
  return invoke<TaxStatusChangeResult>("tax_status_change_add", { input });
}

/** Widerruft den jüngsten gültigen Eintrag mit Grund (frische Re-Auth). */
export function taxStatusChangeRevoke(id: string, reason: string): Promise<TaxStatusChangeResult> {
  return invoke<TaxStatusChangeResult>("tax_status_change_revoke", { id, reason });
}

/** Logo aus einem Upload der Art `logo` (Plan W7). */
export function sellerLogoSet(uploadId: string): Promise<SellerProfile> {
  return invoke<SellerProfile>("seller_logo_set", { uploadId });
}

export function sellerLogoClear(): Promise<SellerProfile> {
  return invoke<SellerProfile>("seller_logo_clear");
}

export function sellerLogoData(): Promise<string | null> {
  return invoke<string | null>("seller_logo_data");
}

// Angebots-Unterschrift (Bild-Upload wie Logo) + globaler Toggle
/** Unterschrift aus einem Upload der Art `unterschrift` (Plan W7). */
export function sellerSignatureSet(uploadId: string): Promise<string | null> {
  return invoke<string | null>("seller_signature_set", { uploadId });
}

export function sellerSignatureClear(): Promise<void> {
  return invoke<void>("seller_signature_clear");
}

export function sellerSignatureData(): Promise<string | null> {
  return invoke<string | null>("seller_signature_data");
}

export function quoteSignatureGet(): Promise<boolean> {
  return invoke<boolean>("quote_signature_get");
}

export function quoteSignatureSet(enabled: boolean): Promise<void> {
  return invoke<void>("quote_signature_set", { enabled });
}

// Opt-in Versionsprüfung gegen die GitHub-Releases (Plan W12, E14)
export function updateCheckGet(): Promise<UpdateCheckSettings> {
  return invoke<UpdateCheckSettings>("update_check_get");
}

export function updateCheckSet(enabled: boolean): Promise<void> {
  return invoke<void>("update_check_set", { enabled });
}

// Inhaber (Vor-/Nachname) + Standard-Fristen
export function sellerOwnerGet(): Promise<string | null> {
  return invoke<string | null>("seller_owner_get");
}

export function sellerOwnerSet(ownerName: string | null): Promise<void> {
  return invoke<void>("seller_owner_set", { ownerName });
}

export interface DocumentTerms {
  quoteValidDays: number;
  invoiceDueDays: number;
}

export function documentTermsGet(): Promise<DocumentTerms> {
  return invoke<DocumentTerms>("document_terms_get");
}

export function documentTermsSet(
  quoteValidDays: number,
  invoiceDueDays: number,
): Promise<void> {
  return invoke<void>("document_terms_set", { quoteValidDays, invoiceDueDays });
}

// Block 17a: PDF-Vorlagen (Switcher)
export function pdfTemplatesList(): Promise<PdfTemplateMeta[]> {
  return invoke<PdfTemplateMeta[]>("pdf_templates_list");
}

export function sellerDefaultTemplateSet(name: string): Promise<SellerProfile> {
  return invoke<SellerProfile>("seller_default_template_set", { name });
}

/** Musterrechnung mit der Vorlage (PDF-Vorschau, Plan W7). */
export function pdfTemplatePreview(name: string): Promise<DownloadTicket> {
  return invoke<DownloadTicket>("pdf_template_preview", { name });
}

// ---- Anfahrtspauschale (Block P1) ------------------------------------------

export function travelSettingsGet(): Promise<TravelSettings> {
  return invoke<TravelSettings>("travel_settings_get");
}

export function travelSettingsSet(
  costPerKmCents: number,
  roundTripDefault: boolean,
): Promise<void> {
  return invoke<void>("travel_settings_set", { costPerKmCents, roundTripDefault });
}

/** Berechnet eine Anfahrts-Position aus km + gespeichertem Kilometersatz. */
export function travelCompute(km: number, roundTrip: boolean): Promise<TravelLine> {
  return invoke<TravelLine>("travel_compute", { km, roundTrip });
}

// ---- Invoices (Block 3b) ---------------------------------------------------

export function invoicesList(filter?: InvoiceListFilter): Promise<InvoiceListItem[]> {
  return invoke<InvoiceListItem[]>("invoices_list", { filter: filter ?? null });
}

export function invoicesGet(id: string): Promise<InvoiceDetail | null> {
  return invoke<InvoiceDetail | null>("invoices_get", { id });
}

export function invoicesCreateDraft(args: CreateDraftArgs): Promise<InvoiceDetail> {
  return invoke<InvoiceDetail>("invoices_create_draft", { args });
}

/** Bearbeitet einen offenen Rechnungs-Entwurf (Lauf F5); die Nummer bleibt. */
export function invoicesUpdateDraft(id: string, args: CreateDraftArgs): Promise<InvoiceDetail> {
  return invoke<InvoiceDetail>("invoices_update_draft", { id, args });
}

export function invoicesValidateDraft(id: string): Promise<ValidationIssueDto[]> {
  return invoke<ValidationIssueDto[]>("invoices_validate_draft", { id });
}

/**
 * Stellt einen Entwurf aus. `buyerReference` ist optional (ADR 0042 Nr. 12):
 * ein Wert (nicht leer, nicht „N/A“) ersetzt die am Entwurf gespeicherte
 * Leitweg-ID; ohne Wert gilt die gespeicherte, sonst „N/A“.
 */
export function invoicesLockAndIssue(
  id: string,
  buyerReference: string | null = null,
): Promise<LockResponse> {
  return invoke<LockResponse>("invoices_lock_and_issue", { id, buyerReference });
}

export function invoicesRecordPayment(args: RecordPaymentArgs): Promise<Invoice> {
  return invoke<Invoice>("invoices_record_payment", { args });
}

/**
 * Storniert eine falsch erfasste Zahlung durch eine Gegenbuchung (Re-Auth,
 * ADR 0042 Nr. 8): Betrag, Datum und Konto der Zahlung mit umgekehrtem
 * Vorzeichen, Grund mindestens 10 Zeichen. Danach richtig neu buchen.
 */
export function invoicesReversePayment(args: ReversePaymentArgs): Promise<Invoice> {
  return invoke<Invoice>("invoices_reverse_payment", args);
}

export function invoicesCancel(args: CancelArgs): Promise<CancelResponse> {
  return invoke<CancelResponse>("invoices_cancel", { args });
}

/**
 * Verwirft einen nummerierten Rechnungs- oder Storno-Entwurf (Re-Auth, Lauf F
 * W14-032). Kein Löschen: Zeitpunkt und Grund (mindestens 10 Zeichen) stehen am
 * Beleg, die Nummer bleibt verbraucht. Die Schaltfläche gestaltet der UI/UX-Lauf.
 */
export function invoicesVoidDraft(args: { id: string; reason: string }): Promise<Invoice> {
  return invoke<Invoice>("invoices_void_draft", args);
}


// ---- Mail (Block 5) --------------------------------------------------------

export function mailAccountsList(): Promise<MailAccount[]> {
  return invoke<MailAccount[]>("mail_accounts_list");
}

export function mailAccountCreate(
  input: MailAccountInput,
  password: string | null,
): Promise<MailAccount> {
  return invoke<MailAccount>("mail_account_create", { input, password });
}

export function mailAccountUpdate(
  id: string,
  input: MailAccountInput,
  password: string | null,
): Promise<MailAccount> {
  return invoke<MailAccount>("mail_account_update", { id, input, password });
}

export function mailAccountDelete(id: string): Promise<void> {
  return invoke<void>("mail_account_delete", { id });
}

export function mailAccountTestConnection(args: TestConnectionArgs): Promise<void> {
  return invoke<void>("mail_account_test_connection", { args });
}

/** Testet ein gespeichertes Konto mit seinem gespeicherten Passwort (Re-Auth, Plan W8). */
export function mailAccountTestSaved(id: string): Promise<void> {
  return invoke<void>("mail_account_test_saved", { id });
}

export function mailSendTest(accountId: string, to: string): Promise<void> {
  return invoke<void>("mail_send_test", { accountId, to });
}

// ---- E-Mail-Versandprotokoll (Block 16b) -----------------------------------

export function emailLogList(limit?: number): Promise<EmailLogEntry[]> {
  return invoke<EmailLogEntry[]>("email_log_list", { limit: limit ?? null });
}

export function emailLogFor(kind: string, id: string): Promise<EmailLogEntry[]> {
  return invoke<EmailLogEntry[]>("email_log_for", { kind, id });
}

export function emailLogSearch(filter: EmailLogFilter): Promise<EmailLogEntry[]> {
  return invoke<EmailLogEntry[]>("email_log_search", { filter });
}

export function mailInvoicePreview(invoiceId: string): Promise<RenderedMail> {
  return invoke<RenderedMail>("mail_invoice_preview", { invoiceId });
}

export function mailSendInvoice(args: SendInvoiceArgs): Promise<SendResult> {
  return invoke<SendResult>("mail_send_invoice", { args });
}

// ---- Quotes / Angebote (Block 6) -------------------------------------------

export function quotesList(filter?: QuoteListFilter): Promise<QuoteListItem[]> {
  return invoke<QuoteListItem[]>("quotes_list", { filter: filter ?? null });
}

export function quotesGet(id: string): Promise<QuoteDetail | null> {
  return invoke<QuoteDetail | null>("quotes_get", { id });
}

export function quotesCreateDraft(args: QuoteCreateDraftArgs): Promise<QuoteDetail> {
  return invoke<QuoteDetail>("quotes_create_draft", { args });
}

export function quotesValidateDraft(id: string): Promise<ValidationIssueDto[]> {
  return invoke<ValidationIssueDto[]>("quotes_validate_draft", { id });
}

export function quotesIssue(id: string): Promise<Quote> {
  return invoke<Quote>("quotes_issue", { id });
}

export function quotesAccept(args: QuoteAcceptArgs): Promise<QuoteDetail> {
  return invoke<QuoteDetail>("quotes_accept", { args });
}

export function quotesReject(args: QuoteRejectArgs): Promise<Quote> {
  return invoke<Quote>("quotes_reject", { args });
}

export function quotesCancel(args: QuoteCancelArgs): Promise<Quote> {
  return invoke<Quote>("quotes_cancel", { args });
}

// ---- Quote → Rechnung-Konvertierung (Block 7) ------------------------------

export function quotesConvertToInvoice(args: QuoteConvertArgs): Promise<InvoiceDetail> {
  return invoke<InvoiceDetail>("quotes_convert_to_invoice", { args });
}

// ---- Pakete (Block P2) -----------------------------------------------------

export function packageCategoriesList(): Promise<PackageCategory[]> {
  return invoke<PackageCategory[]>("package_categories_list");
}
export function packageCategoryCreate(name: string): Promise<PackageCategory> {
  return invoke<PackageCategory>("package_category_create", { name });
}
export function packageCategoryUpdate(id: string, name: string): Promise<void> {
  return invoke<void>("package_category_update", { id, name });
}
export function packageCategoriesReorder(orderedIds: string[]): Promise<void> {
  return invoke<void>("package_categories_reorder", { orderedIds });
}
export function packagesList(): Promise<Package[]> {
  return invoke<Package[]>("packages_list");
}
export function packagesGet(id: string): Promise<Package | null> {
  return invoke<Package | null>("packages_get", { id });
}
export function packagesCreate(
  categoryId: string | null,
  name: string,
  revision: PackageRevisionInput,
): Promise<Package> {
  return invoke<Package>("packages_create", { categoryId, name, revision });
}
export function packagesUpdateAsNewRevision(
  id: string,
  categoryId: string | null,
  name: string,
  revision: PackageRevisionInput,
): Promise<Package> {
  return invoke<Package>("packages_update_as_new_revision", { id, categoryId, name, revision });
}
export function packagesArchive(id: string): Promise<void> {
  return invoke<void>("packages_archive", { id });
}
export function packagesReactivate(id: string): Promise<void> {
  return invoke<void>("packages_reactivate", { id });
}
export function packageRevisionsList(packageId: string): Promise<PackageRevision[]> {
  return invoke<PackageRevision[]>("package_revisions_list", { packageId });
}
export function packageRevisionsGet(
  packageId: string,
  revision: number,
): Promise<PackageRevision | null> {
  return invoke<PackageRevision | null>("package_revisions_get", { packageId, revision });
}
export function packageRevisionsRollback(packageId: string, toRevision: number): Promise<Package> {
  return invoke<Package>("package_revisions_rollback", { packageId, toRevision });
}
export function packagePreview(
  title: string,
  bodyMarkup: string,
  defaultUnitPriceCents: number,
): Promise<DownloadTicket> {
  return invoke<DownloadTicket>("package_preview", { title, bodyMarkup, defaultUnitPriceCents });
}

// ---- Paket → Beleg-Position (Block P3) --------------------------------------

/** Materialisiert eine Beleg-Position aus der aktiven Revision eines Pakets.
 *  Reine Aufbereitung (schreibt nichts) — das Frontend hängt sie an die
 *  Positions-Liste. `documentDate` (YYYY-MM-DD): maßgebliches Datum des
 *  Zielbelegs für den §19-Status (ADR 0042 Nr. 3); ohne Angabe heute. */
export function packageMaterializeItem(
  packageId: string,
  documentDate?: string | null,
): Promise<MaterializedPackageItem> {
  return invoke<MaterializedPackageItem>("package_materialize_item", {
    packageId,
    documentDate: documentDate ?? null,
  });
}

// ---- Paket-Katalog-Broschüre (Block P4) -------------------------------------

/** Rendert die Broschüre für die gewählten Pakete (PDF-Vorschau, Plan W7).
 *  Kein §14-Beleg — kein Nummernkreis, kein Archiv. */
export function packageCatalogRender(packageIds: string[]): Promise<DownloadTicket> {
  return invoke<DownloadTicket>("package_catalog_render", { packageIds });
}

/** Versendet die Broschüre als PDF-Anhang an einen Kontakt. Protokolliert im
 *  append-only email_log (related_kind = 'package_catalog'). */
export function packageCatalogSend(
  args: SendPackageCatalogArgs,
): Promise<SendPackageCatalogResult> {
  return invoke<SendPackageCatalogResult>("package_catalog_send", { args });
}

// ---- Angebots-PDF + Bundle + Versand (Block 8) -----------------------------

/** Erzeugt (falls nötig) das Angebots-PDF; das Frontend zeigt es danach über
 *  `archiveUrl(quote.pdfArchiveId)`. */
export function quotesGeneratePdf(id: string): Promise<Quote> {
  return invoke<Quote>("quotes_generate_pdf", { id });
}

/** Erzeugt das zusammengeführte Bundle-PDF (Angebot + AGB + Datenschutz) als
 *  PDF-Vorschau (Plan W7). Bindet die Legal-Versionen (Pflicht). */
export function quotesOpenBundle(id: string): Promise<DownloadTicket> {
  return invoke<DownloadTicket>("quotes_open_bundle", { id });
}

export function quotesLegalBindings(quoteId: string): Promise<QuoteLegalBinding[]> {
  return invoke<QuoteLegalBinding[]>("quotes_legal_bindings", { quoteId });
}

export function mailQuotePreview(quoteId: string): Promise<RenderedMail> {
  return invoke<RenderedMail>("mail_quote_preview", { quoteId });
}

export function mailSendQuote(args: SendQuoteArgs): Promise<SendQuoteResult> {
  return invoke<SendQuoteResult>("mail_send_quote", { args });
}

// ---- Rechtsdokumente / Legal Documents (Block 8) ---------------------------

export function legalDocumentsList(): Promise<LegalDocument[]> {
  return invoke<LegalDocument[]>("legal_documents_list");
}

export function legalDocumentsUpload(
  docType: string,
  title: string | null,
  uploadId: string,
): Promise<LegalDocument> {
  return invoke<LegalDocument>("legal_documents_upload", { docType, title, uploadId });
}

export function legalDocumentsActivate(id: string): Promise<void> {
  return invoke<void>("legal_documents_activate", { id });
}

export function legalDocumentsDeactivate(id: string): Promise<void> {
  return invoke<void>("legal_documents_deactivate", { id });
}

// ---- Dateien (Plan W7) -----------------------------------------------------
// Archivdateien und Downloads holt der Browser direkt (`GET /files/…`, mit
// Sitzungs-Cookie); eigene Rechnungs-/Angebots-PDFs kommen inline, alles andere
// als Anhang.

/** Adresse eines Downloads (Export einmalig, Vorschau mehrfach abrufbar). */
export function downloadUrl(ticket: DownloadTicket): string {
  return `/files/download/${encodeURIComponent(ticket.downloadId)}`;
}

/** Adresse einer Archivdatei (Hash-geprüft, Audit `archive.read`). */
export function archiveUrl(archiveEntryId: string): string {
  return `/files/archive/${encodeURIComponent(archiveEntryId)}`;
}

/** Speichert einen Export im Browser (Download, nicht in den Speicher geladen). */
export function saveDownload(ticket: DownloadTicket): void {
  saveFile(downloadUrl(ticket), ticket.fileName);
}

/** Speichert eine Archivdatei (Anlage, Beleg-Original) im Browser. */
export function saveArchiveFile(archiveEntryId: string, fileName = ""): void {
  saveFile(archiveUrl(archiveEntryId), fileName);
}

// ---- Generischer Anhang-Upload + Liste (Block 9) ---------------------------

export function attachmentsAdd(args: AddAttachmentArgs): Promise<AttachmentView[]> {
  return invoke<AttachmentView[]>("attachments_add", { args });
}

export function attachmentsListFor(
  parentType: string,
  parentId: string,
): Promise<AttachmentView[]> {
  return invoke<AttachmentView[]>("attachments_list", { parentType, parentId });
}

// ---- Zahlungs-Konten (Block 9) ---------------------------------------------

export function paymentAccountsList(includeInactive = false): Promise<PaymentAccount[]> {
  return invoke<PaymentAccount[]>("payment_accounts_list", { includeInactive });
}

export function paymentAccountsCreate(input: PaymentAccountInput): Promise<PaymentAccount> {
  return invoke<PaymentAccount>("payment_accounts_create", { input });
}

export function paymentAccountsUpdate(
  id: string,
  input: PaymentAccountInput,
): Promise<PaymentAccount> {
  return invoke<PaymentAccount>("payment_accounts_update", { id, input });
}

export function paymentAccountsSetActive(id: string, active: boolean): Promise<void> {
  return invoke<void>("payment_accounts_set_active", { id, active });
}

// ---- Kosten / Expenses (Block 9) -------------------------------------------

export function expensesList(filter?: ExpenseListFilter): Promise<ExpenseListItem[]> {
  return invoke<ExpenseListItem[]>("expenses_list", { filter: filter ?? null });
}

export function expensesGet(id: string): Promise<ExpenseDetail | null> {
  return invoke<ExpenseDetail | null>("expenses_get", { id });
}

export function expensesCreate(args: ExpenseCreateArgs): Promise<ExpenseDetail> {
  return invoke<ExpenseDetail>("expenses_create", { args });
}

export function expensesCancel(args: ExpenseCancelArgs): Promise<Expense> {
  return invoke<Expense>("expenses_cancel", { args });
}

export function expensesSetPayment(args: ExpenseSetPaymentArgs): Promise<Expense> {
  return invoke<Expense>("expenses_set_payment", { args });
}

// ---- E-Rechnung-Empfang / Receive (Block 11) -------------------------------

/** Schritt 1: E-Rechnung (XML oder ZUGFeRD-PDF, Upload der Art `erechnung`)
 *  einlesen, parsen, validieren — ohne zu speichern. Liefert einen
 *  editierbaren Kosten-Vorschlag; der Upload bleibt für Schritt 2 liegen. */
export function expensesParseEinvoice(uploadId: string): Promise<EInvoiceParseResult> {
  return invoke<EInvoiceParseResult>("expenses_parse_einvoice", { uploadId });
}

/** Schritt 2: die (ggf. korrigierte) Eingabe als Kosten festschreiben; das
 *  Original wird write-once archiviert. */
export function expensesCreateFromEinvoice(args: EInvoiceCreateArgs): Promise<ExpenseDetail> {
  return invoke<ExpenseDetail>("expenses_create_from_einvoice", { args });
}

/** PV1-A5: Roh-XML einer empfangenen E-Rechnung anzeigen. Liefert `null`, wenn
 *  der Beleg kein E-Rechnungs-Original im Archiv hat. Wirft Domain-Error bei
 *  Tamper-Detection (Hash-Mismatch im Archiv). */
export function expensesReceiptXmlText(
  expenseId: string,
): Promise<XmlViewerPayload | null> {
  return invoke<XmlViewerPayload | null>("expenses_receipt_xml_text", { expenseId });
}

// ---- Privatbewegungen / Private Movements (Block 9) ------------------------

export function privateMovementsList(
  filter?: PrivateMovementListFilter,
): Promise<PrivateMovementListItem[]> {
  return invoke<PrivateMovementListItem[]>("private_movements_list", { filter: filter ?? null });
}

export function privateMovementsGet(id: string): Promise<PrivateMovement | null> {
  return invoke<PrivateMovement | null>("private_movements_get", { id });
}

export function privateMovementsCreate(
  args: PrivateMovementCreateArgs,
): Promise<PrivateMovement> {
  return invoke<PrivateMovement>("private_movements_create", { args });
}

// ---- Wiederkehrende Abos / Recurring (Block 10) ----------------------------

export function recurringList(includeInactive = false): Promise<RecurringSubscription[]> {
  return invoke<RecurringSubscription[]>("recurring_list", { includeInactive });
}

export function recurringGet(id: string): Promise<RecurringSubscription | null> {
  return invoke<RecurringSubscription | null>("recurring_get", { id });
}

export function recurringCreate(input: RecurringInputDto): Promise<RecurringSubscription> {
  return invoke<RecurringSubscription>("recurring_create", { input });
}

export function recurringUpdate(
  id: string,
  input: RecurringInputDto,
): Promise<RecurringSubscription> {
  return invoke<RecurringSubscription>("recurring_update", { id, input });
}

/**
 * Pausieren/Fortsetzen. Beim Fortsetzen eines Auto-Abos mit verpassten Terminen
 * verlangt der Server `resumeMode` (nachholen oder ab heute fortsetzen).
 */
export function recurringSetActive(
  id: string,
  active: boolean,
  resumeMode?: ResumeMode,
): Promise<RecurringSubscription> {
  return invoke<RecurringSubscription>("recurring_set_active", {
    id,
    active,
    resumeMode: resumeMode ?? null,
  });
}

/** Entscheidung zu verpassten Terminen eines aktiven Abos („Rückstand …“). */
export function recurringResolveBacklog(
  id: string,
  mode: ResumeMode,
): Promise<RecurringSubscription> {
  return invoke<RecurringSubscription>("recurring_resolve_backlog", { id, mode });
}

/** „Jetzt erfassen" — legt die Kosten-Position für den aktuellen Stichtag an. */
export function recurringRunNow(id: string): Promise<Expense> {
  return invoke<Expense>("recurring_run_now", { id });
}

/** Manueller Due-Check für alle Auto-Abos (wie der Scheduler-Tick). */
export function recurringRunDueCheck(): Promise<RecurringProcessReport> {
  return invoke<RecurringProcessReport>("recurring_run_due_check");
}

// ---- Wiederkehrende Ausgangsrechnungen / Abo-Rechnungen (Block RI) ----------

export function recurringInvoicesList(includeInactive = false): Promise<RecurringInvoiceRow[]> {
  return invoke<RecurringInvoiceRow[]>("recurring_invoices_list", { includeInactive });
}

export function recurringInvoicesGet(id: string): Promise<RecurringInvoiceDetail | null> {
  return invoke<RecurringInvoiceDetail | null>("recurring_invoices_get", { id });
}

export function recurringInvoicesCreate(
  input: RecurringInvoiceInputDto,
): Promise<RecurringInvoiceDetail> {
  return invoke<RecurringInvoiceDetail>("recurring_invoices_create", { input });
}

export function recurringInvoicesUpdate(
  id: string,
  input: RecurringInvoiceInputDto,
): Promise<RecurringInvoiceDetail> {
  return invoke<RecurringInvoiceDetail>("recurring_invoices_update", { id, input });
}

/**
 * Pausieren/Fortsetzen. Beim Fortsetzen einer Vorlage mit verpassten Terminen
 * verlangt der Server `resumeMode` (nachholen oder ab heute fortsetzen).
 */
export function recurringInvoicesSetActive(
  id: string,
  active: boolean,
  resumeMode?: ResumeMode,
): Promise<RecurringInvoiceRow> {
  return invoke<RecurringInvoiceRow>("recurring_invoices_set_active", {
    id,
    active,
    resumeMode: resumeMode ?? null,
  });
}

/** Entscheidung zu verpassten Terminen einer aktiven Vorlage („Rückstand …“). */
export function recurringInvoicesResolveBacklog(
  id: string,
  mode: ResumeMode,
): Promise<RecurringInvoiceRow> {
  return invoke<RecurringInvoiceRow>("recurring_invoices_resolve_backlog", { id, mode });
}

/** „Jetzt erstellen" — erzeugt die Rechnung für den aktuellen Stichtag. Liefert die Rechnungs-ID. */
export function recurringInvoicesRunNow(id: string): Promise<string> {
  return invoke<string>("recurring_invoices_run_now", { id });
}

/** Manueller Due-Check für alle fälligen Abo-Rechnungen (wie der Scheduler-Tick). */
export function recurringInvoicesRunDueCheck(): Promise<RecurringInvoiceProcessReport> {
  return invoke<RecurringInvoiceProcessReport>("recurring_invoices_run_due_check");
}

// ---- Anlagen / Assets + AfA (Block 12) -------------------------------------

export function assetsList(filter?: AssetListFilter): Promise<AssetListItem[]> {
  return invoke<AssetListItem[]>("assets_list", { filter: filter ?? null });
}

export function assetsGet(id: string): Promise<AssetDetail | null> {
  return invoke<AssetDetail | null>("assets_get", { id });
}

/** Geladene BMF-AfA-Tabelle (Kategorien + GWG-Grenze) fürs Formular. */
export function assetsAfaTable(): Promise<AfaTabellen> {
  return invoke<AfaTabellen>("assets_afa_table");
}

/** AfA-Methoden-Vorschlag (PRD §6.17). */
export function assetsSuggestMethod(
  expenseCategory: string | null,
  acquisitionCostCents: number,
  expenseId: string | null = null,
): Promise<AssetSuggestion> {
  return invoke<AssetSuggestion>("assets_suggest_method", {
    expenseCategory,
    acquisitionCostCents,
    expenseId,
  });
}

export function assetsCreate(input: AssetInputDto): Promise<AssetDetail> {
  return invoke<AssetDetail>("assets_create", { input });
}

export function assetsUpdate(id: string, input: AssetInputDto): Promise<AssetDetail> {
  return invoke<AssetDetail>("assets_update", { id, input });
}

export function assetsDispose(args: AssetDisposeArgs): Promise<AssetDetail> {
  return invoke<AssetDetail>("assets_dispose", { args });
}

/** Vorschau des Abgangs zum Datum (liest nur, Lauf 2026.11 U4): Restbuchwert,
 *  AfA bis zum Abgang und ggf. der Grund, aus dem `assetsDispose` ablehnen würde. */
export function assetsDisposalPreview(
  id: string,
  disposalDate: string,
): Promise<AssetDisposalPreview> {
  return invoke<AssetDisposalPreview>("assets_disposal_preview", { id, disposalDate });
}

/** „AfA für Jahr X jetzt buchen" — bucht bis einschließlich fiscalYear (Catch-up). */
export function depreciationAccrueYear(fiscalYear: number): Promise<AccrueReport> {
  return invoke<AccrueReport>("depreciation_accrue_year", { fiscalYear });
}

/** Setzt die noch nicht festgeschriebene AfA einer Anlage zurück (offenes GJ). */
export function depreciationResetAsset(assetId: string): Promise<AssetDetail> {
  return invoke<AssetDetail>("depreciation_reset_asset", { assetId });
}

export function depreciationListForYear(fiscalYear: number): Promise<DepreciationEntry[]> {
  return invoke<DepreciationEntry[]>("depreciation_list_for_year", { fiscalYear });
}

// ---- EÜR / Steuer-Auswertung (Block 13) ------------------------------------

/** Cash-Basis-EÜR eines Geschäftsjahres (Einnahmen/Ausgaben/AfA/Überschuss). */
export function euerComputeReport(fiscalYear: number): Promise<EuerReport> {
  return invoke<EuerReport>("euer_compute_report", { fiscalYear });
}

/** Geschäftsjahre mit Bewegungen (für den Jahres-Selector); aktuelles Jahr immer dabei. */
export function euerAvailableYears(): Promise<number[]> {
  return invoke<number[]>("euer_available_years");
}

// ---- ELSTER-Ausfüllhilfe + Einzelaufstellung + AVEÜR (Block 14a) -----------

/** Komplettes EÜR-Paket eines GJ (Anlage-EÜR-Zeilen + Einzelaufstellung + AVEÜR). */
export function euerPackage(fiscalYear: number): Promise<EuerPackage> {
  return invoke<EuerPackage>("euer_package", { fiscalYear });
}

/** ELSTER-Ausfüllhilfe (Zeilen-Summen) als CSV zum Herunterladen. */
export function euerExportElster(fiscalYear: number): Promise<ElsterExportResult> {
  return invoke<ElsterExportResult>("euer_export_elster", { fiscalYear });
}

/** Einzelaufstellung (Einnahmen/Ausgaben/AVEÜR) als ZIP zum Herunterladen. */
export function euerExportDetailZip(fiscalYear: number): Promise<EuerDetailExport> {
  return invoke<EuerDetailExport>("euer_export_detail_zip", { fiscalYear });
}

/** Wie viele aktive Anlagen für das GJ noch keine gebuchte AfA haben. */
export function euerAfaPending(fiscalYear: number): Promise<AfaPending> {
  return invoke<AfaPending>("euer_afa_pending", { fiscalYear });
}

/** Erzeugt das vollständige EÜR-PDF (Anlage EÜR + AVEÜR + Einzelaufstellung). */
export function euerExportPdf(fiscalYear: number): Promise<EuerPdfExport> {
  return invoke<EuerPdfExport>("euer_export_pdf", { fiscalYear });
}

/** Erzeugt den DATEV-Buchungsstapel (EXTF) für die Steuerberater-Übergabe.
 *  Berater- und Mandantennummer sind optional (Feld 11/12 des DATEV-Kopfs;
 *  1001–9999999 bzw. 1–99999, sonst lehnt der Server ab — F5, U4). */
export function euerExportDatev(
  fiscalYear: number,
  skr: string,
  numbers: DatevHeaderNumbers = {},
): Promise<DatevExport> {
  return invoke<DatevExport>("euer_export_datev", {
    fiscalYear,
    skr,
    consultantNumber: numbers.consultantNumber ?? null,
    clientNumber: numbers.clientNumber ?? null,
  });
}

/** Erzeugt das komplette Steuerberater-Paket (ZIP); Nummern wie bei
 *  `euerExportDatev` (gehen in den DATEV-Stapel im Paket). */
export function euerExportStbZip(
  fiscalYear: number,
  skr: string,
  numbers: DatevHeaderNumbers = {},
): Promise<StbExport> {
  return invoke<StbExport>("euer_export_stb_zip", {
    fiscalYear,
    skr,
    consultantNumber: numbers.consultantNumber ?? null,
    clientNumber: numbers.clientNumber ?? null,
  });
}

// ---- Backup + Restore (Block 4) --------------------------------------------

/** Off-Site-Ziel (Plan W7, mehrere Ziele; ohne Passwort). */
export interface TargetView {
  id: string;
  label: string;
  kind: "directory" | "sftp";
  /** Verzeichnis-Ziel: Ordner relativ zu `targetsRoot`. */
  directory: string | null;
  host: string | null;
  port: number | null;
  user: string | null;
  remoteDir: string | null;
  hostFingerprint: string | null;
  lastResult: { ok: boolean; at: string; error: string | null } | null;
  /** Jüngste erfolgreiche Kopie auf diesem Ziel (RFC 3339, Europe/Berlin), auch
   *  wenn der letzte Lauf scheiterte; null = noch keine Kopie (Lauf 2026.11 U4). */
  lastCopyAt: string | null;
}

export interface SftpProbe {
  fingerprint: string;
  writeOk: boolean;
  writeError: string | null;
}

export interface BackupSettings {
  passphraseSet: boolean;
  unlocked: boolean;
  /** Lokaler Floor-Ordner (immer geschrieben, G1-BKP.4). */
  floorPath: string;
  /** Wurzel der Verzeichnis-Ziele (`/backup-targets`). */
  targetsRoot: string;
  /** Eingebundene Unterordner der Wurzel. */
  availableDirs: string[];
  targets: TargetView[];
  /** `missing` | `failing` | `stale` (Kopie fehlt oder ist älter als die
   *  Frist der Regel „Backup überfällig“, Lauf F3a) | `ok`. */
  offsiteStatus: "missing" | "failing" | "stale" | "ok";
  /** Stand der Off-Site-Sicherung: älteste der je Ziel jüngsten Kopien
   *  (`targets[].lastCopyAt`, RFC 3339); null ohne Ziel oder solange ein Ziel
   *  keine Kopie hat (Lauf 2026.11 U4). */
  offsiteLastCopyAt: string | null;
  /** Alter davon in Berliner Kalendertagen (0 = heute), wie der Status „stale“. */
  offsiteAgeDays: number | null;
  /** Uhrzeit des Tages-Backups (`HH:MM`, Europe/Berlin; Lauf F). */
  dailyTime: string;
}

export interface BackupOutcome {
  historyId: string;
  filePath: string;
  fileName: string;
  sizeBytes: number;
  retentionTag: string;
  triggerReason: string;
  createdAt: string;
  /** Ergebnis je Off-Site-Ziel (Plan W7). */
  offsite: OffsiteResult[];
}

/** Ergebnis der Spiegelung auf ein Off-Site-Ziel (`backup::OffsiteResult`). */
export interface OffsiteResult {
  targetId: string;
  label: string;
  /** Pfad/URI bei Erfolg. */
  location: string | null;
  /** Fehlertext; bei `suspended` der Grund der Aussetzung. */
  error: string | null;
  /** Spiegelung ausgesetzt (Pause nach einer Wiederherstellung bis zur Freigabe) —
   *  kein Fehlschlag des Ziels; vor `error` prüfen (Lauf 2026.11 U4). */
  suspended: boolean;
}

export interface BackupHistoryItem {
  id: string;
  createdAt: string;
  targetPath: string;
  fileSizeBytes: number;
  retentionTag: string;
  triggerReason: string;
  dbSchemaVersion: number;
  appVersion: string;
  verifiedAt: string | null;
}

/** Ein Eintrag im append-only Backup-Protokoll (`backup_log`, G1-LOG / ADR 0034). */
export interface BackupLogEntry {
  id: string;
  createdAt: string;
  trigger: string;       // "manual" | "auto_daily" | "auto_critical" | "pre_restore"
  targetKind: string;    // "local" | "directory" | "sftp"
  targetLabel: string | null;
  fileName: string;
  fullPath: string;
  sizeBytes: number;
  status: string;        // "ok" | "failed"
  detail: string | null;
}

/** Filter für die Backup-Protokoll-Suche. Alle Felder optional. */
export interface BackupLogFilter {
  search?: string | null;
  dateFrom?: string | null;     // "YYYY-MM-DD", lokale Zeit
  dateTo?: string | null;       // "YYYY-MM-DD", lokale Zeit
  status?: string | null;       // "ok" | "failed"
  trigger?: string | null;      // "manual" | "auto_daily" | "auto_critical" | "pre_restore"
  targetKind?: string | null;   // "local" | "directory" | "sftp"
  limit?: number | null;
}

export interface RestoreReport {
  requiresRestart: boolean;
  /** Dateiname des Sicherheits-Backups vor dem Restore. */
  preRestoreBackup: string;
  /** Quelle (Upload-Dateiname bzw. Backup aus der Historie). */
  source: string;
  stagedAt: string;
  /** Backup-Format (1 = ohne Logo/Unterschrift, 2 = mit). */
  formatVersion: number;
  /** Schema-Version des Backups (ältere werden beim Entsperren migriert). */
  schemaVersion: number;
  /** Logo und Unterschrift werden mit eingespielt. */
  brandingRestored: boolean;
  /** Hinweis für die Anzeige, z. B. bei Format-1-Backups. */
  notice: string | null;
}

export interface ExportReport {
  /** Abruf über `/files/download/{id}` (Plan W7). */
  download: DownloadTicket | null;
  tableCount: number;
  totalRows: number;
  archiveFileCount: number;
  zipSizeBytes: number;
}


export function backupGetSettings(): Promise<BackupSettings> {
  return invoke<BackupSettings>("backup_get_settings");
}

/** Uhrzeit des Tages-Backups setzen (`HH:MM`, Europe/Berlin). */
export function backupSetDailyTime(time: string): Promise<string> {
  return invoke<string>("backup_set_daily_time", { time });
}

export function backupCreateNow(): Promise<BackupOutcome> {
  return invoke<BackupOutcome>("backup_create_now");
}

/** Verzeichnis-Ziel unter `/backup-targets` hinzufügen (Re-Auth). */
export function backupTargetAddDirectory(label: string, directory: string): Promise<TargetView> {
  return invoke<TargetView>("backup_target_add_directory", { label, directory });
}

/** SFTP-Ziel hinzufügen (Re-Auth; Fingerprint aus `backupTestSftp`). */
export function backupTargetAddSftp(args: {
  label: string;
  host: string;
  port: number;
  user: string;
  remoteDir: string;
  hostFingerprint: string;
  password: string;
}): Promise<TargetView> {
  return invoke<TargetView>("backup_target_add_sftp", args);
}

/** Off-Site-Ziel entfernen (Re-Auth; Dateien am Ziel bleiben). */
export function backupTargetRemove(id: string): Promise<void> {
  return invoke<void>("backup_target_remove", { id });
}

/** Testet einen neuen SFTP-Server (Re-Auth, Passwort Pflicht) und liefert den
 *  Host-Fingerprint, den `backupTargetAddSftp` danach übernimmt (Plan W8). */
export function backupTestSftp(
  host: string,
  port: number,
  user: string,
  remoteDir: string,
  password: string,
): Promise<SftpProbe> {
  return invoke<SftpProbe>("backup_test_sftp", { host, port, user, remoteDir, password });
}

/** Testet ein gespeichertes SFTP-Ziel per ID (gespeichertes Passwort, gepinnter Fingerprint). */
export function backupTestSftpSaved(id: string): Promise<SftpProbe> {
  return invoke<SftpProbe>("backup_test_sftp_saved", { id });
}

export function backupList(): Promise<BackupHistoryItem[]> {
  return invoke<BackupHistoryItem[]>("backup_list");
}

/** Backup-Protokoll: jüngste Einträge (neueste zuerst). G1-LOG / ADR 0034. */
export function backupLogList(limit?: number): Promise<BackupLogEntry[]> {
  return invoke<BackupLogEntry[]>("backup_log_list", { limit: limit ?? null });
}

/** Backup-Protokoll: serverseitige Suche/Filterung. G1-LOG / ADR 0034. */
export function backupLogSearch(filter: BackupLogFilter): Promise<BackupLogEntry[]> {
  return invoke<BackupLogEntry[]>("backup_log_search", { filter });
}

/** Quelle eines Restores (Plan W7): hochgeladene `.kbk` oder lokales Backup
 *  aus der Historie. */
export type RestoreSource = { uploadId: string } | { backupId: string };

/** Lädt ein Backup (verschlüsselte `.kbk`) aus dem lokalen Backup-Ordner oder
 *  einem Verzeichnis-Ziel herunter (Re-Auth, einmaliges Ticket; Lauf F3a). */
export function backupDownload(backupId: string): Promise<DownloadTicket> {
  return invoke<DownloadTicket>("backup_download", { backupId });
}

/** Restore Phase A (Job, Re-Auth); danach startet der Server neu. */
export function backupRestoreApply(
  source: RestoreSource,
  passphrase: string,
): Promise<RestoreReport> {
  return invoke<RestoreReport>("backup_restore_apply", { ...source, passphrase });
}

// ---- Migrations-Export (Block 4) -------------------------------------------

/** Migrations-Export (alle Tabellen + Archiv) als ZIP zum Herunterladen. */
export function migrationExportRun(): Promise<ExportReport> {
  return invoke<ExportReport>("migration_export_run", {});
}

// ---- Notifications / Hinweise (Block 15) -----------------------------------

export function notificationsList(includeDismissed = false): Promise<AppNotification[]> {
  return invoke<AppNotification[]>("notifications_list", { includeDismissed });
}

export function notificationsUnreadCount(): Promise<number> {
  return invoke<number>("notifications_unread_count");
}

export function notificationsDismiss(id: string): Promise<void> {
  return invoke<void>("notifications_dismiss", { id });
}

export function notificationsDismissAll(): Promise<number> {
  return invoke<number>("notifications_dismiss_all");
}

export function notificationRulesList(): Promise<NotificationRule[]> {
  return invoke<NotificationRule[]>("notification_rules_list");
}

export function notificationRulesSetEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("notification_rules_set_enabled", { id, enabled });
}

/** Reminder-Regeln + (falls fällig) Integrity-Check jetzt ausführen. */
export function notificationsRunChecks(): Promise<number> {
  return invoke<number>("notifications_run_checks");
}

// ---- Geschäftsjahr-Abschluss / GJ-Lock (Block 15) --------------------------

export function fiscalYearOverview(): Promise<FiscalYearOverview> {
  return invoke<FiscalYearOverview>("fiscal_year_overview");
}

export function fiscalYearClose(year: number): Promise<FiscalYearLock> {
  return invoke<FiscalYearLock>("fiscal_year_close", { year });
}

export function fiscalYearClosedList(): Promise<FiscalYearLock[]> {
  return invoke<FiscalYearLock[]>("fiscal_year_closed_list");
}

export function fiscalYearAutoCloseSet(enabled: boolean): Promise<void> {
  return invoke<void>("fiscal_year_auto_close_set", { enabled });
}

// ---- System / Audit-Trail / Integrität (Block 15) --------------------------

export function auditTrailList(limit?: number): Promise<AuditEntry[]> {
  return invoke<AuditEntry[]>("audit_trail_list", { limit: limit ?? null });
}

export function archiveIntegrityRun(): Promise<IntegrityCheckSummary> {
  return invoke<IntegrityCheckSummary>("archive_integrity_run");
}

export function archiveIntegrityHistory(limit?: number): Promise<IntegrityCheckRow[]> {
  return invoke<IntegrityCheckRow[]>("archive_integrity_history", { limit: limit ?? null });
}

// ---- „Über"-Dialog (Block G2-DOC.3.5) --------------------------------------

/** Statische App-Metadaten (Version, Schema-Version, Build-Commit, Lizenz). */
/** Pause der automatischen Belegerzeugung (Uhrsprung, Restore). */
export function schedulerHoldStatus(): Promise<SchedulerHoldStatus> {
  return invoke<SchedulerHoldStatus>("scheduler_hold_status");
}

/** Pause aufheben (Re-Auth). */
export function schedulerReleaseHold(): Promise<SchedulerHoldStatus> {
  return invoke<SchedulerHoldStatus>("scheduler_release_hold");
}

/** Nummernkreis (zuletzt vergebene Nummer je Art und Jahr; Lauf F, D-24). */
export interface NumberingCounter {
  docType: string;
  label: string;
  fiscalYear: number;
  lastValue: number;
  lastNumber: string | null;
}

export function numberingCountersList(): Promise<NumberingCounter[]> {
  return invoke<NumberingCounter[]>("numbering_counters_list");
}

/** Nummernkreis anheben (nur aufwärts, Re-Auth, Begründung im Audit-Log). */
export function numberingRaise(args: {
  docType: string;
  fiscalYear: number;
  lastValue: number;
  reason: string;
}): Promise<NumberingCounter[]> {
  return invoke<NumberingCounter[]>("numbering_raise", args);
}

/** Wiederhergestellter Entwurf, dessen Nummer vor dem Restore schon festgeschrieben war (F1d). */
export interface RestoredDraftConflict {
  /** `invoice` (auch Storno) oder `quote`. */
  kind: "invoice" | "quote";
  id: string;
  number: string;
}

export function numberingRestoredDraftsList(): Promise<RestoredDraftConflict[]> {
  return invoke<RestoredDraftConflict[]>("numbering_restored_drafts_list");
}

/** Abgleich mit dem versandten Original bestätigen (Re-Auth, Begründung im Audit-Log). */
export function numberingRestoredDraftConfirm(args: {
  kind: "invoice" | "quote";
  id: string;
  reason: string;
}): Promise<RestoredDraftConflict[]> {
  return invoke<RestoredDraftConflict[]>("numbering_restored_draft_confirm", args);
}

/** Reihen, die nach einer Wiederherstellung auf „Rückstand …“ warten (F1d). */
export interface RestoreReviewList {
  templates: string[];
  subscriptions: string[];
}

export function recurringRestoreReviewList(): Promise<RestoreReviewList> {
  return invoke<RestoreReviewList>("recurring_restore_review_list");
}

export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
