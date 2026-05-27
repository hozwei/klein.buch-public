// Tauri-Command-Bridge. Frontend ruft Backend via `invoke<T>(name, args)`.
// Block 2: contacts_* + seller_profile_* + paragraph_19_info.
// Block 3b: invoices_* — Draft, Lock-and-Issue, Cancel, Payment.

import { invoke } from "@tauri-apps/api/core";
import type {
  AnonymizeCheck,
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
  OauthStatus,
  EmailLogEntry,
  EmailLogFilter,
  Paragraph19Info,
  PdfTemplateMeta,
  TravelSettings,
  TravelLine,
  DropFolderSettings,
  DropFolderSyncResult,
  RecordPaymentArgs,
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
  AfaTabellen,
  MethodSuggestion,
  DepreciationEntry,
  AccrueReport,
  EuerReport,
  ElsterExportResult,
  EuerPackage,
  EuerDetailExport,
  EuerPdfExport,
  DatevExport,
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
  zipPath: string;
  fileName: string;
  invoiceCount: number;
  quoteCount: number;
  expenseCount: number;
  documentCount: number;
  bundledDocumentCount: number;
  emailCount: number;
}

/** Erzeugt die DSGVO-Auskunft (Art. 15) zu einem Kontakt als ZIP (PDF + JSON +
 *  Originaldateien) nach data/export/dsgvo und öffnet den Ordner. Read-only. */
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

export function sellerLogoSet(fileBytes: number[], fileName: string): Promise<SellerProfile> {
  return invoke<SellerProfile>("seller_logo_set", { fileBytes, fileName });
}

export function sellerLogoClear(): Promise<SellerProfile> {
  return invoke<SellerProfile>("seller_logo_clear");
}

export function sellerLogoData(): Promise<string | null> {
  return invoke<string | null>("seller_logo_data");
}

