<script lang="ts">
  // Rechnungs-Editor für „Neue Rechnung“ (`/invoices/new`) und „Entwurf bearbeiten“
  // (`/invoices/[id]/edit`, U4 Warteliste (6)). Ein gemeinsamer Baustein der beiden
  // Routen; `editId` = gespeicherter Entwurf, `null` = neue Rechnung. Die Nummer
  // eines Entwurfs bleibt beim Bearbeiten.
  //
  // U4: Ablehnungen des Servers stehen am Feld (Rechnungsdatum, Empfänger) bzw. als
  // stehendes Banner über dem Formular (mehrere Gründe, ADR 0041 Nr. 22) statt als
  // Toast. Client-Prüfungen bilden nur Server-Regeln nach (Geschäftsjahr der
  // Nummer, Empfänger eines Entwurfs aus einem Angebot).
  //
  // ADR 0042 Nr. 10–12: Leistungsdatum oder Leistungszeitraum (ServiceDateFields),
  // Vorausrechnung erlaubt (nur Hinweis), Leitweg-ID am Entwurf.
  import { todayBerlin, addDaysIso, yearOfIsoDate } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Badge from "#lib/Badge.svelte";
  import { goto } from "$app/navigation";
  import {
    contactsList,
    invoicesCreateDraft,
    invoicesGet,
    invoicesUpdateDraft,
    pdfTemplatesList,
    sellerProfileGet,
    documentTermsGet,
  } from "#lib/api.ts";
  import type {
    Contact,
    CreateDraftArgs,
    InvoiceItemInput,
    PdfTemplateMeta,
    SellerProfile,
    TaxCategoryCode,
    TravelLine,
    MaterializedPackageItem,
    RestoreConflictState,
  } from "#lib/types.ts";
  import { date, euro } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { applyTaxRules, documentTotals, REGEL_TAX_CODES, roundHalfAwayFromZero } from "#lib/tax.ts";
  import { pdfTemplateLabel, pdfTemplateDefaultOptionLabel, unitCodeError, unitLabel, unitOptions } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import ContactSelect from "#lib/ContactSelect.svelte";
  import TravelLineAdder from "#lib/TravelLineAdder.svelte";
  import PackageItemAdder from "#lib/PackageItemAdder.svelte";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import TaxStatusNotice from "#lib/TaxStatusNotice.svelte";
  import { invoiceTaxDate } from "#lib/taxStatus.ts";
  import { taxStatusAtDate } from "#lib/taxStatusAt.svelte.ts";
  import ServiceDateFields from "#lib/ServiceDateFields.svelte";
  import BuyerReferenceField from "#lib/BuyerReferenceField.svelte";
  import RestoreConflictBanner from "#lib/RestoreConflictBanner.svelte";
  import { restoreSaveFlash } from "#lib/restoreConflict.ts";
  import {
    buyerReferenceError,
    normalizeBuyerReference,
    serviceDateInvalid,
    serviceDateModeOf,
    serviceDatePayload,
    type ServiceDateMode,
  } from "#lib/serviceDate.ts";

  let { editId = null }: { editId?: string | null } = $props();

  // Form-Position = InvoiceItemInput, aber descriptionMarkup als String (für den
  // bind:value-Editor) + transienter Paket-Name fürs Badge (Backend ignoriert ihn).
  // Menge, Preis und Satz sind `null`, solange das Feld leer oder ungültig ist
  // (MoneyInput/QuantityInput, U1); beim Speichern geprüft.
  type FormItem = Omit<InvoiceItemInput, "quantity" | "unitPriceCents" | "taxRatePercent"> & {
    quantity: number | null;
    unitPriceCents: number | null;
    taxRatePercent: number | null;
    descriptionMarkup: string;
    descriptionTitle: string;
    packageName?: string;
  };

  // Rich-Position (aus Paket oder „angepasst"): hat Titel/Markup → wird mit
  // Titel-Feld + Markdown-Editor bearbeitet statt einer schmalen Beschreibung.
  function isRich(it: FormItem): boolean {
    return !!(it.descriptionTitle || it.descriptionMarkup);
  }
  function labelMissing(it: FormItem): boolean {
    return isRich(it) ? !(it.descriptionTitle ?? "").trim() : !it.description.trim();
  }
  // Eine leere, unangetastete Custom-Zeile (z. B. die Default-Position 1).
  function isEmptyItem(it: FormItem): boolean {
    return !isRich(it) && !it.description.trim() && !it.unitPriceCents;
  }
  function lineNet(it: FormItem): number {
    return roundHalfAwayFromZero((it.quantity ?? 0) * (it.unitPriceCents ?? 0));
  }
  function toInput(it: FormItem): InvoiceItemInput {
    return {
      ...it,
      quantity: it.quantity ?? 0,
      unitPriceCents: it.unitPriceCents ?? 0,
      taxRatePercent: it.taxRatePercent ?? 0,
    };
  }

  // ---- State ----
  let seller = $state<SellerProfile | null>(null);
  let contacts = $state<Contact[]>([]);
  let templates = $state<PdfTemplateMeta[]>([]);
  let loading = $state(true);
  // U4: Entwurf zum Bearbeiten nicht gefunden → eigener Zustand statt Fehler-Toast über leerem Formular.
  let notFound = $state(false);
  let saving = $state(false);
  // Nach dem ersten Speicher-Versuch zeigen alle Felder ihre Meldungen.
  let submitted = $state(false);
  // Entwurf: Nummer, Geschäftsjahr der Nummer, Herkunft aus einem Angebot.
  let editNumber = $state<string | null>(null);
  let draftFiscalYear = $state<number | null>(null);
  let fromQuote = $state(false);
  // Warteliste (8), ADR 0042 Nr. 16: Restore-Vormerkung aus dem Lesefeld des
  // Entwurfs — `flagged` (vor dem Ausstellen abgleichen) oder `confirmed`
  // (Speichern merkt ihn wieder vor; Hinweis schon vor dem Speichern).
  let restoreConflict = $state<RestoreConflictState | null>(null);

  // Ablehnung des Servers: Gründe als Banner, einzelne Gründe am Feld (nur,
  // solange das Feld den abgelehnten Wert hat).
  let serverReasons = $state<string[]>([]);
  let serverDateErr = $state<{ value: string; msg: string } | null>(null);
  let serverContactErr = $state<{ value: string; msg: string } | null>(null);
  let serverBannerEl: HTMLDivElement | undefined = $state();

  // Heute (Europe/Berlin) — Obergrenze für das Rechnungsdatum (domain::invoice).
  // Das Leistungsdatum darf danach liegen (Vorausrechnung, ADR 0042 Nr. 11).
  const today = todayBerlin();
  let contactId = $state("");
  let invoiceDate = $state(today);
  // U1: Leistungsdatum (§14 Abs. 4 Nr. 6 UStG) mit dem Rechnungsdatum
  // vorbelegt — sichtbar und änderbar; es folgt dem Rechnungsdatum, bis es von
  // Hand geändert wird (Angebote tragen kein eigenes Leistungsdatum).
  // ADR 0042 Nr. 10: alternativ ein Leistungszeitraum von–bis.
  let serviceMode = $state<ServiceDateMode>("date");
  let deliveryDate = $state(today);
  let deliveryDateTouched = $state(false);
  $effect(() => {
    if (!deliveryDateTouched) deliveryDate = invoiceDate;
  });
  let periodStart = $state("");
  let periodEnd = $state("");
  // ADR 0042 Nr. 12: Leitweg-ID am Entwurf (revidiert ADR 0041 Nr. 91).
  let buyerReference = $state("");
  let dueDate = $state("");
  // V1-04: Klein.Buch rechnet nur in Euro (domain::invoice, CurrencyUnsupported) —
  // kein Freitextfeld mehr. Ein älterer Entwurf mit anderer Währung wird beim
  // Speichern auf EUR gesetzt (Hinweis am Feld).
  let currency = $state("EUR");
  let pdfTemplate = $state("default");
  let notes = $state("");

  // Bezahlt-Hinweis: reiner PDF-Text je Rechnung. KEINE EÜR-Buchung — die
  // Zahlung wird weiterhin separat über „+ Zahlung“ auf der Rechnungsseite gebucht.
  let alreadyPaid = $state(false);
  let paymentNote = $state("");
  const todayDe = date(today);
  const paymentTemplates = [
    `Betrag dankend bar erhalten am ${todayDe}`,
    "Rechnungsbetrag bereits in bar beglichen",
    `Bezahlt am ${todayDe} in bar`,
    `Teilzahlung in bar bereits am ${todayDe}`,
  ];

  // PDF-Vorlage als Auswahl (KB-0050). „default“ ist der Platzhalter für die
  // Standard-Vorlage aus den Einstellungen (Server: commands::invoices).
  let templateOptions = $derived(templates.filter((t) => t.name !== "default").map((t) => t.name));
  // Warteliste (11): Stellt die gewählte Vorlage Stornos dar? Sonst entsteht ein
  // Storno mit der Ersatzvorlage (`stornoFallback`, wie die Rechnungs-Pipeline).
  let effectiveTemplate = $derived(
    pdfTemplate === "default" ? seller?.defaultPdfTemplate || "default" : pdfTemplate,
  );
  let templateMeta = $derived(templates.find((t) => t.name === effectiveTemplate) ?? null);
  let stornoHint = $derived(
    templateMeta && !templateMeta.supportsStorno
      ? `Die Vorlage „${pdfTemplateLabel(templateMeta.name)}“ stellt keine Storno-Belege dar. ` +
          `Wird die Rechnung storniert, entsteht der Storno-Beleg mit der Vorlage „${pdfTemplateLabel(templateMeta.stornoFallback ?? "klassisch")}“.`
      : "",
  );

  let items = $state<FormItem[]>([
    emptyItem(1),
  ]);

  function emptyItem(position: number): FormItem {
    return {
      position,
      description: "",
      quantity: 1,
      unitCode: "C62",
      // V4-05: leer statt 0,00 — so meldet das Feld einen vergessenen Preis selbst.
      unitPriceCents: null,
      taxRatePercent: 0,
      taxCategoryCode: "E" as TaxCategoryCode,
      descriptionMarkup: "",
      descriptionTitle: "",
    };
  }

  // ---- §19-Hardline: bei is_kleinunternehmer USt-Felder sperren ----
  // ADR 0042 Nr. 3: maßgeblich ist der Status am Leistungsdatum (sonst
  // Rechnungsdatum), nicht der von heute. Bis er für das aktuelle Datum geladen
  // ist, zeigt die Seite den zuletzt bekannten Wert.
  // Bei einem Zeitraum gilt dessen Ende (ADR 0042 Nr. 10).
  let serviceFields = $derived(serviceDatePayload(serviceMode, deliveryDate, periodStart, periodEnd));
  let taxDate = $derived(invoiceTaxDate({ ...serviceFields, invoiceDate }, today));
  const taxAt = taxStatusAtDate(() => taxDate);
  let todayKlein = $derived(seller ? seller.isKleinunternehmer === 1 : null);
  let isKlein = $derived(taxAt.isKlein ?? todayKlein === true);
  // §19: E/0 erzwingen; Regelbesteuerung: E wird S 19 % (KB-0014). In place
  // (lib/tax.ts) — eine neue Array-Identität triggerte den Effect endlos. Nur mit
  // dem Status genau dieses Datums, sonst schriebe ein veralteter Wert die Sätze
  // eines geladenen Entwurfs um.
  $effect(() => {
    applyTaxRules(
      items as unknown as InvoiceItemInput[],
      seller && taxAt.date === taxDate ? taxAt.isKlein : null,
    );
  });

  // ---- Totals (live, wie der Server: USt je Steuergruppe) ----
  let totals = $derived(documentTotals(items.map(toInput)));

  // ---- Feldfehler (spiegeln domain::invoice und update_draft; der Server prüft erneut) ----
  let errContact = $derived(contactId ? "" : "Bitte einen Empfänger wählen.");
  let errInvoiceDate = $derived(
    !invoiceDate
      ? "Bitte das Rechnungsdatum angeben."
      : invoiceDate > today
        ? "Das Rechnungsdatum darf nicht in der Zukunft liegen."
        : draftFiscalYear !== null && yearOfIsoDate(invoiceDate) !== draftFiscalYear
          ? `Das Rechnungsdatum muss im Geschäftsjahr ${draftFiscalYear} bleiben — die Nummer ${editNumber ?? ""} gehört zu dessen Nummernkreis. Für ein anderes Jahr den Entwurf verwerfen und neu anlegen.`
          : "",
  );
  // Leistungsdatum bzw. -zeitraum: Meldungen stehen am Feld (ServiceDateFields).
  let errServiceDate = $derived(serviceDateInvalid(serviceMode, deliveryDate, periodStart, periodEnd));
  let errBuyerReference = $derived(buyerReferenceError(buyerReference));
  // V1-04: Fälligkeit ist Pflicht für die E-Rechnung (domain::invoice, XRechnungDueDateMissing
  // bei offenem Betrag) — Meldung schon beim Speichern statt erst beim Ausstellen.
  let errDueDate = $derived(
    !dueDate
      ? "Bitte eine Fälligkeit angeben — Pflicht für die E-Rechnung."
      : invoiceDate && dueDate < invoiceDate
        ? "Die Fälligkeit darf nicht vor dem Rechnungsdatum liegen."
        : "",
  );
  // Angezeigte Meldungen: eigene Prüfung zuerst, sonst die Ablehnung des Servers.
  let shownInvoiceDateErr = $derived(
    (submitted && errInvoiceDate) ||
      (serverDateErr && serverDateErr.value === invoiceDate ? serverDateErr.msg : ""),
  );
  let shownContactErr = $derived(
    serverContactErr && serverContactErr.value === contactId ? serverContactErr.msg : undefined,
  );
  function itemInvalid(it: FormItem): boolean {
    return (
      labelMissing(it) ||
      it.quantity == null ||
      it.unitPriceCents == null ||
      (!isKlein && it.taxRatePercent == null) ||
      !!unitCodeError(it.unitCode)
    );
  }

  // V4-05: Ein Gesamtbetrag von 0 € lehnt der Server ab (domain::invoice, TotalNotPositive).
  // Die Meldung steht nach dem ersten Speichern an den Preisfeldern mit 0 €, nicht als Toast.
  const ZERO_TOTAL_MSG = "Bitte einen Einzelpreis eingeben — der Gesamtbetrag muss über 0 € liegen.";
  let zeroTotal = $derived(submitted && totals.gross <= 0);
  function priceError(it: FormItem): string | undefined {
    return zeroTotal && it.unitPriceCents === 0 ? ZERO_TOTAL_MSG : undefined;
  }

  onMount(async () => {
    loading = true;
    try {
      [seller, contacts, templates] = await Promise.all([
        sellerProfileGet(),
        contactsList(false),
        // Ohne Liste bleibt die Auswahl bei „Standard aus den Einstellungen“.
        pdfTemplatesList().catch(() => [] as PdfTemplateMeta[]),
      ]);
      if (editId) {
        await loadDraft(editId);
      } else {
        const terms = await documentTermsGet();
        dueDate = addDays(invoiceDate, terms.invoiceDueDays);
      }
      // Status am Belegdatum vor dem ersten Zeichnen (kein Springen der Spalten).
      await taxAt.load(taxDate).catch(() => {});
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loading = false;
    }
  });

  // Übernimmt einen gespeicherten Entwurf ins Formular. Nur offene Entwürfe
  // (keine Stornos, nicht festgeschrieben, nicht verworfen) — sonst zurück zur
  // Detailseite; der Server prüft dasselbe noch einmal.
  async function loadDraft(id: string) {
    const d = await invoicesGet(id);
    if (!d) {
      notFound = true;
      return;
    }
    const inv = d.invoice;
    if (inv.lockedAt || inv.voidedAt || inv.isStornoFor || inv.status !== "draft") {
      flash("Nur offene Rechnungs-Entwürfe lassen sich bearbeiten.", "error");
      goto(`/invoices/${id}`, { replaceState: true });
      return;
    }
    editNumber = inv.invoiceNumber;
    draftFiscalYear = inv.fiscalYear;
    fromQuote = !!inv.derivedFromQuoteId;
    contactId = inv.contactId;
    if (!contacts.some((c) => c.id === inv.contactId) && d.buyer) contacts = [...contacts, d.buyer];
    deliveryDateTouched = true; // gespeicherten Wert nicht überschreiben
    invoiceDate = inv.invoiceDate.slice(0, 10);
    deliveryDate = inv.deliveryDate?.slice(0, 10) ?? "";
    serviceMode = serviceDateModeOf(inv);
    periodStart = inv.servicePeriodStart?.slice(0, 10) ?? "";
    periodEnd = inv.servicePeriodEnd?.slice(0, 10) ?? "";
    buyerReference = inv.buyerReference ?? "";
    dueDate = inv.dueDate?.slice(0, 10) ?? "";
    currency = inv.currencyCode;
    pdfTemplate = inv.pdfTemplate;
    notes = inv.notes ?? "";
    alreadyPaid = !!inv.paymentNote;
    paymentNote = inv.paymentNote ?? "";
    items = d.items.map((it) => ({
      position: it.position,
      description: it.description,
      quantity: it.quantity,
      unitCode: it.unitCode,
      unitPriceCents: it.unitPriceCents,
      taxRatePercent: it.taxRatePercent,
      taxCategoryCode: it.taxCategoryCode,
      descriptionTitle: it.descriptionTitle ?? "",
      descriptionMarkup: it.descriptionMarkup ?? "",
      sourcePackageId: it.sourcePackageId,
      sourcePackageRevision: it.sourcePackageRevision,
    }));
    if (items.length === 0) items = [emptyItem(1)];
    restoreConflict = d.restoreConflict ?? null;
  }

  function addItem() {
    items = [...items, emptyItem(items.length + 1)];
  }

  // Hängt eine Position an — füllt aber eine leere Schluss-Zeile (z. B. die
  // Default-Position 1), statt eine weitere leere Zeile stehen zu lassen.
  function addOrFill(item: FormItem) {
    const last = items[items.length - 1];
    const base = last && isEmptyItem(last) ? items.slice(0, -1) : items;
    items = [...base, item].map((it, idx) => ({ ...it, position: idx + 1 }));
  }

  function addTravelLine(line: TravelLine) {
    addOrFill({
      position: 0,
      description: line.description,
      quantity: line.quantity,
      unitCode: line.unitCode,
      unitPriceCents: line.unitPriceCents,
      taxRatePercent: 0,
      taxCategoryCode: line.taxCategoryCode,
      descriptionMarkup: "",
      descriptionTitle: "",
    });
  }

  function addPackage(m: MaterializedPackageItem) {
    addOrFill({
      position: 0,
      description: m.description,
      quantity: m.quantity,
      unitCode: m.unitCode,
      unitPriceCents: m.unitPriceCents,
      taxRatePercent: m.taxRatePercent,
      taxCategoryCode: m.taxCategoryCode,
      descriptionTitle: m.descriptionTitle,
      descriptionMarkup: m.descriptionMarkup,
      sourcePackageId: m.sourcePackageId,
      sourcePackageRevision: m.sourcePackageRevision,
      packageName: m.packageName,
    });
  }

  // „Paket anpassen": Provenienz-Zeiger lösen, aber Titel + Markup BEHALTEN —
  // die Position bleibt eine frei editierbare Rich-Position (Titel-Feld +
  // Markdown-Editor), nur ohne „aus Paket"-Bindung.
  function detachPackage(idx: number) {
    const it = items[idx];
    it.sourcePackageId = null;
    it.sourcePackageRevision = null;
    it.packageName = undefined;
  }

  // Befund E (U4): Eine Paket-Position mit einer Einheit außerhalb der Liste
  // (Freitext älterer Pakete, z. B. „Pauschal“) bekommt ein Auswahlfeld. Die Wahl
  // ändert die Kopie und löst die Bindung ans Paket (sauberer Bruch wie „Paket
  // anpassen“; der Server löst sie bei abweichender Einheit ebenso). Die Zeile
  // wird dabei neu aufgebaut — der Fokus bleibt auf dem Einheiten-Feld.
  async function unitChosenForPackage(idx: number) {
    detachPackage(idx);
    await tick();
    document.getElementById(`item-${idx}-unit`)?.focus();
  }

  function removeItem(idx: number) {
    items = items.filter((_, i) => i !== idx).map((it, i) => ({ ...it, position: i + 1 }));
  }

  /** Fokus auf das erste als ungültig markierte Feld des Formulars. */
  async function focusFirstInvalid(): Promise<void> {
    await tick();
    const el = document.querySelector<HTMLElement>('#invoice-form [aria-invalid="true"]');
    el?.focus();
  }

  const BLOCKED_PREFIX = "Der Entwurf kann nicht gespeichert werden: ";

  /**
   * Ordnet eine Ablehnung des Servers zu: Rechnungsdatum (Geschäftsjahr der
   * Nummer) und Empfänger am Feld, Strukturprüfungen (mehrere Gründe, „; “
   * getrennt) und alles Übrige als Banner. Technische Fehler bleiben ein Toast.
   */
  async function showServerError(e: unknown) {
    const msg = String(e);
    serverReasons = [];
    serverDateErr = null;
    serverContactErr = null;
    if (/^(Server nicht erreichbar|Fehler \d{3}$)/.test(msg)) {
      flash(msg, "error");
      return;
    }
    if (msg.startsWith("Das Rechnungsdatum muss im Geschäftsjahr")) {
      serverDateErr = { value: invoiceDate, msg };
      await focusFirstInvalid();
      return;
    }
    if (/Empfänger/.test(msg) && !msg.startsWith(BLOCKED_PREFIX)) {
      serverContactErr = { value: contactId, msg };
      await focusFirstInvalid();
      return;
    }
    serverReasons = msg.startsWith(BLOCKED_PREFIX)
      ? msg.slice(BLOCKED_PREFIX.length).split("; ").map((s) => s.trim()).filter(Boolean)
      : [msg];
    await tick();
    serverBannerEl?.focus();
  }

  async function save() {
    if (saving) return;
    submitted = true;
    if (
      errContact || errInvoiceDate || errServiceDate || errBuyerReference || errDueDate ||
      items.some(itemInvalid)
    ) {
      await focusFirstInvalid();
      return;
    }
    if (totals.gross <= 0) {
      // Meldung an den Preisfeldern (priceError); nur ohne solches Feld als Toast.
      await focusFirstInvalid();
      if (!items.some((it) => !it.sourcePackageId && it.unitPriceCents === 0)) {
        flash("Der Gesamtbetrag muss größer als 0 € sein.", "error");
      }
      return;
    }
    saving = true;
    serverReasons = [];
    try {
      const args: CreateDraftArgs = {
        contactId,
        input: {
          direction: "issued",
          invoiceDate,
          // ADR 0042 Nr. 10: Leistungsdatum ODER Zeitraum, nie beides.
          ...serviceFields,
          // ADR 0042 Nr. 12: Leitweg-ID am Entwurf; leer = keine.
          buyerReference: normalizeBuyerReference(buyerReference),
          dueDate: dueDate || null,
          currencyCode: "EUR",
          items: items.map(toInput),
          notes: notes.trim() || null,
          paymentNote: alreadyPaid ? (paymentNote.trim() || null) : null,
          pdfTemplate,
          isStornoFor: null,
          cancelReason: null,
        },
      };
      const detail = editId ? await invoicesUpdateDraft(editId, args) : await invoicesCreateDraft(args);
      // Warteliste (8): Ein nach der Wiederherstellung schon abgeglichener Entwurf
      // ist nach jeder Änderung wieder vorgemerkt (Server, Lauf F8).
      // Der Hinweis stand schon vor dem Speichern (RestoreConflictBanner).
      const done = restoreSaveFlash(restoreConflict, "invoice", !!editId);
      flash(done.msg, done.kind);
      goto(`/invoices/${detail.invoice.id}`);
    } catch (e) {
      await showServerError(e);
    } finally {
      saving = false;
    }
  }

  function addDays(iso: string, days: number): string {
    return addDaysIso(iso, days);
  }

  const saveLabel = $derived(saving ? "Speichere …" : editId ? "Änderungen speichern" : "Als Entwurf speichern");
