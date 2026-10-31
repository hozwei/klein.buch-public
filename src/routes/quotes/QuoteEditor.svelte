<script lang="ts">
  // Angebots-Editor für „Neues Angebot“ (`/quotes/new`) und „Entwurf bearbeiten“
  // (`/quotes/[id]/edit`, ADR 0042 Nr. 13; Muster InvoiceEditor). `editId` =
  // gespeicherter Entwurf, `null` = neues Angebot. Die Nummer AN-JJJJ-NNNN bleibt
  // beim Bearbeiten; das Angebotsdatum bleibt im Geschäftsjahr der Nummer.
  //
  // Ablehnungen des Servers stehen am Feld (Angebotsdatum, Empfänger) bzw. als
  // stehendes Banner über dem Formular (mehrere Gründe, ADR 0041 Nr. 22) statt als
  // Toast. Client-Prüfungen bilden nur Server-Regeln nach.
  //
  // ADR 0042 Nr. 16 (Warteliste 8): Restore-Vormerkung aus dem Lesefeld
  // `restoreConflict`; ein bestätigter Entwurf zeigt VOR dem Speichern, dass die
  // Änderung ihn wieder vormerkt.
  import { todayBerlin, addDaysIso, daysFromTodayBerlin, yearOfIsoDate } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Badge from "#lib/Badge.svelte";
  import { goto } from "$app/navigation";
  import {
    contactsList,
    quotesCreateDraft,
    quotesGet,
    quotesUpdateDraft,
    pdfTemplatesList,
    sellerProfileGet,
    documentTermsGet,
  } from "#lib/api.ts";
  import type {
    Contact,
    QuoteCreateDraftArgs,
    PdfTemplateMeta,
    QuoteItemInput,
    SellerProfile,
    TaxCategoryCode,
    TravelLine,
    MaterializedPackageItem,
    RestoreConflictState,
  } from "#lib/types.ts";
  import RestoreConflictBanner from "#lib/RestoreConflictBanner.svelte";
  import { restoreSaveFlash } from "#lib/restoreConflict.ts";
  import { canEditQuote } from "./quoteStatus.ts";
  import { euro } from "#lib/format.ts";
  import { pdfTemplateLabel, pdfTemplateDefaultOptionLabel, unitCodeError, unitLabel, unitOptions } from "#lib/labels.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { applyTaxRules, documentTotals, REGEL_TAX_CODES, roundHalfAwayFromZero } from "#lib/tax.ts";
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
  import { relevantTaxDate } from "#lib/taxStatus.ts";
  import { taxStatusAtDate } from "#lib/taxStatusAt.svelte.ts";

  let { editId = null }: { editId?: string | null } = $props();

  // Form-Position = QuoteItemInput, aber descriptionMarkup als String (für den
  // bind:value-Editor) + transienter Paket-Name fürs Badge (Backend ignoriert ihn).
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
  function toInput(it: FormItem): QuoteItemInput {
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
  // Entwurf zum Bearbeiten nicht gefunden → eigener Zustand statt Fehler-Toast über leerem Formular.
  let notFound = $state(false);
  let saving = $state(false);
  // Nach dem ersten Speicher-Versuch zeigen alle Felder ihre Meldungen.
  let submitted = $state(false);
  // Entwurf: Nummer und Geschäftsjahr der Nummer.
  let editNumber = $state<string | null>(null);
  let draftFiscalYear = $state<number | null>(null);
  // Warteliste (8), ADR 0042 Nr. 16: Restore-Vormerkung des Entwurfs.
  let restoreConflict = $state<RestoreConflictState | null>(null);

  // Ablehnung des Servers: Gründe als Banner, einzelne Gründe am Feld (nur,
  // solange das Feld den abgelehnten Wert hat).
  let serverReasons = $state<string[]>([]);
  let serverDateErr = $state<{ value: string; msg: string } | null>(null);
  let serverContactErr = $state<{ value: string; msg: string } | null>(null);
  let serverBannerEl: HTMLDivElement | undefined = $state();

  let contactId = $state("");
  let quoteDate = $state(todayBerlin());
  // Default-Bindefrist: 30 Tage.
  let validUntil = $state(
    daysFromTodayBerlin(30),
  );
  let pdfTemplate = $state("default");
  let notes = $state("");

  // PDF-Vorlage als Auswahl (KB-0050). „default“ ist der Platzhalter für die
  // Standard-Vorlage aus den Einstellungen (Server: commands::quotes); Namen
  // zentral aus labels.ts (Warteliste (17)).
  let templateOptions = $derived(templates.filter((t) => t.name !== "default").map((t) => t.name));

  let items = $state<FormItem[]>([emptyItem(1)]);

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
  // ADR 0042 Nr. 3: Status am Angebotsdatum, nicht der von heute.
  let taxDate = $derived(relevantTaxDate(null, quoteDate, todayBerlin()));
  const taxAt = taxStatusAtDate(() => taxDate);
  let todayKlein = $derived(seller ? seller.isKleinunternehmer === 1 : null);
  let isKlein = $derived(taxAt.isKlein ?? todayKlein === true);
  // In-place Mutation pro Property (NICHT items = items.map(...)), siehe
  // invoices/new — sonst Endlos-Effect.
  // Regelbesteuerung: E wird S 19 % (KB-0014; lib/tax.ts).
  $effect(() => {
    // Nur mit dem Status genau dieses Datums (kein Umschreiben mit einem veralteten Wert).
    applyTaxRules(items as unknown as QuoteItemInput[], seller && taxAt.date === taxDate ? taxAt.isKlein : null);
  });

  // ---- Totals (live) ----
  // Wie der Server: USt je Steuergruppe aus der Nettosumme (BR-CO-17).
  let totals = $derived(documentTotals(items.map(toInput)));

  // ---- Feldfehler (spiegeln domain::quote; der Server prüft erneut) ----
  let errContact = $derived(contactId ? "" : "Bitte einen Empfänger wählen.");
  let errQuoteDate = $derived(
    !quoteDate
      ? "Bitte das Angebotsdatum angeben."
      : draftFiscalYear !== null && yearOfIsoDate(quoteDate) !== draftFiscalYear
        ? `Das Angebotsdatum muss im Geschäftsjahr ${draftFiscalYear} bleiben — die Nummer ${editNumber ?? ""} gehört zu dessen Nummernkreis. Für ein anderes Jahr den Entwurf verwerfen und neu anlegen.`
        : "",
  );
  // Angezeigte Meldungen: eigene Prüfung zuerst, sonst die Ablehnung des Servers.
  let shownQuoteDateErr = $derived(
    (submitted && errQuoteDate) ||
      (serverDateErr && serverDateErr.value === quoteDate ? serverDateErr.msg : ""),
  );
  let shownContactErr = $derived(
    serverContactErr && serverContactErr.value === contactId ? serverContactErr.msg : undefined,
  );
  let errValidUntil = $derived(
    !validUntil
      ? "Bitte angeben, bis wann das Angebot gilt."
      : quoteDate && validUntil < quoteDate
        ? "„Gültig bis“ darf nicht vor dem Angebotsdatum liegen."
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

  // V4-05: Ein Gesamtbetrag von 0 € lehnt der Server ab (domain::quote, TotalNotPositive).
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
        validUntil = addDays(quoteDate, terms.quoteValidDays);
      }
      await taxAt.load(taxDate).catch(() => {});
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loading = false;
    }
  });

  // Übernimmt einen gespeicherten Entwurf ins Formular. Nur offene Entwürfe
  // (nicht fertiggestellt, nicht verworfen) — sonst zurück zur Detailseite; der
  // Server prüft dasselbe noch einmal (ADR 0042 Nr. 13).
  async function loadDraft(id: string) {
    const d = await quotesGet(id);
    if (!d) {
      notFound = true;
      return;
    }
    const q = d.quote;
    if (!canEditQuote(q)) {
      flash("Nur offene Angebots-Entwürfe lassen sich bearbeiten.", "error");
      goto(`/quotes/${id}`, { replaceState: true });
      return;
    }
    editNumber = q.quoteNumber;
    draftFiscalYear = q.fiscalYear;
    contactId = q.contactId;
    if (!contacts.some((c) => c.id === q.contactId) && d.buyer) contacts = [...contacts, d.buyer];
    quoteDate = q.quoteDate.slice(0, 10);
    validUntil = q.validUntil.slice(0, 10);
    pdfTemplate = q.pdfTemplate;
    notes = q.notes ?? "";
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
    const el = document.querySelector<HTMLElement>('#quote-form [aria-invalid="true"]');
    el?.focus();
  }

  const BLOCKED_PREFIX = "Der Entwurf kann nicht gespeichert werden: ";

  /**
   * Ordnet eine Ablehnung des Servers zu: Angebotsdatum (Geschäftsjahr der
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
    if (msg.startsWith("Das Angebotsdatum muss im Geschäftsjahr")) {
      serverDateErr = { value: quoteDate, msg };
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
    if (errContact || errQuoteDate || errValidUntil || items.some(itemInvalid)) {
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
      const args: QuoteCreateDraftArgs = {
        contactId,
        input: {
          quoteDate,
          validUntil,
          // V1-04: nur Euro (domain::quote) — kein Eingabefeld.
          currencyCode: "EUR",
          items: items.map(toInput),
          notes: notes.trim() || null,
          pdfTemplate,
        },
      };
      const detail = editId ? await quotesUpdateDraft(editId, args) : await quotesCreateDraft(args);
      // Warteliste (8): Ein nach der Wiederherstellung schon abgeglichener Entwurf
      // ist nach jeder Änderung wieder vorgemerkt (Server, `reflag_confirmed_in`);
      // der Hinweis stand schon vor dem Speichern (RestoreConflictBanner).
      const done = restoreSaveFlash(restoreConflict, "quote", !!editId);
      flash(done.msg, done.kind);
      goto(`/quotes/${detail.quote.id}`);
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
  const sellerNext = $derived(editId ? `/quotes/${editId}/edit` : "/quotes/new");
</script>

<PageBar
  back={editId ? `/quotes/${editId}` : "/quotes"}
  backLabel={editId ? "Angebot" : "Angebote"}
  title={editId ? (editNumber ? `Entwurf ${editNumber} bearbeiten` : "Entwurf bearbeiten") : "Neues Angebot"}
>
  {#snippet actions()}
    {#if !notFound}
    <a href={editId ? `/quotes/${editId}` : "/quotes"} class="btn-secondary">Abbrechen</a>
    <!-- V2-06: während des Speicherns fokussierbar (aria-disabled), Klick ohne Wirkung. -->
    <Button type="submit" form="quote-form" ariaDisabled={saving}>{saveLabel}</Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if notFound}
  <Banner kind="error" href="/quotes" linkLabel="Zu den Angeboten">
    Dieses Angebot gibt es nicht (mehr). Prüfe den Link oder öffne das Angebot aus der Liste.
  </Banner>
{:else if !seller}
  <Banner kind="warning">
    Deine Firmendaten fehlen. <a class="kb-link" href={`/settings/seller?next=${sellerNext}`}>Jetzt ausfüllen</a>.
  </Banner>
{:else}
  <TaxStatusNotice isKlein={taxAt.isKlein} day={taxAt.date} {todayKlein} dateLabel="Angebotsdatum" noun="auf dem Angebot" />
  {#if editId && editNumber}
    <RestoreConflictBanner conflict={restoreConflict} number={editNumber} doc="quote" where="editor" />
  {/if}

  <form id="quote-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
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
          id="quote-contact"
          required
          requiredMessage="Bitte einen Empfänger wählen."
          showErrors={submitted}
          error={shownContactErr}
          class="span-all kb-w-long"
        />
        <label>
          <span>Angebotsdatum <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            class="kb-w-date"
            bind:value={quoteDate}
            required
            aria-invalid={shownQuoteDateErr ? "true" : undefined}
            aria-describedby={shownQuoteDateErr ? "err-quote-date" : draftFiscalYear !== null ? "hint-quote-date" : undefined}
          />
          {#if shownQuoteDateErr}
            <span id="err-quote-date" class="field-err">{shownQuoteDateErr}</span>
          {:else if draftFiscalYear !== null}
            <span id="hint-quote-date" class="field-hint">Im Geschäftsjahr {draftFiscalYear}, zu dem die Nummer gehört.</span>
          {/if}
        </label>
        <label>
          <span>Gültig bis <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            class="kb-w-date"
            bind:value={validUntil}
            min={quoteDate || undefined}
            required
            aria-invalid={submitted && errValidUntil ? "true" : undefined}
            aria-describedby={submitted && errValidUntil ? "err-valid-until" : undefined}
          />
          {#if submitted && errValidUntil}<span id="err-valid-until" class="field-err">{errValidUntil}</span>{/if}
        </label>
        <!-- V1-04: Währung fest (nur Euro), kein Eingabefeld. -->
        <div class="fixed-field">
          <span class="fixed-lbl">Währung</span>
          <span class="fixed-val">Euro (EUR)</span>
        </div>
        <label class="span-all">
          PDF-Vorlage
          <select class="kb-w-medium" bind:value={pdfTemplate}>
            <option value="default">{pdfTemplateDefaultOptionLabel(seller.defaultPdfTemplate)}</option>
            {#each templateOptions as name (name)}
              <option value={name}>{pdfTemplateLabel(name)}</option>
            {/each}
            {#if pdfTemplate !== "default" && !templateOptions.includes(pdfTemplate)}
              <option value={pdfTemplate}>{pdfTemplateLabel(pdfTemplate)}</option>
            {/if}
          </select>
        </label>
      </div>
    </section>

    <section class="card">
      <header class="card-hdr">
        <h2>Positionen</h2>
        <button type="button" onclick={addItem} class="btn-secondary">+ Position</button>
      </header>
      <table class="cards items">
        <caption class="kb-visually-hidden">Positionen des Angebots</caption>
        <thead>
          <tr>
            <th scope="col">#</th>
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
                  <!-- Regelbesteuerung: Satz und Steuerart je Position wie im Rechnungs-Editor
                       (KB-0047); sie gehören nicht zum Paket-Inhalt, die Provenienz bleibt. -->
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
      <h2 id="quote-notes-h">Notiz</h2>
      <textarea id="quote-notes" rows="3" bind:value={notes} aria-labelledby="quote-notes-h" aria-describedby="hint-quote-notes"></textarea>
      <p id="hint-quote-notes" class="field-hint">Zum Beispiel, was im Angebot enthalten ist oder bis wann es gilt.</p>
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
  .card-hdr { display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3); flex-wrap: wrap; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; align-items: start; }
  /* Empfänger und Vorlage über die volle Zeile, Felder so breit wie ihr Inhalt (U3-031). */
  .grid > :global(.span-all), .grid > .span-all { grid-column: 1 / -1; }
  .grid :global(.contact-select) { margin-bottom: 0; }
  .grid :global(.contact-select > label) { color: var(--text-muted); }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
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
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): überschrieb
     padding/radius/font/color der globalen .btn-*-Klassen aus tokens.css
     und brach app-weite Button-Konsistenz. Die globalen Tokens greifen jetzt. */
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