// Angebots-Unterschrift (Bild-Upload wie Logo) + globaler Toggle
export function sellerSignatureSet(fileBytes: number[], fileName: string): Promise<string | null> {
  return invoke<string | null>("seller_signature_set", { fileBytes, fileName });
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

export function pdfTemplatePreview(name: string): Promise<void> {
  return invoke<void>("pdf_template_preview", { name });
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

// ---- Drop-Folder fuer eingehende E-Rechnungen (Block PV1-DROP) -------------

export function dropFolderSettingsGet(): Promise<DropFolderSettings> {
  return invoke<DropFolderSettings>("drop_folder_settings_get");
}

export function dropFolderSettingsSet(
  args: DropFolderSettings,
): Promise<DropFolderSettings> {
  return invoke<DropFolderSettings>("drop_folder_settings_set", { args });
}

/** Manueller Trigger fuer den Drop-Folder-Sync (gegen den 5-min-Tick). */
export function dropFolderSyncNow(): Promise<DropFolderSyncResult> {
  return invoke<DropFolderSyncResult>("drop_folder_sync_now");
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

export function invoicesValidateDraft(id: string): Promise<ValidationIssueDto[]> {
  return invoke<ValidationIssueDto[]>("invoices_validate_draft", { id });
}

export function invoicesLockAndIssue(
  id: string,
  buyerReference: string | null = null,
): Promise<LockResponse> {
  return invoke<LockResponse>("invoices_lock_and_issue", { id, buyerReference });
}

export function invoicesRecordPayment(args: RecordPaymentArgs): Promise<Invoice> {
  return invoke<Invoice>("invoices_record_payment", { args });
}

export function invoicesCancel(args: CancelArgs): Promise<CancelResponse> {
  return invoke<CancelResponse>("invoices_cancel", { args });
}

export function invoicesOpenPdf(id: string): Promise<void> {
  return invoke<void>("invoices_open_pdf", { id });
}

export function invoicesRevealPdf(id: string): Promise<void> {
  return invoke<void>("invoices_reveal_pdf", { id });
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

export function mailSendTest(accountId: string, to: string): Promise<void> {
  return invoke<void>("mail_send_test", { accountId, to });
}

// ---- Mail OAuth (Block 16) -------------------------------------------------

export function mailOauthStatus(accountId: string): Promise<OauthStatus> {
  return invoke<OauthStatus>("mail_oauth_status", { accountId });
}

/** Öffnet den Browser zur Microsoft-Anmeldung und wartet auf den Abschluss. */
export function mailOauthConnect(accountId: string): Promise<OauthStatus> {
  return invoke<OauthStatus>("mail_oauth_connect", { accountId });
}

export function mailOauthDisconnect(accountId: string): Promise<void> {
  return invoke<void>("mail_oauth_disconnect", { accountId });
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

export function quotesAttachmentsList(quoteId: string): Promise<AttachmentView[]> {
  return invoke<AttachmentView[]>("quotes_attachments_list", { quoteId });
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
): Promise<void> {
  return invoke<void>("package_preview", { title, bodyMarkup, defaultUnitPriceCents });
}

// ---- Paket → Beleg-Position (Block P3) --------------------------------------

/** Materialisiert eine Beleg-Position aus der aktiven Revision eines Pakets.
 *  Reine Aufbereitung (schreibt nichts) — das Frontend hängt sie an die
 *  Positions-Liste. */
export function packageMaterializeItem(packageId: string): Promise<MaterializedPackageItem> {
  return invoke<MaterializedPackageItem>("package_materialize_item", { packageId });
}

// ---- Paket-Katalog-Broschüre (Block P4) -------------------------------------

/** Rendert die Broschüre für die gewählten Pakete und öffnet sie (Druck/Vorschau).
 *  Kein §14-Beleg — kein Nummernkreis, kein Archiv. */
export function packageCatalogRender(packageIds: string[]): Promise<void> {
  return invoke<void>("package_catalog_render", { packageIds });
}

/** Versendet die Broschüre als PDF-Anhang an einen Kontakt. Protokolliert im
 *  append-only email_log (related_kind = 'package_catalog'). */
export function packageCatalogSend(
  args: SendPackageCatalogArgs,
): Promise<SendPackageCatalogResult> {
  return invoke<SendPackageCatalogResult>("package_catalog_send", { args });
}

// ---- Angebots-PDF + Bundle + Versand (Block 8) -----------------------------

/** Erzeugt (falls nötig) das Angebots-PDF; das Frontend öffnet es danach via
 *  attachmentsOpen(quote.pdfArchiveId). */
export function quotesGeneratePdf(id: string): Promise<Quote> {
  return invoke<Quote>("quotes_generate_pdf", { id });
}

/** Erzeugt das zusammengeführte Bundle-PDF (Angebot + AGB + Datenschutz) und
 *  öffnet es im PDF-Viewer. Bindet die Legal-Versionen (Pflicht). */
export function quotesOpenBundle(id: string): Promise<void> {
  return invoke<void>("quotes_open_bundle", { id });
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
  fileBytes: number[],
  fileName: string,
): Promise<LegalDocument> {
  return invoke<LegalDocument>("legal_documents_upload", { docType, title, fileBytes, fileName });
}

export function legalDocumentsActivate(id: string): Promise<void> {
  return invoke<void>("legal_documents_activate", { id });
}

export function legalDocumentsDeactivate(id: string): Promise<void> {
  return invoke<void>("legal_documents_deactivate", { id });
}

// ---- Attachments (Block 6) -------------------------------------------------

export function attachmentsOpen(archiveEntryId: string): Promise<void> {
  return invoke<void>("attachments_open", { archiveEntryId });
}

export function attachmentsReveal(archiveEntryId: string): Promise<void> {
  return invoke<void>("attachments_reveal", { archiveEntryId });
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

export function paymentAccountsEnsureDefaults(): Promise<PaymentAccount[]> {
  return invoke<PaymentAccount[]>("payment_accounts_ensure_defaults");
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

/** Schritt 1: E-Rechnung (XML oder ZUGFeRD-PDF) einlesen, parsen, validieren —
 *  ohne zu speichern. Liefert einen editierbaren Kosten-Vorschlag. */
export function expensesParseEinvoice(
  fileBytes: number[],
  fileName: string,
): Promise<EInvoiceParseResult> {
  return invoke<EInvoiceParseResult>("expenses_parse_einvoice", { fileBytes, fileName });
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

export function recurringSetActive(id: string, active: boolean): Promise<RecurringSubscription> {
  return invoke<RecurringSubscription>("recurring_set_active", { id, active });
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

export function recurringInvoicesSetActive(
  id: string,
  active: boolean,
): Promise<RecurringInvoiceRow> {
  return invoke<RecurringInvoiceRow>("recurring_invoices_set_active", { id, active });
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
): Promise<MethodSuggestion> {
  return invoke<MethodSuggestion>("assets_suggest_method", {
    expenseCategory,
    acquisitionCostCents,
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

/** Schreibt die ELSTER-Ausfüllhilfe (Zeilen-Summen) als CSV an den gewählten Pfad. */
export function euerExportElster(
  fiscalYear: number,
  targetPath: string,
): Promise<ElsterExportResult> {
  return invoke<ElsterExportResult>("euer_export_elster", { fiscalYear, targetPath });
}

/** Schreibt die Einzelaufstellung (Einnahmen/Ausgaben/AVEÜR) als ZIP. */
export function euerExportDetailZip(
  fiscalYear: number,
  targetPath: string,
): Promise<EuerDetailExport> {
  return invoke<EuerDetailExport>("euer_export_detail_zip", { fiscalYear, targetPath });
}

/** Zeigt eine exportierte Datei im Datei-Explorer/Finder (markiert sie). */
export function euerRevealPath(path: string): Promise<void> {
  return invoke<void>("euer_reveal_path", { path });
}

/** Wie viele aktive Anlagen für das GJ noch keine gebuchte AfA haben. */
export function euerAfaPending(fiscalYear: number): Promise<AfaPending> {
  return invoke<AfaPending>("euer_afa_pending", { fiscalYear });
}

/** Erzeugt das vollständige EÜR-PDF (Anlage EÜR + AVEÜR + Einzelaufstellung). */
export function euerExportPdf(
  fiscalYear: number,
  targetPath: string,
): Promise<EuerPdfExport> {
  return invoke<EuerPdfExport>("euer_export_pdf", { fiscalYear, targetPath });
}

/** Erzeugt den DATEV-Buchungsstapel (EXTF) für die Steuerberater-Übergabe. */
export function euerExportDatev(
  fiscalYear: number,
  skr: string,
  targetPath: string,
): Promise<DatevExport> {
  return invoke<DatevExport>("euer_export_datev", { fiscalYear, skr, targetPath });
}

/** Erzeugt das komplette Steuerberater-Paket (ZIP). */
export function euerExportStbZip(
  fiscalYear: number,
  skr: string,
  targetPath: string,
): Promise<StbExport> {
  return invoke<StbExport>("euer_export_stb_zip", { fiscalYear, skr, targetPath });
}

// ---- Backup + Restore (Block 4) --------------------------------------------

export interface DetectedTarget {
  label: string;
  path: string;
}

export interface SftpTargetView {
  host: string;
  port: number;
  user: string;
  remoteDir: string;
  hostFingerprint: string | null;
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
  /** Off-Site-Ziel (Verzeichnis), falls konfiguriert. */
  targetPath: string | null;
  defaultSuggestion: string;
  detectedTargets: DetectedTarget[];
  sftp: SftpTargetView | null;
}

export interface BackupOutcome {
  historyId: string;
  filePath: string;
  fileName: string;
  sizeBytes: number;
  retentionTag: string;
  triggerReason: string;
  createdAt: string;
  /** Off-Site-Pfad/URI bei erfolgreicher Spiegelung (sonst null). */
  mirrorTarget: string | null;
  /** Fehlertext, falls die Off-Site-Spiegelung fehlschlug (sonst null). */
  mirrorError: string | null;
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

export interface RestorePreview {
  filePath: string;
  createdAt: string;
  appVersion: string;
  formatVersion: number;
  schemaVersion: number;
  currentSchemaVersion: number;
  compatible: boolean;
  contentSizeBytes: number;
}

export interface RestoreReport {
  requiresRestart: boolean;
  preRestoreBackupPath: string;
  sourceFile: string;
  stagedAt: string;
}

export interface ExportReport {
  zipPath: string;
  tableCount: number;
  totalRows: number;
  archiveFileCount: number;
  zipSizeBytes: number;
}

export function backupNeedsOnboarding(): Promise<boolean> {
  return invoke<boolean>("backup_needs_onboarding");
}

export function backupIsUnlocked(): Promise<boolean> {
  return invoke<boolean>("backup_is_unlocked");
}

export function backupGetSettings(): Promise<BackupSettings> {
  return invoke<BackupSettings>("backup_get_settings");
}

export function backupSetupPassphrase(passphrase: string): Promise<BackupOutcome> {
  return invoke<BackupOutcome>("backup_setup_passphrase", { passphrase });
}

export function backupUnlock(passphrase: string): Promise<boolean> {
  return invoke<boolean>("backup_unlock", { passphrase });
}

export function backupCreateNow(): Promise<BackupOutcome> {
  return invoke<BackupOutcome>("backup_create_now");
}

export function backupSetTarget(path: string): Promise<void> {
  return invoke<void>("backup_set_target", { path });
}

export function backupTestSftp(
  host: string,
  port: number,
  user: string,
  remoteDir: string,
  password: string,
): Promise<SftpProbe> {
  return invoke<SftpProbe>("backup_test_sftp", { host, port, user, remoteDir, password });
}

export function backupSetSftpTarget(
  host: string,
  port: number,
  user: string,
  remoteDir: string,
  hostFingerprint: string,
  password: string,
): Promise<void> {
  return invoke<void>("backup_set_sftp_target", {
    host,
    port,
    user,
    remoteDir,
    hostFingerprint,
    password,
  });
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

/** Öffnet einen lokalen Backup-Ordner (Floor/Off-Site-Verzeichnis) im Explorer. */
export function backupOpenFolder(path: string): Promise<void> {
  return invoke<void>("backup_open_folder", { path });
}

/** Zeigt eine lokale Backup-Datei im enthaltenden Ordner (markiert sie). */
export function backupRevealPath(path: string): Promise<void> {
  return invoke<void>("backup_reveal_path", { path });
}

export function backupRestorePreview(path: string): Promise<RestorePreview> {
  return invoke<RestorePreview>("backup_restore_preview", { path });
}

export function backupRestoreApply(path: string, passphrase: string): Promise<RestoreReport> {
  return invoke<RestoreReport>("backup_restore_apply", { path, passphrase });
}

// ---- Migrations-Export (Block 4) -------------------------------------------

export function migrationExportRun(targetPath: string): Promise<ExportReport> {
  return invoke<ExportReport>("migration_export_run", { targetPath });
}

// ---- Factory Reset (G1-RESET, ADR 0036) ------------------------------------

/** Pre-Flight-Status fürs Reset-UI. */
export interface FactoryResetCheck {
  /** Anzahl festgeschriebener Belege; > 0 ⇒ Export oder Quittung Pflicht. */
  lockedDocuments: number;
  /** Ist ein Off-Site-Ziel konfiguriert (bleibt bestehen)? */
  hasOffSiteTarget: boolean;
  /** Off-Site-Label (Pfad bzw. sftp://…), falls vorhanden. */
  offSiteLabel: string | null;
  /** Exaktes Tipp-Bestätigungswort. */
  confirmWord: string;
  /** Exakter Wortlaut der Aufbewahrungs-Quittung. */
  retentionReceiptText: string;
}

/** Liest Belege-Zählung, Off-Site-Hinweis und die exakten Bestätigungstexte. */
export function factoryResetCheck(): Promise<FactoryResetCheck> {
  return invoke<FactoryResetCheck>("factory_reset_check");
}

/**
 * Führt den Factory Reset aus: merkt ihn vor und startet die App neu (der Nuke
 * läuft beim Neustart vor dem Pool-Open). Auf dem Erfolgspfad kehrt der Aufruf
 * nicht zurück (die App startet neu); nur Fehler resolven/rejecten.
 */
export function factoryReset(args: {
  passphrase: string;
  confirmWord: string;
  exportConfirmed: boolean;
  retentionReceipt: string;
}): Promise<void> {
  return invoke<void>("factory_reset", args);
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

export function fiscalYearAutoCloseGet(): Promise<boolean> {
  return invoke<boolean>("fiscal_year_auto_close_get");
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
export function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

/**
 * Bundle-Pfad zur generierten Drittanbieter-Lizenz-Übersicht. Im Release
 * stehen dort cargo-about- + pnpm-licenses-Output, im Dev-Build der Stub.
 * Falls die Datei fehlt, wirft das Backend eine sprechende Fehlermeldung.
 */
export function thirdPartyLicensesPath(): Promise<string> {
  return invoke<string>("third_party_licenses_path");
}
