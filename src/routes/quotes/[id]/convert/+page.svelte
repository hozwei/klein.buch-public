<script lang="ts">
  import { todayBerlin, yearOfIsoDate, addDaysIso } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    quotesGet,
    sellerProfileGet,
    quotesConvertToInvoice,
    documentTermsGet,
    pdfTemplatesList,
  } from "#lib/api.ts";
  import type {
    QuoteConvertArgs,
    PdfTemplateMeta,
    QuoteDetail,
    QuoteItemInput,
    SellerProfile,
    TaxCategoryCode,
    TravelLine,
    MaterializedPackageItem,
  } from "#lib/types.ts";
  import { date, euro, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { applyTaxRules, documentTotals, REGEL_TAX_CODES, roundHalfAwayFromZero } from "#lib/tax.ts";
  import { pdfTemplateLabel, pdfTemplateDefaultOptionLabel, unitCodeError, unitLabel, unitOptions } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Badge from "#lib/Badge.svelte";
  import TravelLineAdder from "#lib/TravelLineAdder.svelte";
  import PackageItemAdder from "#lib/PackageItemAdder.svelte";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import TaxStatusNotice from "#lib/TaxStatusNotice.svelte";
  import { invoiceTaxDate, quoteConversionHint } from "#lib/taxStatus.ts";
  import { taxStatusAtDate } from "#lib/taxStatusAt.svelte.ts";
  import ServiceDateFields from "#lib/ServiceDateFields.svelte";
  import BuyerReferenceField from "#lib/BuyerReferenceField.svelte";
  import {
    buyerReferenceError,
    normalizeBuyerReference,
    serviceDateInvalid,
    serviceDatePayload,
    type ServiceDateMode,
  } from "#lib/serviceDate.ts";

  // Form-Position = QuoteItemInput, aber descriptionMarkup als String (für den
  // bind:value-Editor) + transienter Paket-Name fürs Badge.
  // Menge, Preis und Satz sind `null`, solange das Feld leer oder ungültig ist
  // (MoneyInput/QuantityInput, U1); beim Speichern geprüft.
  type FormItem = Omit<QuoteItemInput, "quantity" | "unitPriceCents" | "taxRatePercent"> & {
    quantity: number | null;
    unitPriceCents: number | null;
    taxRatePercent: number | null;
    descriptionMarkup: string;
    descriptionTitle: string;
    packageName?: string;
  };

  // Rich-Position (aus Paket oder „angepasst"): hat Titel/Markup → Titel-Feld +
  // Markdown-Editor statt schmaler Beschreibung.
  function isRich(it: FormItem): boolean {
    return !!(it.descriptionTitle || it.descriptionMarkup);
  }
  function labelMissing(it: FormItem): boolean {
    return isRich(it) ? !(it.descriptionTitle ?? "").trim() : !it.description.trim();
  }
  // Eine leere, unangetastete Custom-Zeile.
  function isEmptyItem(it: FormItem): boolean {
    return !isRich(it) && !it.description.trim() && !it.unitPriceCents;
  }
  function lineNet(it: FormItem): number {
    return roundHalfAwayFromZero((it.quantity ?? 0) * (it.unitPriceCents ?? 0));
  }
  function toInput(it: FormItem): QuoteItemInput {
    return {
      ...it,
      quantity: it.quantity ?? 0,
      unitPriceCents: it.unitPriceCents ?? 0,
      taxRatePercent: it.taxRatePercent ?? 0,
    };
  }

  let id = $derived(page.params.id ?? "");

  let detail = $state<QuoteDetail | null>(null);
  let seller = $state<SellerProfile | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let templates = $state<PdfTemplateMeta[]>([]);
  // Nach dem ersten Speicher-Versuch zeigen alle Felder ihre Meldungen.
  let submitted = $state(false);

  // Rechnungs-spezifische Felder (das Angebot trägt sie nicht).
  // Heute = Obergrenze für das Rechnungsdatum (keine Zukunft). Das Leistungsdatum
  // darf danach liegen (Vorausrechnung, ADR 0042 Nr. 11).
  const today = todayBerlin();
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
  // ADR 0042 Nr. 12: Leitweg-ID der Rechnung, am Entwurf gespeichert.
  let buyerReference = $state("");
  let dueDate = $state("");
  let pdfTemplate = $state("default");
  let notes = $state("");
  let currency = $state("EUR");

  // Bezahlt-Hinweis: reiner PDF-Text. KEINE EÜR-Buchung.
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
  // Standard-Vorlage aus den Einstellungen (Server: commands::invoices); Namen
  // zentral aus labels.ts (Warteliste (17)).
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

  // Positionen werden aus dem Angebot übernommen und sind anpassbar.
  let items = $state<FormItem[]>([]);

  // ADR 0042 Nr. 3: Status am Leistungsdatum der Rechnung (sonst Rechnungsdatum),
  // nicht der von heute und nicht der eingefrorene des Angebots.
  // Bei einem Zeitraum gilt dessen Ende (ADR 0042 Nr. 10).
  let serviceFields = $derived(serviceDatePayload(serviceMode, deliveryDate, periodStart, periodEnd));
  let taxDate = $derived(invoiceTaxDate({ ...serviceFields, invoiceDate }, today));
  const taxAt = taxStatusAtDate(() => taxDate);
  let todayKlein = $derived(seller ? seller.isKleinunternehmer === 1 : null);
  let isKlein = $derived(taxAt.isKlein ?? todayKlein === true);
  let conversionHint = $derived(
    quoteConversionHint(
      detail ? detail.quote.isKleinunternehmer === 1 : null,
      taxAt.date === taxDate ? taxAt.isKlein : null,
    ),
  );
  // Konvertierung nur aus angenommenem Angebot (Hard-Rule). Backend prüft
  // ebenfalls — das hier ist die UI-Sperre.
  let convertible = $derived(detail?.quote.status === "accepted");

  let fiscalYear = $derived(() => {
    return yearOfIsoDate(invoiceDate);
  });

  // §19-Hardline: USt-Felder gesperrt, Items auf category=E / rate=0 zwingen.
  // In-place pro Property mutieren (keine neue Array-Identität → kein Effect-Loop).
  // Regelbesteuerung: E wird S 19 % (KB-0014; lib/tax.ts).
  $effect(() => {
    // Nur mit dem Status genau dieses Datums (kein Umschreiben mit einem veralteten Wert).
    applyTaxRules(items as unknown as QuoteItemInput[], seller && taxAt.date === taxDate ? taxAt.isKlein : null);
  });

  // Wie der Server: USt je Steuergruppe aus der Nettosumme (BR-CO-17).
  let totals = $derived(documentTotals(items.map(toInput)));

  // ---- Feldfehler (spiegeln domain::invoice; der Server prüft erneut) ----
  let errInvoiceDate = $derived(
    !invoiceDate
      ? "Bitte das Rechnungsdatum angeben."
      : invoiceDate > today
        ? "Das Rechnungsdatum darf nicht in der Zukunft liegen."
        : "",
  );
  // Leistungsdatum bzw. -zeitraum: Meldungen stehen am Feld (ServiceDateFields).
  let errServiceDate = $derived(serviceDateInvalid(serviceMode, deliveryDate, periodStart, periodEnd));
  let errBuyerReference = $derived(buyerReferenceError(buyerReference));
  // V1-04: Fälligkeit ist Pflicht für die E-Rechnung (domain::invoice, XRechnungDueDateMissing
  // bei offenem Betrag) — Meldung schon beim Umwandeln statt erst beim Ausstellen.
  let errDueDate = $derived(
    !dueDate
      ? "Bitte eine Fälligkeit angeben — Pflicht für die E-Rechnung."
      : invoiceDate && dueDate < invoiceDate
        ? "Die Fälligkeit darf nicht vor dem Rechnungsdatum liegen."
        : "",
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
    error = null;
    try {
      [detail, seller, templates] = await Promise.all([
        quotesGet(id),
        sellerProfileGet(),
        // Ohne Liste bleibt die Auswahl bei „Standard aus den Einstellungen“.
        pdfTemplatesList().catch(() => [] as PdfTemplateMeta[]),
      ]);
      if (!detail) {
        error = "Angebot nicht gefunden.";
        return;
      }
      const q = detail.quote;
      currency = q.currencyCode;
      pdfTemplate = q.pdfTemplate;
      notes = q.notes ?? "";
      items = detail.items.map((it) => ({
        position: it.position,
        description: it.description,
        quantity: it.quantity,
        unitCode: it.unitCode,
        unitPriceCents: it.unitPriceCents,
        taxRatePercent: it.taxRatePercent,
        taxCategoryCode: it.taxCategoryCode,
        // Paket-Provenienz + Titel + Markup aus dem Angebot übernehmen (bleibt verknüpft).
        descriptionTitle: it.descriptionTitle ?? "",
        descriptionMarkup: it.descriptionMarkup ?? "",
        sourcePackageId: it.sourcePackageId,
        sourcePackageRevision: it.sourcePackageRevision,
      }));
      // Fälligkeit aus dem Standard-Zahlungsziel vorbelegen (wie bei „Neue
      // Rechnung") — sonst bliebe das Feld bei der Konvertierung leer und die
      // Rechnung trüge kein „Fällig am".
      const terms = await documentTermsGet();
      dueDate = addDays(invoiceDate, terms.invoiceDueDays);
      await taxAt.load(taxDate).catch(() => {});
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function addDays(iso: string, days: number): string {
    return addDaysIso(iso, days);
  }

  function addItem() {
    items = [
      ...items,
      {
        position: items.length + 1,
        description: "",
        quantity: 1,
        unitCode: "C62",
        // V4-05: leer statt 0,00 — so meldet das Feld einen vergessenen Preis selbst.
        unitPriceCents: null,
        taxRatePercent: 0,
        taxCategoryCode: "E" as TaxCategoryCode,
        descriptionMarkup: "",
        descriptionTitle: "",
      },
    ];
  }

  // Hängt eine Position an — füllt aber eine leere Schluss-Zeile, statt eine
  // weitere leere Zeile stehen zu lassen.
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

  // „Paket anpassen": Provenienz-Zeiger lösen, Titel + Markup behalten (bleibt
  // editierbare Rich-Position).
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
    const el = document.querySelector<HTMLElement>('#convert-form [aria-invalid="true"]');
    el?.focus();
  }

  async function save() {
    if (!detail || saving) return;
    if (items.length === 0) {
      flash("Mindestens eine Position erforderlich.", "error");
      return;
    }
    submitted = true;
    if (errInvoiceDate || errServiceDate || errBuyerReference || errDueDate || items.some(itemInvalid)) {
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
    try {
      const args: QuoteConvertArgs = {
        quoteId: detail.quote.id,
        invoiceDate,
        // ADR 0042 Nr. 10/12: Leistungsdatum ODER Zeitraum, Leitweg-ID (leer = keine).
        ...serviceFields,
        buyerReference: normalizeBuyerReference(buyerReference),
        dueDate: dueDate || null,
        pdfTemplate: pdfTemplate || null,
        notes: notes.trim() || null,
        paymentNote: alreadyPaid ? (paymentNote.trim() || null) : null,
        items: items.map(toInput),
      };
      const inv = await quotesConvertToInvoice(args);
      flash("Rechnungs-Entwurf erstellt.");
      goto(`/invoices/${inv.invoice.id}`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      saving = false;
    }
  }
</script>

<PageBar
  back={`/quotes/${id}`}
  backLabel="Angebot"
  title={detail ? `Angebot ${detail.quote.quoteNumber} in Rechnung umwandeln` : "Angebot in Rechnung umwandeln"}
>
  {#snippet actions()}
    {#if convertible}
      <a href={`/quotes/${id}`} class="btn-secondary">Abbrechen</a>
      <!-- V2-06: während des Erzeugens fokussierbar (aria-disabled), Klick ohne Wirkung. -->
      <Button type="submit" form="convert-form" ariaDisabled={saving}>
        {saving ? "Erzeuge Rechnung …" : "Rechnung erstellen"}
      </Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error && !detail}
  <Banner>Fehler: {error}</Banner>
{:else if detail}
  {@const q = detail.quote}
  <p class="sub">
    Aus Angebot <DocNo value={q.quoteNumber} href={`/quotes/${id}`} inline /> ({q.currencyCode}) wird ein
    Rechnungs-Entwurf erzeugt. Die Positionen sind übernommen und hier noch
    anpassbar; ausgestellt wird erst die Rechnung selbst.
  </p>

  {#if !convertible}
    <Banner kind="warning">
      Dieses Angebot ist noch nicht angenommen. Eine Rechnung lässt sich nur aus
      einem <strong>angenommenen</strong> Angebot erstellen.
      <a class="kb-link" href={`/quotes/${id}`}>Zurück zum Angebot</a>.
    </Banner>
  {:else}
    <TaxStatusNotice isKlein={taxAt.isKlein} day={taxAt.date} {todayKlein}
      dateLabel={serviceFields.servicePeriodEnd ? "Ende des Leistungszeitraums" : serviceFields.deliveryDate ? "Leistungsdatum" : "Rechnungsdatum"} noun="auf der Rechnung" />
    {#if conversionHint}
      <Banner kind="warning" title="Anderer Steuerstatus als im Angebot.">{conversionHint}</Banner>
    {/if}

    <form id="convert-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
      <p class="req-legend">Felder mit <span aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
      <section class="card">
        <h2>Allgemein</h2>
        <div class="grid">
          <label>
            <span>Rechnungsdatum <span class="req" aria-hidden="true">*</span></span>
            <input
              type="date"
              class="kb-w-date"
              bind:value={invoiceDate}
              max={today}
              required
              aria-invalid={submitted && errInvoiceDate ? "true" : undefined}
              aria-describedby={submitted && errInvoiceDate ? "err-invoice-date" : undefined}
            />
            {#if submitted && errInvoiceDate}<span id="err-invoice-date" class="field-err">{errInvoiceDate}</span>{/if}
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
              idPrefix="conv-svc"
              ondeliveryinput={() => (deliveryDateTouched = true)}
            />
          </div>
          <!-- V1-04: Währung fest (nur Euro), kein Eingabefeld. -->
          <div class="fixed-field">
            <span class="fixed-lbl">Währung</span>
            <span class="fixed-val">{currency === "EUR" ? "Euro (EUR)" : currency}</span>
          </div>
          <label class="span-all">
            PDF-Vorlage
            <select class="kb-w-medium" bind:value={pdfTemplate} aria-describedby={stornoHint ? "hint-pdf-template" : undefined}>
              <option value="default">{pdfTemplateDefaultOptionLabel(seller?.defaultPdfTemplate)}</option>
              {#each templateOptions as name (name)}
                <option value={name}>{pdfTemplateLabel(name)}</option>
              {/each}
              {#if pdfTemplate !== "default" && !templateOptions.includes(pdfTemplate)}
                <option value={pdfTemplate}>{pdfTemplateLabel(pdfTemplate)}</option>
              {/if}
            </select>
            {#if stornoHint}<span id="hint-pdf-template" class="field-hint">{stornoHint}</span>{/if}
          </label>
          <!-- ADR 0042 Nr. 12: Leitweg-ID am Rechnungs-Entwurf. -->
          <BuyerReferenceField bind:value={buyerReference} id="conv-buyer-ref" showErrors={submitted} class="span-all" />
        </div>
        <p class="muted small">Jahr der Rechnung: {fiscalYear()} · Empfänger: {detail.buyer?.name ?? "—"}</p>
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
              <th scope="col">Beschreibung <span class="req" aria-hidden="true">*</span></th>
              <th scope="col" class="kb-num">Menge <span class="req" aria-hidden="true">*</span></th>
              <th scope="col">Einheit</th>
              <th scope="col" class="kb-num">Einzelpreis <span class="req" aria-hidden="true">*</span></th>
              {#if !isKlein}
                <th scope="col" class="kb-num">USt-Satz <span class="req" aria-hidden="true">*</span></th>
                <th scope="col">USt-Art</th>
              {/if}
              <th scope="col" class="kb-num">Netto</th>
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
                    <td class="kb-num" data-label="Menge">{formatQuantity(it.quantity ?? 0)}</td>
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
                    <td class="kb-num" data-label="Einzelpreis">{euro(it.unitPriceCents ?? 0)}</td>
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
                    <td class="kb-num" data-label="USt-Satz">{percent(it.taxRatePercent ?? 0)}</td>
                    <td data-label="USt-Art">{it.taxCategoryCode}</td>
                  {/if}
                  <td class="kb-num" data-label="Netto">{euro(lineNet(it))}</td>
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
                  <td class="kb-num" data-label="Netto">{euro(lineNet(it))}</td>
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
              <td colspan={isKlein ? 5 : 7} class="sum-label"><strong>Netto</strong></td>
              <td class="kb-num" data-label="Netto"><strong>{euro(totals.net)}</strong></td>
              <td class="c-actions"></td>
            </tr>
            {#if !isKlein}
              <tr>
                <td colspan="7" class="sum-label">USt</td>
                <td class="kb-num" data-label="USt">{euro(totals.tax)}</td>
                <td class="c-actions"></td>
              </tr>
            {/if}
            <tr class="kb-sum">
              <td colspan={isKlein ? 5 : 7} class="sum-label"><strong>Brutto</strong></td>
              <td class="kb-num" data-label="Brutto"><strong>{euro(totals.gross)}</strong></td>
              <td class="c-actions"></td>
            </tr>
          </tfoot>
        </table>
        <TravelLineAdder onAdd={addTravelLine} />
        <PackageItemAdder onAdd={addPackage} documentDate={taxDate} />
      </section>

      <section class="card">
        <h2 id="convert-notes-h">Notiz</h2>
        <textarea id="convert-notes" rows="3" bind:value={notes} aria-labelledby="convert-notes-h" aria-describedby="hint-convert-notes"></textarea>
        <p id="hint-convert-notes" class="field-hint">Zum Beispiel die Bestellnummer des Kunden.</p>
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
      <div class="kb-form-end"><Button type="submit" ariaDisabled={saving}>{saving ? "Erzeuge Rechnung …" : "Rechnung erstellen"}</Button></div>
    </form>
  {/if}
{/if}

<style>
  .sub { color: var(--text-muted); margin: 0 0 1rem; }
  .small { font-size: var(--fs-sm); }
  /* Feste Angabe ohne Eingabe (Währung, V1-04): Label wie die Felder, Wert als Text. */
  .fixed-field { display: flex; flex-direction: column; gap: 4px; }
  .fixed-lbl { font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .fixed-val { min-height: var(--control-h); display: flex; align-items: center; color: var(--text); }
  /* .card entfernt — globale Definition aus tokens.css. */
  .req-legend { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .field-err { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  /* U1 (2.5.8): ganze Zeile als Trefferfläche, mindestens --target hoch. */
  .paid-toggle { display: flex; flex-direction: row; align-items: center; gap: 0.5rem; color: var(--text); min-height: var(--target); cursor: pointer; }
  .paid-hint { color: var(--text-muted); line-height: 1.5; margin: 0.6rem 0; }
  /* V3-21: Vorschläge als normale Sekundärknöpfe (eine Höhe, eine Kante, design.md §4.1). */
  .paid-chips { display: flex; flex-wrap: wrap; gap: var(--sp-2); margin-bottom: var(--sp-3); }
  .card-hdr { display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3); flex-wrap: wrap; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; align-items: start; }
  /* Vorlage über die volle Zeile, Felder so breit wie ihr Inhalt (U3-031). */
  .grid > .span-all, .grid > :global(.span-all) { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .paid-note { margin-top: var(--sp-2); }
  input, select, textarea {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text); background: var(--surface-card);
  }
  textarea { resize: vertical; width: 100%; box-sizing: border-box; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; border-bottom: 1px solid var(--line); vertical-align: middle; }
  th { background: var(--surface-sunken); font-weight: 600; font-size: var(--fs-sm); }
  /* Beträge über `.kb-num` (V3-10, ADR 0042 Nr. 16); Summen-Bezeichnung rechts davor. */
  td.sum-label { text-align: right; }
  td.c-qty { width: 7.5rem; }
  /* Einheit im Klartext (U1-060): Platz für „Pauschale“, „Kilometer“ samt Pfeil. */
  td.c-unit { width: 9rem; }
  td.c-unit select { width: 100%; }
  td.c-price { width: 9.5rem; }
  td.c-rate { width: 7rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): überschrieb
     padding/radius/font/color der globalen .btn-*-Klassen aus tokens.css
     und brach app-weite Button-Konsistenz. Die globalen Tokens greifen jetzt. */
  .muted { color: var(--text-muted); }
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