</script>

<PageBar
  back={editId ? `/invoices/${editId}` : "/invoices"}
  backLabel={editId ? "Rechnung" : "Rechnungen"}
  title={editId ? (editNumber ? `Entwurf ${editNumber} bearbeiten` : "Entwurf bearbeiten") : "Neue Rechnung"}
>
  {#snippet actions()}
    {#if !notFound}
    <a href={editId ? `/invoices/${editId}` : "/invoices"} class="btn-secondary">Abbrechen</a>
    <!-- V2-06: während des Speicherns fokussierbar (aria-disabled), Klick ohne Wirkung. -->
    <Button type="submit" form="invoice-form" ariaDisabled={saving}>{saveLabel}</Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if notFound}
  <Banner kind="error" href="/invoices" linkLabel="Zu den Rechnungen">
    Diese Rechnung gibt es nicht (mehr). Prüfe den Link oder öffne die Rechnung aus der Liste.
  </Banner>
{:else if !seller}
  <Banner kind="warning">
    Deine Firmendaten fehlen. <a class="kb-link" href="/settings/seller?next=/invoices/new">Jetzt ausfüllen</a>.
  </Banner>
{:else}
  <TaxStatusNotice isKlein={taxAt.isKlein} day={taxAt.date} {todayKlein}
    dateLabel={serviceFields.servicePeriodEnd ? "Ende des Leistungszeitraums" : serviceFields.deliveryDate ? "Leistungsdatum" : "Rechnungsdatum"} noun="auf der Rechnung" />
  {#if taxAt.error}
    <Banner kind="warning">
      Der Steuerstatus am Leistungsdatum ließ sich nicht laden ({taxAt.error}). Klein.Buch prüft
      ihn beim Speichern und Ausstellen.
    </Banner>
  {/if}
  {#if editId && editNumber}
    <RestoreConflictBanner conflict={restoreConflict} number={editNumber} doc="invoice" where="editor" />
  {/if}

  <form id="invoice-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
    {#if serverReasons.length > 0}
      <!-- Ablehnung des Servers bleibt stehen, bis erneut gespeichert wird (ADR 0041 Nr. 22). -->
      <div class="server-err" tabindex="-1" bind:this={serverBannerEl}>
        <Banner kind="error" title={editId ? "Die Änderungen wurden nicht gespeichert." : "Der Entwurf wurde nicht gespeichert."}>
          {#if serverReasons.length === 1}
            {serverReasons[0]}
          {:else}
            <ul class="reasons">
              {#each serverReasons as r (r)}<li>{r}</li>{/each}
            </ul>
          {/if}
        </Banner>
      </div>
    {/if}
    <p class="req-legend">Felder mit <span aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
    <section class="card">
      <h2>Allgemein</h2>
      <div class="grid">
        <ContactSelect
          bind:value={contactId}
          contacts={contacts}
          label="Empfänger"
          id="inv-contact"
          required
          disabled={fromQuote}
          requiredMessage="Bitte einen Empfänger wählen."
          showErrors={submitted}
          error={shownContactErr}
          hint={fromQuote ? "Der Entwurf stammt aus einem Angebot; der Empfänger bleibt der des Angebots." : undefined}
          class="span-all kb-w-long"
        />
        <label>
          <span>Rechnungsdatum <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            class="kb-w-date"
            bind:value={invoiceDate}
            max={today}
            required
            aria-invalid={shownInvoiceDateErr ? "true" : undefined}
            aria-describedby={shownInvoiceDateErr ? "err-invoice-date" : draftFiscalYear !== null ? "hint-invoice-date" : undefined}
          />
          {#if shownInvoiceDateErr}
            <span id="err-invoice-date" class="field-err">{shownInvoiceDateErr}</span>
          {:else if draftFiscalYear !== null}
            <span id="hint-invoice-date" class="field-hint">Im Geschäftsjahr {draftFiscalYear}, zu dem die Nummer gehört.</span>
          {/if}
        </label>
        <label>
          <span>Fälligkeit <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            class="kb-w-date"
            bind:value={dueDate}
            min={invoiceDate || undefined}
            required
            aria-invalid={submitted && errDueDate ? "true" : undefined}
            aria-describedby={submitted && errDueDate ? "err-due-date" : "hint-due-date"}
          />
          {#if submitted && errDueDate}
            <span id="err-due-date" class="field-err">{errDueDate}</span>
          {:else}
            <span id="hint-due-date" class="field-hint">Pflicht für die E-Rechnung.</span>
          {/if}
        </label>
        <!-- ADR 0042 Nr. 10/11: Leistungsdatum oder -zeitraum; Vorausrechnung erlaubt (Hinweis). -->
        <div class="span-all">
          <ServiceDateFields
            bind:mode={serviceMode}
            bind:deliveryDate
            bind:periodStart
            bind:periodEnd
            {invoiceDate}
            showErrors={submitted}
            idPrefix="inv-svc"
            ondeliveryinput={() => (deliveryDateTouched = true)}
          />
        </div>
        <!-- V1-04: Währung fest (nur Euro), kein Eingabefeld. -->
        <div class="fixed-field">
          <span class="fixed-lbl">Währung</span>
          <span class="fixed-val">Euro (EUR)</span>
          {#if currency !== "EUR"}
            <span class="field-hint">Gespeichert war „{currency}“. Klein.Buch rechnet nur in Euro; beim Speichern gilt EUR.</span>
          {/if}
        </div>
        <label class="span-all">
          PDF-Vorlage
          <select class="kb-w-medium" bind:value={pdfTemplate} aria-describedby={stornoHint ? "hint-pdf-template" : undefined}>
            <option value="default">{pdfTemplateDefaultOptionLabel(seller.defaultPdfTemplate)}</option>
            {#each templateOptions as name (name)}
              <option value={name}>{pdfTemplateLabel(name)}</option>
            {/each}
            {#if pdfTemplate !== "default" && !templateOptions.includes(pdfTemplate)}
              <option value={pdfTemplate}>{pdfTemplateLabel(pdfTemplate)}</option>
            {/if}
          </select>
          {#if stornoHint}<span id="hint-pdf-template" class="field-hint">{stornoHint}</span>{/if}
        </label>
        <!-- ADR 0042 Nr. 12: Leitweg-ID am Entwurf (revidiert ADR 0041 Nr. 91), beim Ausstellen übernommen. -->
        <BuyerReferenceField bind:value={buyerReference} id="inv-buyer-ref" showErrors={submitted} class="span-all" />
      </div>
    </section>

    <section class="card">
      <header class="card-hdr">
        <h2>Positionen</h2>
        <button type="button" onclick={addItem} class="btn-secondary">+ Position</button>
      </header>
      <table class="cards items">
        <caption class="kb-visually-hidden">Positionen der Rechnung</caption>
        <thead>
          <tr>
            <th scope="col">#</th>
            <!-- V4-15: Pflichtspalten mit * (aria-hidden; die Felder tragen aria-required). -->
            <th scope="col">Beschreibung <span class="req" aria-hidden="true">*</span></th>
            <th scope="col" class="right">Menge <span class="req" aria-hidden="true">*</span></th>
            <th scope="col">Einheit</th>
            <th scope="col" class="right">Einzelpreis <span class="req" aria-hidden="true">*</span></th>
            {#if !isKlein}
              <th scope="col" class="right">USt-Satz <span class="req" aria-hidden="true">*</span></th>
              <th scope="col">USt-Art</th>
            {/if}
            <th scope="col" class="right">Netto</th>
            <th scope="col"><span class="kb-visually-hidden">Aktionen</span></th>
          </tr>
        </thead>
        <tbody>
          {#each items as it, i (i)}
            {@const n = it.position}
            {@const descErr = submitted && labelMissing(it)}
            <!-- Befund E: Einheit außerhalb der Liste; die Meldung steht sofort, sie stammt aus den Daten (Paket, Entwurf). -->
            {@const unitErr = unitCodeError(it.unitCode, !!it.sourcePackageId)}
            {#if isRich(it)}
              {@const linked = !!it.sourcePackageId}
              <tr class="pkg-row">
                <td data-label="Pos.">{n}</td>
                <td class="cards-full" data-label={linked ? "Titel" : "Titel *"}>
                  {#if linked}
                    <Badge kind="tag">Aus Paket: {it.packageName ?? "Paket"} (V{it.sourcePackageRevision})</Badge>
                    <div class="pkg-title">{it.descriptionTitle}</div>
                  {:else}
                    <label class="kb-visually-hidden" for="item-{i}-title">Titel, Position {n}</label>
                    <input
                      id="item-{i}-title"
                      type="text"
                      bind:value={it.descriptionTitle}
                      required
                      aria-required="true"
                      aria-invalid={descErr ? "true" : undefined}
                      aria-describedby={descErr ? `item-${i}-title-err` : undefined}
                    />
                    {#if descErr}<span id="item-{i}-title-err" class="field-err">Bitte einen Titel eingeben.</span>{/if}
                  {/if}
                </td>
                {#if linked}
                  <td class="right" data-label="Menge">{formatQuantity(it.quantity ?? 0)}</td>
                  {#if unitErr}
                    <td class="c-unit" data-label="Einheit">
                      <select
                        id="item-{i}-unit"
                        bind:value={it.unitCode}
                        onchange={() => unitChosenForPackage(i)}
                        aria-label={`Einheit, Position ${n}`}
                        aria-invalid="true"
                        aria-describedby={`item-${i}-unit-err`}
                      >
                        {#each unitOptions(it.unitCode) as u (u.value)}<option value={u.value}>{u.label}</option>{/each}
                      </select>
                      <span id="item-{i}-unit-err" class="field-err">{unitErr}</span>
                    </td>
                  {:else}
                    <td data-label="Einheit">{unitLabel(it.unitCode)}</td>
                  {/if}
                  <td class="right" data-label="Einzelpreis">{euro(it.unitPriceCents ?? 0)}</td>
                {:else}
                  <td class="c-qty" data-label="Menge *">
                    <QuantityInput bind:value={it.quantity} ariaLabel={`Menge, Position ${n}`} required showErrors={submitted} />
                  </td>
                  <td class="c-unit" data-label="Einheit">
                    <select
                      id="item-{i}-unit"
                      bind:value={it.unitCode}
                      aria-label={`Einheit, Position ${n}`}
                      aria-invalid={unitErr ? "true" : undefined}
                      aria-describedby={unitErr ? `item-${i}-unit-err` : undefined}
                    >
                      {#each unitOptions(it.unitCode) as u (u.value)}<option value={u.value}>{u.label}</option>{/each}
                    </select>
                    {#if unitErr}<span id="item-{i}-unit-err" class="field-err">{unitErr}</span>{/if}
                  </td>
                  <td class="c-price" data-label="Einzelpreis *">
                    <MoneyInput bind:cents={it.unitPriceCents} ariaLabel={`Einzelpreis, Position ${n}`} required showErrors={submitted} error={priceError(it)} />
                  </td>
                {/if}
                {#if !isKlein}
                  <!-- Regelbesteuerung: Satz und Steuerart je Position (KB-0047); sie gehören
                       nicht zum Paket-Inhalt, die Provenienz bleibt. -->
                  <td class="c-rate" data-label="USt-Satz *">
                    <QuantityInput bind:value={it.taxRatePercent} maxFractionDigits={2} unit="%" noun="Steuersatz" ariaLabel={`USt-Satz, Position ${n}`} required showErrors={submitted} />
                  </td>
                  <td data-label="USt-Art">
                    <select bind:value={it.taxCategoryCode} aria-label={`USt-Art, Position ${n}`}>
                      {#each REGEL_TAX_CODES as code (code)}<option value={code}>{code}</option>{/each}
                    </select>
                  </td>
                {/if}
                <td class="right" data-label="Netto">{euro(lineNet(it))}</td>
                {#if linked || items.length > 1}
                  <td class="c-actions">
                    <div class="kb-actions">
                      {#if linked}
                        <button type="button" class="btn-secondary" onclick={() => detachPackage(i)} aria-label={`Paket anpassen, Position ${n}`}>Paket anpassen</button>
                      {/if}
                      {#if items.length > 1}
                        <button type="button" class="btn-danger" onclick={() => removeItem(i)} aria-label={`Position ${n} entfernen`}>×</button>
                      {/if}
                    </div>
                  </td>
                {:else}
                  <td class="c-actions"></td>
                {/if}
              </tr>
              <tr class="pkg-detail">
                <td colspan="99" class="cards-full">
                  {#if linked}
                    <div class="pkg-desc">{it.description}</div>
                  {:else}
                    <MarkdownEditor
                      bind:value={it.descriptionMarkup}
                      rows={5}
                      id={`item-${i}-markup`}
                      label={`Beschreibung, Position ${n} (Markdown, erscheint formatiert auf dem PDF)`}
                    />
                  {/if}
                </td>
              </tr>
            {:else}
              <tr>
                <td data-label="Pos.">{n}</td>
                <td class="cards-full" data-label="Beschreibung *">
                  <label class="kb-visually-hidden" for="item-{i}-desc">Beschreibung, Position {n}</label>
                  <input
                    id="item-{i}-desc"
                    type="text"
                    bind:value={it.description}
                    required
                    aria-required="true"
                    aria-invalid={descErr ? "true" : undefined}
                    aria-describedby={descErr ? `item-${i}-desc-err` : undefined}
                  />
                  {#if descErr}<span id="item-{i}-desc-err" class="field-err">Bitte eine Beschreibung eingeben.</span>{/if}
                </td>
                <td class="c-qty" data-label="Menge *">
                  <QuantityInput bind:value={it.quantity} ariaLabel={`Menge, Position ${n}`} required showErrors={submitted} />
                </td>
                <td class="c-unit" data-label="Einheit">
                  <select
                    id="item-{i}-unit"
                    bind:value={it.unitCode}
                    aria-label={`Einheit, Position ${n}`}
                    aria-invalid={unitErr ? "true" : undefined}
                    aria-describedby={unitErr ? `item-${i}-unit-err` : undefined}
                  >
                    {#each unitOptions(it.unitCode) as u (u.value)}<option value={u.value}>{u.label}</option>{/each}
                  </select>
                  {#if unitErr}<span id="item-{i}-unit-err" class="field-err">{unitErr}</span>{/if}
                </td>
                <td class="c-price" data-label="Einzelpreis *">
                  <MoneyInput bind:cents={it.unitPriceCents} ariaLabel={`Einzelpreis, Position ${n}`} required showErrors={submitted} error={priceError(it)} />
                </td>
                {#if !isKlein}
                  <td class="c-rate" data-label="USt-Satz *">
                    <QuantityInput bind:value={it.taxRatePercent} maxFractionDigits={2} unit="%" noun="Steuersatz" ariaLabel={`USt-Satz, Position ${n}`} required showErrors={submitted} />
                  </td>
                  <td data-label="USt-Art">
                    <select bind:value={it.taxCategoryCode} aria-label={`USt-Art, Position ${n}`}>
                      {#each REGEL_TAX_CODES as code (code)}<option value={code}>{code}</option>{/each}
                    </select>
                  </td>
                {/if}
                <td class="right" data-label="Netto">{euro(lineNet(it))}</td>
                {#if items.length > 1}
                  <td class="c-actions">
                    <button type="button" class="btn-danger" onclick={() => removeItem(i)} aria-label={`Position ${n} entfernen`}>×</button>
                  </td>
                {:else}
                  <td class="c-actions"></td>
                {/if}
              </tr>
            {/if}
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <td colspan={isKlein ? 5 : 7} class="right sum-label"><strong>Netto</strong></td>
            <td class="right" data-label="Netto"><strong>{euro(totals.net)}</strong></td>
            <td class="c-actions"></td>
          </tr>
          {#if !isKlein}
            <tr>
              <td colspan="7" class="right sum-label">USt</td>
              <td class="right" data-label="USt">{euro(totals.tax)}</td>
              <td class="c-actions"></td>
            </tr>
          {/if}
          <tr class="kb-sum">
            <td colspan={isKlein ? 5 : 7} class="right sum-label"><strong>Brutto</strong></td>
            <td class="right" data-label="Brutto"><strong>{euro(totals.gross)}</strong></td>
            <td class="c-actions"></td>
          </tr>
        </tfoot>
      </table>
      <TravelLineAdder onAdd={addTravelLine} />
      <PackageItemAdder onAdd={addPackage} documentDate={taxDate} />
    </section>

    <section class="card">
      <h2 id="invoice-notes-h">Notiz</h2>
      <textarea id="invoice-notes" rows="3" bind:value={notes} aria-labelledby="invoice-notes-h" aria-describedby="hint-invoice-notes"></textarea>
      <p id="hint-invoice-notes" class="field-hint">Zum Beispiel ein Zahlungshinweis oder die Bestellnummer des Kunden.</p>
    </section>

    <section class="card">
      <h2>Bezahlung</h2>
      <label class="paid-toggle">
        <input type="checkbox" bind:checked={alreadyPaid} />
        <strong>Teil oder ganz bereits gezahlt</strong>
      </label>
      {#if alreadyPaid}
        <p class="paid-hint small">
          Reiner Hinweis auf dem Beleg — die Zahlung wird dadurch <strong>nicht</strong>
          verbucht. Erfasse sie nach dem Ausstellen über „+ Zahlung“ auf der Rechnungsseite.
        </p>
        <div class="paid-chips" role="group" aria-label="Textvorschläge für den Bezahlt-Hinweis">
          {#each paymentTemplates as t (t)}
            <button type="button" class="btn-secondary" onclick={() => (paymentNote = t)}>{t}</button>
          {/each}
        </div>
        <label class="paid-note">
          Text des Hinweises auf dem Beleg
          <textarea rows="2" bind:value={paymentNote}></textarea>
        </label>
      {/if}
    </section>

    <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end"><Button type="submit" ariaDisabled={saving}>{saveLabel}</Button></div>
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .req-legend { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .field-err { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  .server-err { outline: none; }
  .server-err:focus-visible { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); border-radius: var(--radius-control); }
  .reasons { margin: var(--sp-1) 0 0; padding-left: 1.25rem; }
  /* U1 (2.5.8): ganze Zeile als Trefferfläche, mindestens --target hoch. */
  .paid-toggle { display: flex; flex-direction: row; align-items: center; gap: 0.5rem; color: var(--text); min-height: var(--target); cursor: pointer; }
  .paid-hint { color: var(--text-muted); line-height: 1.5; margin: 0.6rem 0; }
  /* V3-21: Vorschläge als normale Sekundärknöpfe (eine Höhe, eine Kante, design.md §4.1). */
  .paid-chips { display: flex; flex-wrap: wrap; gap: var(--sp-2); margin-bottom: var(--sp-3); }
  .card-hdr { display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3); flex-wrap: wrap; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; align-items: start; }
  /* Empfänger und Vorlage über die volle Zeile, Felder so breit wie ihr Inhalt (U3-031). */
  .grid > :global(.span-all), .grid > .span-all { grid-column: 1 / -1; }
  .grid :global(.contact-select) { margin-bottom: 0; }
  .grid :global(.contact-select > label) { color: var(--text-muted); }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .paid-note { margin-top: var(--sp-2); }
  input, select, textarea {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text); background: var(--surface-card);
  }
  textarea { resize: vertical; width: 100%; box-sizing: border-box; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid var(--line); vertical-align: middle; }
  th { background: var(--surface-sunken); font-weight: 600; font-size: var(--fs-sm); }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  td.c-qty { width: 7.5rem; }
  /* Einheit im Klartext (U1-060): Platz für „Pauschale“, „Kilometer“ samt Pfeil. */
  td.c-unit { width: 9rem; }
  td.c-unit select { width: 100%; }
  td.c-price { width: 9.5rem; }
  td.c-rate { width: 7rem; }
  .muted { color: var(--text-muted); }
  /* Feste Angabe ohne Eingabe (Währung, V1-04): Label wie die Felder, Wert als Text. */
  .fixed-field { display: flex; flex-direction: column; gap: 4px; }
  .fixed-lbl { font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .fixed-val { min-height: var(--control-h); display: flex; align-items: center; color: var(--text); }
  .pkg-desc { font-size: var(--fs-sm); color: var(--text-muted); margin-top: 0.2rem; white-space: pre-line; overflow-wrap: anywhere; }
  .pkg-title { font-weight: 600; margin-top: 0.25rem; overflow-wrap: anywhere; }
  .pkg-row td { background: var(--surface-sunken); border-bottom: 0; }
  .pkg-detail td { background: var(--surface-sunken); padding-top: 0; }
  /* Plan W10 (Karten unter 820 px): die Summen-Beschriftung steht im
     data-label der Betragszelle; Paket-Kopf und -Beschreibung bilden eine Karte.
     Leere Aktionszellen (eine einzige Position) erscheinen nicht als Kartenzeile. */
  @media screen and (max-width: 820px) {
    td.sum-label { display: none; }
    td.c-qty, td.c-unit, td.c-price, td.c-rate { width: auto; }
    tr.pkg-row { margin-bottom: 0; border-bottom: 0; border-radius: var(--r-lg) var(--r-lg) 0 0; }
    tr.pkg-detail { border-top: 0; border-radius: 0 0 var(--r-lg) var(--r-lg); }
  }
</style>
