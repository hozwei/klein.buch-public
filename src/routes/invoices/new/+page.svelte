<script lang="ts">
  import { todayBerlin, addDaysIso } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
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
  } from "#lib/types.ts";
  import { date, euro } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { applyTaxRules, documentTotals, REGEL_TAX_CODES, roundHalfAwayFromZero } from "#lib/tax.ts";
  import Banner from "#lib/Banner.svelte";
  import TravelLineAdder from "#lib/TravelLineAdder.svelte";
  import PackageItemAdder from "#lib/PackageItemAdder.svelte";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";

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
  let saving = $state(false);
  // Nach dem ersten Speicher-Versuch zeigen alle Felder ihre Meldungen.
  let submitted = $state(false);
  // Lauf F5 (Neuer Fund F2a): `?edit=<id>` bearbeitet einen gespeicherten Entwurf
  // mit demselben Formular (Gestaltung im UI/UX-Lauf). Die Nummer bleibt.
  const editId = page.url.searchParams.get("edit");
  let editNumber = $state<string | null>(null);

  // Heute (Europe/Berlin) — Obergrenze für Rechnungs- und Leistungsdatum:
  // beide dürfen nicht in der Zukunft liegen (domain::invoice).
  const today = todayBerlin();
  let contactId = $state("");
  let invoiceDate = $state(today);
  // U1: Leistungsdatum (Pflicht, §14 Abs. 4 Nr. 6 UStG) mit dem Rechnungsdatum
  // vorbelegt — sichtbar und änderbar; es folgt dem Rechnungsdatum, bis es von
  // Hand geändert wird (Angebote tragen kein eigenes Leistungsdatum).
  let deliveryDate = $state(today);
  let deliveryDateTouched = $state(false);
  $effect(() => {
    if (!deliveryDateTouched) deliveryDate = invoiceDate;
  });
  let dueDate = $state("");
  let currency = $state("EUR");
  let pdfTemplate = $state("default");
  let notes = $state("");
  let buyerReference = $state("N/A");

  // Bezahlt-Hinweis: reiner PDF-Text je Rechnung. KEINE EÜR-Buchung — die
  // Zahlung wird weiterhin separat über „Zahlung erfassen" gebucht.
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
  const TEMPLATE_LABELS: Record<string, string> = {
    default: "Standard",
    modern: "Modern",
    klassisch: "Klassisch",
    minimal: "Minimal",
  };
  const tplLabel = (name: string) => TEMPLATE_LABELS[name] ?? name;
  let templateOptions = $derived(templates.filter((t) => t.name !== "default").map((t) => t.name));

  let items = $state<FormItem[]>([
    emptyItem(1),
  ]);

  function emptyItem(position: number): FormItem {
    return {
      position,
      description: "",
      quantity: 1,
      unitCode: "C62",
      unitPriceCents: 0,
      taxRatePercent: 0,
      taxCategoryCode: "E" as TaxCategoryCode,
      descriptionMarkup: "",
      descriptionTitle: "",
    };
  }

  // ---- §19-Hardline: bei is_kleinunternehmer USt-Felder sperren ----
  let isKlein = $derived(seller?.isKleinunternehmer === 1);
  // §19: E/0 erzwingen; Regelbesteuerung: E wird S 19 % (KB-0014). In place
  // (lib/tax.ts) — eine neue Array-Identität triggerte den Effect endlos.
  $effect(() => {
    applyTaxRules(items as unknown as InvoiceItemInput[], seller ? isKlein : null);
  });

  // ---- Totals (live, wie der Server: USt je Steuergruppe) ----
  let totals = $derived(documentTotals(items.map(toInput)));

  // ---- Feldfehler (spiegeln domain::invoice; der Server prüft erneut) ----
  let errContact = $derived(contactId ? "" : "Bitte einen Empfänger wählen.");
  let errInvoiceDate = $derived(
    !invoiceDate
      ? "Bitte das Rechnungsdatum angeben."
      : invoiceDate > today
        ? "Das Rechnungsdatum darf nicht in der Zukunft liegen."
        : "",
  );
  let errDeliveryDate = $derived(
    !deliveryDate
      ? "Bitte das Leistungsdatum angeben."
      : deliveryDate > (invoiceDate || today)
        ? "Das Leistungsdatum darf nicht nach dem Rechnungsdatum liegen."
        : "",
  );
  let errDueDate = $derived(
    dueDate && invoiceDate && dueDate < invoiceDate
      ? "Die Fälligkeit darf nicht vor dem Rechnungsdatum liegen."
      : "",
  );
  function itemInvalid(it: FormItem): boolean {
    return (
      labelMissing(it) ||
      it.quantity == null ||
      it.unitPriceCents == null ||
      (!isKlein && it.taxRatePercent == null)
    );
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
    if (!d) throw new Error("Rechnung nicht gefunden.");
    const inv = d.invoice;
    if (inv.lockedAt || inv.voidedAt || inv.isStornoFor || inv.status !== "draft") {
      flash("Nur offene Rechnungs-Entwürfe lassen sich bearbeiten.", "error");
      goto(`/invoices/${id}`);
      return;
    }
    editNumber = inv.invoiceNumber;
    contactId = inv.contactId;
    if (!contacts.some((c) => c.id === inv.contactId) && d.buyer) contacts = [...contacts, d.buyer];
    deliveryDateTouched = true; // gespeicherten Wert nicht überschreiben
    invoiceDate = inv.invoiceDate.slice(0, 10);
    deliveryDate = inv.deliveryDate?.slice(0, 10) ?? "";
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

  function removeItem(idx: number) {
    items = items.filter((_, i) => i !== idx).map((it, i) => ({ ...it, position: i + 1 }));
  }

  /** Fokus auf das erste als ungültig markierte Feld des Formulars. */
  async function focusFirstInvalid(): Promise<void> {
    await tick();
    const el = document.querySelector<HTMLElement>('#invoice-form [aria-invalid="true"]');
    el?.focus();
  }

  async function save() {
    submitted = true;
    if (
      errContact || errInvoiceDate || errDeliveryDate || errDueDate ||
      items.some(itemInvalid)
    ) {
      await focusFirstInvalid();
      return;
    }
    if (totals.gross <= 0) {
      flash("Der Gesamtbetrag muss größer als 0 € sein.", "error");
      return;
    }
    saving = true;
    try {
      const args: CreateDraftArgs = {
        contactId,
        buyerReference: buyerReference.trim() || "N/A",
        input: {
          direction: "issued",
          invoiceDate,
          deliveryDate: deliveryDate || null,
          dueDate: dueDate || null,
          currencyCode: currency,
          items: items.map(toInput),
          notes: notes.trim() || null,
          paymentNote: alreadyPaid ? (paymentNote.trim() || null) : null,
          pdfTemplate,
          isStornoFor: null,
          cancelReason: null,
        },
      };
      const detail = editId ? await invoicesUpdateDraft(editId, args) : await invoicesCreateDraft(args);
      flash(editId ? "Entwurf geändert." : "Entwurf gespeichert.");
      goto(`/invoices/${detail.invoice.id}`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      saving = false;
    }
  }

  function addDays(iso: string, days: number): string {
    return addDaysIso(iso, days);
  }
</script>

<PageBar
  back={editId ? `/invoices/${editId}` : "/invoices"}
  backLabel={editId ? "Entwurf" : "Liste"}
  title={editId ? `Entwurf ${editNumber ?? ""} bearbeiten` : "Neue Rechnung"}
>
  {#snippet actions()}
    <a href={editId ? `/invoices/${editId}` : "/invoices"} class="btn-secondary btn-sm">Abbrechen</a>
    <button type="submit" form="invoice-form" class="btn-primary btn-sm" disabled={saving}>
      {saving ? "Speichere …" : editId ? "Änderungen speichern" : "Als Entwurf speichern"}
    </button>
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if !seller}
  <Banner kind="warning">
    Deine Firmendaten fehlen. <a class="txt-link" href="/settings/seller">Jetzt ausfüllen</a>.
  </Banner>
{:else}
  {#if isKlein}
    <Banner kind="info">
      Du bist als Kleinunternehmer (§19) eingestellt — auf der Rechnung wird keine
      Mehrwertsteuer ausgewiesen. Die MwSt-Felder sind deshalb ausgeblendet.
    </Banner>
  {/if}

  <form id="invoice-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
    <p class="req-legend">Felder mit <span aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
    <section class="card">
      <h2>Allgemein</h2>
      <div class="grid">
        <label>
          <span>Empfänger <span class="req" aria-hidden="true">*</span></span>
          <select
            bind:value={contactId}
            required
            aria-invalid={submitted && errContact ? "true" : undefined}
            aria-describedby={submitted && errContact ? "err-contact" : undefined}
          >
            <option value="" disabled>— wählen —</option>
            {#each contacts as c (c.id)}
              <option value={c.id}>{c.name}{c.city ? ` (${c.city})` : ""}</option>
            {/each}
          </select>
          {#if submitted && errContact}<span id="err-contact" class="field-err">{errContact}</span>{/if}
        </label>
        <label>
          <span>Rechnungsdatum <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            bind:value={invoiceDate}
            max={today}
            required
            aria-invalid={submitted && errInvoiceDate ? "true" : undefined}
            aria-describedby={submitted && errInvoiceDate ? "err-invoice-date" : undefined}
          />
          {#if submitted && errInvoiceDate}<span id="err-invoice-date" class="field-err">{errInvoiceDate}</span>{/if}
        </label>
        <label>
          <span>Leistungsdatum <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            bind:value={deliveryDate}
            oninput={() => (deliveryDateTouched = true)}
            max={invoiceDate || today}
            required
            aria-invalid={submitted && errDeliveryDate ? "true" : undefined}
            aria-describedby={submitted && errDeliveryDate ? "err-delivery-date" : undefined}
          />
          {#if submitted && errDeliveryDate}<span id="err-delivery-date" class="field-err">{errDeliveryDate}</span>{/if}
        </label>
        <label>
          Fälligkeit
          <input
            type="date"
            bind:value={dueDate}
            min={invoiceDate || undefined}
            aria-invalid={submitted && errDueDate ? "true" : undefined}
            aria-describedby={submitted && errDueDate ? "err-due-date" : undefined}
          />
          {#if submitted && errDueDate}<span id="err-due-date" class="field-err">{errDueDate}</span>{/if}
        </label>
        <label>
          Währung
          <input type="text" bind:value={currency} maxlength="3" autocomplete="off" autocapitalize="characters" spellcheck="false" />
        </label>
        <label>
          PDF-Vorlage
          <select bind:value={pdfTemplate}>
            <option value="default">Wie in den Einstellungen ({tplLabel(seller.defaultPdfTemplate)})</option>
            {#each templateOptions as name (name)}
              <option value={name}>{tplLabel(name)}</option>
            {/each}
            {#if pdfTemplate !== "default" && !templateOptions.includes(pdfTemplate)}
              <option value={pdfTemplate}>{tplLabel(pdfTemplate)}</option>
            {/if}
          </select>
        </label>
        <label>
          Referenz (nur für Behörden)
          <input type="text" bind:value={buyerReference} autocomplete="off" spellcheck="false" aria-describedby="hint-buyer-ref" />
          <span id="hint-buyer-ref" class="field-hint">Leitweg-ID bei öffentlichen Auftraggebern, sonst „N/A“.</span>
        </label>
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
            <th scope="col">Beschreibung</th>
            <th scope="col" class="right">Menge</th>
            <th scope="col">Einheit</th>
            <th scope="col" class="right">Einzelpreis</th>
            {#if !isKlein}
              <th scope="col" class="right">USt-Satz</th>
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
            {#if isRich(it)}
              {@const linked = !!it.sourcePackageId}
              <tr class="pkg-row">
                <td data-label="Pos.">{n}</td>
                <td class="cards-full" data-label="Titel">
                  {#if linked}
                    <span class="pkg-badge">aus Paket: {it.packageName ?? "Paket"} (V{it.sourcePackageRevision})</span>
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
                  <td data-label="Einheit">{it.unitCode}</td>
                  <td class="right" data-label="Einzelpreis">{euro(it.unitPriceCents ?? 0)}</td>
                {:else}
                  <td class="c-qty" data-label="Menge">
                    <QuantityInput bind:value={it.quantity} ariaLabel={`Menge, Position ${n}`} required showErrors={submitted} />
                  </td>
                  <td class="c-unit" data-label="Einheit">
                    <input type="text" bind:value={it.unitCode} maxlength="3" aria-label={`Einheit, Position ${n} (Code, z. B. C62 für Stück)`} autocomplete="off" autocapitalize="characters" spellcheck="false" />
                  </td>
                  <td class="c-price" data-label="Einzelpreis">
                    <MoneyInput bind:cents={it.unitPriceCents} ariaLabel={`Einzelpreis, Position ${n}`} required showErrors={submitted} />
                  </td>
                {/if}
                {#if !isKlein}
                  <!-- Regelbesteuerung: Satz und Steuerart je Position (KB-0047); sie gehören
                       nicht zum Paket-Inhalt, die Provenienz bleibt. -->
                  <td class="c-rate" data-label="USt-Satz">
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
                    <div class="pkg-actions">
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
                <td class="cards-full" data-label="Beschreibung">
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
                <td class="c-qty" data-label="Menge">
                  <QuantityInput bind:value={it.quantity} ariaLabel={`Menge, Position ${n}`} required showErrors={submitted} />
                </td>
                <td class="c-unit" data-label="Einheit">
                  <input type="text" bind:value={it.unitCode} maxlength="3" aria-label={`Einheit, Position ${n} (Code, z. B. C62 für Stück)`} autocomplete="off" autocapitalize="characters" spellcheck="false" />
                </td>
                <td class="c-price" data-label="Einzelpreis">
                  <MoneyInput bind:cents={it.unitPriceCents} ariaLabel={`Einzelpreis, Position ${n}`} required showErrors={submitted} />
                </td>
                {#if !isKlein}
                  <td class="c-rate" data-label="USt-Satz">
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
          <tr class="grand">
            <td colspan={isKlein ? 5 : 7} class="right sum-label"><strong>Brutto</strong></td>
            <td class="right" data-label="Brutto"><strong>{euro(totals.gross)}</strong></td>
            <td class="c-actions"></td>
          </tr>
        </tfoot>
      </table>
      <TravelLineAdder onAdd={addTravelLine} />
      <PackageItemAdder onAdd={addPackage} />
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
          verbucht. Erfasse sie bei Bedarf zusätzlich über „Zahlung erfassen".
        </p>
        <div class="paid-chips" role="group" aria-label="Textvorschläge für den Bezahlt-Hinweis">
          {#each paymentTemplates as t (t)}
            <button type="button" class="chip" onclick={() => (paymentNote = t)}>{t}</button>
          {/each}
        </div>
        <label class="paid-note">
          Text des Hinweises auf dem Beleg
          <textarea rows="2" bind:value={paymentNote}></textarea>
        </label>
      {/if}
    </section>

    <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end"><button type="submit" class="btn-primary" disabled={saving}>{saving ? "Speichere …" : editId ? "Änderungen speichern" : "Als Entwurf speichern"}</button></div>
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .req-legend { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .field-err { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  /* U1 (2.5.8): ganze Zeile als Trefferfläche, mindestens --target hoch. */
  .paid-toggle { display: flex; flex-direction: row; align-items: center; gap: 0.5rem; color: var(--text); min-height: var(--target); cursor: pointer; }
  .paid-hint { color: var(--text-muted); line-height: 1.5; margin: 0.6rem 0; }
  .paid-chips { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.6rem; }
  .chip {
    cursor: pointer; font: inherit; font-size: var(--fs-sm); min-height: var(--target);
    padding: 4px 12px; border: 1px solid var(--line-control); border-radius: var(--r-pill);
    background: var(--surface-card); color: var(--accent-on-soft); text-align: left;
  }
  @media (hover: hover) {
    .chip:hover { background: var(--surface-hover); }
  }
  .card-hdr { display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3); flex-wrap: wrap; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; }
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
  td.c-unit { width: 5.5rem; }
  td.c-unit input { width: 100%; }
  td.c-price { width: 9.5rem; }
  td.c-rate { width: 7rem; }
  tr.grand td { border-top: 2px solid var(--text); }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): überschrieb
     padding/radius/font/color der globalen .btn-*-Klassen aus tokens.css
     und brach app-weite Button-Konsistenz. Die globalen Tokens greifen jetzt. */
  .muted { color: var(--text-muted); }
  /* Links im Fließtext unterstrichen (1.4.1). */
  .txt-link { text-decoration: underline; text-underline-offset: 2px; }
  .pkg-badge { display: inline-block; font-size: var(--fs-xs); font-weight: 600; color: var(--status-info-fg); background: var(--status-info-bg); border: 1px solid var(--status-info-line); border-radius: var(--r-pill); padding: 0.05rem 0.5rem; }
  .pkg-desc { font-size: var(--fs-sm); color: var(--text-muted); margin-top: 0.2rem; white-space: pre-line; overflow-wrap: anywhere; }
  .pkg-title { font-weight: 600; margin-top: 0.25rem; overflow-wrap: anywhere; }
  .pkg-actions { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center; justify-content: flex-end; }
  .pkg-row td { background: var(--surface-sunken); border-bottom: 0; }
  .pkg-detail td { background: var(--surface-sunken); padding-top: 0; }
  /* Plan W10 (Karten unter 820 px): die Summen-Beschriftung steht im
     data-label der Betragszelle; Paket-Kopf und -Beschreibung bilden eine Karte.
     Leere Aktionszellen (eine einzige Position) erscheinen nicht als Kartenzeile. */
  @media (max-width: 820px) {
    td.sum-label { display: none; }
    td.c-qty, td.c-unit, td.c-price, td.c-rate { width: auto; }
    tr.pkg-row { margin-bottom: 0; border-bottom: 0; border-radius: var(--r-lg) var(--r-lg) 0 0; }
    tr.pkg-detail { border-top: 0; border-radius: 0 0 var(--r-lg) var(--r-lg); }
  }
</style>
