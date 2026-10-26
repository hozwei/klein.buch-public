<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import PageBar from "#lib/PageBar.svelte";
  import {
    contactsList,
    sellerProfileGet,
    recurringInvoicesCreate,
    recurringInvoicesUpdate,
    recurringInvoicesGet,
  } from "#lib/api.ts";
  import type {
    Contact,
    Frequency,
    InvoiceItemInput,
    SellerProfile,
    TaxCategoryCode,
    TravelLine,
    MaterializedPackageItem,
    RecurringInvoiceInputDto,
    RecurringInvoiceMode,
  } from "#lib/types.ts";
  import { FREQUENCIES } from "#lib/labels.ts";
  import { euro, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { applyTaxRules, documentTotals, REGEL_TAX_CODES, roundHalfAwayFromZero } from "#lib/tax.ts";
  import Banner from "#lib/Banner.svelte";
  import TravelLineAdder from "#lib/TravelLineAdder.svelte";
  import PackageItemAdder from "#lib/PackageItemAdder.svelte";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  // Wie in invoices/new: Form-Position = InvoiceItemInput + Markup/Titel als
  // String (für bind:value) + transienter Paket-Name fürs Badge. Menge, Preis
  // und Satz sind `null`, solange das Feld leer oder ungültig ist (U1).
  type FormItem = Omit<InvoiceItemInput, "quantity" | "unitPriceCents" | "taxRatePercent"> & {
    quantity: number | null;
    unitPriceCents: number | null;
    taxRatePercent: number | null;
    descriptionMarkup: string;
    descriptionTitle: string;
    packageName?: string;
  };

  function isRich(it: FormItem): boolean {
    return !!(it.descriptionTitle || it.descriptionMarkup);
  }
  function labelMissing(it: FormItem): boolean {
    return isRich(it) ? !(it.descriptionTitle ?? "").trim() : !it.description.trim();
  }
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

  let editId = $state<string | null>(null);
  let loaded = $state(false);
  let busy = $state(false);
  // Nach dem ersten Speicher-Versuch zeigen alle Felder ihre Meldungen.
  let submitted = $state(false);

  let seller = $state<SellerProfile | null>(null);
  let customers = $state<Contact[]>([]);

  // Kopf
  let label = $state("");
  let contactId = $state<string>("");
  let frequency = $state<Frequency>("monthly");
  let dayOfPeriod = $state<number | null>(1);
  let nextDueDate = $state(todayBerlin());
  let endDate = $state<string>("");
  let paymentTermsDays = $state<number | null>(14);
  let autoMode = $state<RecurringInvoiceMode>("draft");
  let servicePeriodNote = $state(true);
  let pdfTemplate = $state("default");

  let items = $state<FormItem[]>([emptyItem(1)]);

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

  // §19-Hardline: bei Kleinunternehmer USt-Felder sperren + auf E/0 zwingen.
  let isKlein = $derived(seller?.isKleinunternehmer === 1);
  // Regelbesteuerung: E wird S 19 % (KB-0014; lib/tax.ts).
  $effect(() => {
    applyTaxRules(items as unknown as InvoiceItemInput[], seller ? isKlein : null);
  });

  // Wie der Server: USt je Steuergruppe aus der Nettosumme (BR-CO-17).
  let totals = $derived(documentTotals(items.map(toInput)));

  // ---- Feldfehler (der Server prüft erneut) ----
  let errLabel = $derived(label.trim() ? "" : "Bitte eine Bezeichnung eingeben.");
  let errContact = $derived(contactId ? "" : "Bitte einen Kunden auswählen.");
  let errEndDate = $derived(
    endDate && endDate < nextDueDate ? "Das Laufzeit-Ende liegt vor der nächsten Fälligkeit." : "",
  );
  function itemInvalid(it: FormItem): boolean {
    return (
      labelMissing(it) ||
      it.quantity == null ||
      it.unitPriceCents == null ||
      (!isKlein && it.taxRatePercent == null)
    );
  }
  let dayRef: QuantityInput | undefined = $state();
  let termsRef: QuantityInput | undefined = $state();

  onMount(async () => {
    try {
      [seller, customers] = await Promise.all([
        sellerProfileGet(),
        contactsList(false).then((all) =>
          all.filter((c) => c.contactType === "customer" || c.contactType === "both"),
        ),
      ]);
      // Layout kommt aus den Einstellungen (Standard-Layout) — kein Feld im Formular.
      pdfTemplate = seller?.defaultPdfTemplate || "default";

      const id = page.url.searchParams.get("id");
      if (id) {
        editId = id;
        const d = await recurringInvoicesGet(id);
        if (!d) {
          flash("Vorlage nicht gefunden.", "error");
        } else {
          const t = d.template;
          label = t.label;
          contactId = t.contactId;
          frequency = t.frequency as Frequency;
          dayOfPeriod = t.dayOfPeriod;
          nextDueDate = t.nextDueDate.slice(0, 10);
          endDate = t.endDate ? t.endDate.slice(0, 10) : "";
          paymentTermsDays = t.paymentTermsDays;
          autoMode = (t.autoMode as RecurringInvoiceMode) ?? "draft";
          servicePeriodNote = t.servicePeriodNote === 1;
          pdfTemplate = t.pdfTemplate || "default";
          items = d.items.map((it, idx) => ({
            position: idx + 1,
            description: it.description,
            quantity: it.quantity,
            unitCode: it.unitCode,
            unitPriceCents: it.unitPriceCents,
            taxRatePercent: it.taxRatePercent,
            taxCategoryCode: it.taxCategoryCode as TaxCategoryCode,
            descriptionMarkup: it.descriptionMarkup ?? "",
            descriptionTitle: it.descriptionTitle ?? "",
            sourcePackageId: it.sourcePackageId,
            sourcePackageRevision: it.sourcePackageRevision,
          }));
          if (items.length === 0) items = [emptyItem(1)];
        }
      }
    } catch (e) {
      flash("Laden fehlgeschlagen: " + String(e), "error");
    } finally {
      loaded = true;
    }
  });

  function onCustomerSelect() {
    const c = customers.find((v) => v.id === contactId);
    if (c && !label.trim()) label = c.name;
  }

  function addItem() {
    items = [...items, emptyItem(items.length + 1)];
  }
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
  function detachPackage(idx: number) {
    const it = items[idx];
    it.sourcePackageId = null;
    it.sourcePackageRevision = null;
    it.packageName = undefined;
  }
  function removeItem(idx: number) {
    items = items.filter((_, i) => i !== idx).map((it, i) => ({ ...it, position: i + 1 }));
    if (items.length === 0) items = [emptyItem(1)];
  }

  /** Fokus auf das erste als ungültig markierte Feld des Formulars. */
  async function focusFirstInvalid(): Promise<void> {
    await tick();
    const el = document.querySelector<HTMLElement>('#ri-form [aria-invalid="true"]');
    el?.focus();
  }

  async function save() {
    submitted = true;
    const dayOk = dayRef?.validate() ?? true;
    const termsOk = termsRef?.validate() ?? true;
    if (
      errLabel || errContact || errEndDate || !dayOk || !termsOk ||
      dayOfPeriod == null || dayOfPeriod < 1 || paymentTermsDays == null ||
      items.some(itemInvalid)
    ) {
      await focusFirstInvalid();
      return;
    }
    if (totals.gross <= 0) return flash("Der Gesamtbetrag muss größer als 0 € sein.", "error");

    const input: RecurringInvoiceInputDto = {
      label: label.trim(),
      contactId,
      frequency,
      dayOfPeriod: dayOfPeriod ?? 1,
      nextDueDate,
      startDate: null,
      endDate: endDate || null,
      autoMode,
      paymentTermsDays: paymentTermsDays ?? 0,
      pdfTemplate: pdfTemplate || "default",
      servicePeriodNote,
      notes: null,
      items: items.map(toInput),
    };

    busy = true;
    try {
      if (editId) {
        await recurringInvoicesUpdate(editId, input);
        flash("Vorlage gespeichert.");
      } else {
        await recurringInvoicesCreate(input);
        flash("Vorlage angelegt.");
      }
      await goto("/recurring-invoices");
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/recurring-invoices" backLabel="Abo-Rechnungen" title={editId ? "Vorlage bearbeiten" : "Neue Vorlage"}>
  {#snippet actions()}
    {#if loaded}
      <a class="btn-secondary" href="/recurring-invoices">Abbrechen</a>
      <button type="submit" form="ri-form" class="btn-primary" disabled={busy}>
        {busy ? "Speichere …" : editId ? "Speichern" : "Vorlage anlegen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if !loaded}
  <p class="muted">Lade …</p>
{:else}
  {#if isKlein}
    <Banner kind="info">
      Kleinunternehmer (§19): auf den erzeugten Rechnungen wird keine Umsatzsteuer
      ausgewiesen — die MwSt-Felder sind ausgeblendet.
    </Banner>
  {/if}

  <form id="ri-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
    <p class="req-legend">Felder mit <span aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
    <section class="card">
      <h2>Allgemein</h2>
      <div class="grid">
        <label class="span2">
          <span>Bezeichnung (intern) <span class="req" aria-hidden="true">*</span></span>
          <input
            type="text"
            bind:value={label}
            required
            autocomplete="off"
            aria-invalid={submitted && errLabel ? "true" : undefined}
            aria-describedby={submitted && errLabel ? "err-label" : "hint-label"}
          />
          {#if submitted && errLabel}
            <span id="err-label" class="field-err">{errLabel}</span>
          {:else}
            <span id="hint-label" class="field-hint">Zum Beispiel „Wartung Server – Müller GmbH“. Wird beim Wählen des Kunden vorbelegt.</span>
          {/if}
        </label>
        <label>
          <span>Kunde <span class="req" aria-hidden="true">*</span></span>
          <select
            bind:value={contactId}
            onchange={onCustomerSelect}
            required
            aria-invalid={submitted && errContact ? "true" : undefined}
            aria-describedby={submitted && errContact ? "err-contact" : undefined}
          >
            <option value="" disabled>— wählen —</option>
            {#each customers as c (c.id)}
              <option value={c.id}>{c.name}{c.city ? ` (${c.city})` : ""}</option>
            {/each}
          </select>
          {#if submitted && errContact}<span id="err-contact" class="field-err">{errContact}</span>{/if}
        </label>
        <label>
          Frequenz
          <select bind:value={frequency}>
            {#each FREQUENCIES as f}<option value={f.value}>{f.label}</option>{/each}
          </select>
        </label>
        <QuantityInput
          bind:this={dayRef}
          bind:value={dayOfPeriod}
          integer
          grouping={false}
          max={31}
          noun="Stichtag"
          label="Stichtag (Tag im Monat, 1–31)"
          required
          class="ri-field"
          showErrors={submitted}
          error={submitted && dayOfPeriod === 0 ? "Der Stichtag liegt zwischen 1 und 31." : undefined}
        />
        <label>
          Nächste Fälligkeit
          <!-- Neue Vorlagen beginnen nicht in der Vergangenheit (R1-S2-NEXTDUE);
               bei bestehenden schreibt Klein.Buch den Termin selbst fort. -->
          <input
            type="date"
            bind:value={nextDueDate}
            min={editId ? undefined : todayBerlin()}
            disabled={!!editId}
            aria-describedby={editId ? "hint-next-due" : undefined}
          />
          {#if editId}
            <span id="hint-next-due" class="field-hint">Schreibt Klein.Buch selbst fort. Verpasste Termine findest du in der Liste unter „Rückstand …“.</span>
          {/if}
        </label>
        <label>
          Laufzeit-Ende (optional)
          <input
            type="date"
            bind:value={endDate}
            min={nextDueDate || undefined}
            aria-invalid={submitted && errEndDate ? "true" : undefined}
            aria-describedby={submitted && errEndDate ? "err-end-date" : undefined}
          />
          {#if submitted && errEndDate}<span id="err-end-date" class="field-err">{errEndDate}</span>{/if}
        </label>
        <QuantityInput
          bind:this={termsRef}
          bind:value={paymentTermsDays}
          integer
          unit="Tage"
          noun="Zahlungsziel"
          label="Zahlungsziel"
          required
          class="ri-field"
          showErrors={submitted}
        />
      </div>
    </section>

    <section class="card">
      <header class="card-hdr">
        <h2>Positionen</h2>
        <button type="button" onclick={addItem} class="btn-secondary">+ Position</button>
      </header>
      <table class="cards items">
        <caption class="kb-visually-hidden">Positionen der Vorlage</caption>
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
                  <td class="right" data-label="USt-Satz">{percent(it.taxRatePercent ?? 0)}</td>
                  <td data-label="USt-Art">{it.taxCategoryCode}</td>
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

    <fieldset class="card modepick">
      <legend>Was soll am Stichtag passieren?</legend>
      <label class="radio" class:sel={autoMode === "draft"}>
        <input type="radio" name="autoMode" value="draft" bind:group={autoMode} />
        <span><strong>Nur Entwurf vorbereiten – ich gebe selbst frei.</strong>
          Am Stichtag wird ein Rechnungs-<em>Entwurf</em> angelegt und gemeldet. Sicherste Variante.</span>
      </label>
      <label class="radio" class:sel={autoMode === "issue"}>
        <input type="radio" name="autoMode" value="issue" bind:group={autoMode} />
        <span><strong>Automatisch erstellen – ich versende selbst.</strong>
          Die Rechnung wird automatisch <em>festgeschrieben</em> (Nummer, PDF, E-Rechnung).</span>
      </label>
      <label class="radio" class:sel={autoMode === "issue_send"}>
        <input type="radio" name="autoMode" value="issue_send" bind:group={autoMode} />
        <span><strong>Automatisch erstellen und per E-Mail senden.</strong>
          Versendet über dein Standard-Mail-Konto (Einstellungen → E-Mail). Schlägt der Versand fehl, bekommst du einen Hinweis und kannst manuell senden.</span>
      </label>
    </fieldset>

    <section class="card">
      <Toggle
        bind:checked={servicePeriodNote}
        label={"Leistungszeitraum automatisch auf die Rechnung schreiben (z. B. „Leistungszeitraum: Mai 2026“)"}
      />
      <p class="hint">
        Belegdatum jeder erzeugten Rechnung ist der Erstellungstag (rechtlich vorgegeben);
        der Leistungszeitraum steht separat. Pakete und Anfahrt fügst du oben wie bei einer
        normalen Rechnung hinzu.
      </p>
    </section>
    <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end"><button type="submit" class="btn-primary" disabled={busy}>{busy ? "Speichere …" : editId ? "Speichern" : "Vorlage anlegen"}</button></div>
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .req-legend { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .field-err { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  .card-hdr { display: flex; justify-content: space-between; align-items: center; gap: var(--sp-3); flex-wrap: wrap; }
  .span2 { grid-column: 1 / -1; }
  /* Labels der Bausteine (MoneyInput/QuantityInput) im Raster wie die übrigen. */
  .grid :global(.ri-field) { margin-bottom: 0; }
  .grid :global(.ri-field label) { color: var(--text-muted); margin-bottom: 4px; }
  .modepick { border: 1px solid var(--line); min-width: 0; }
  .modepick legend { font-size: var(--fs-base); color: var(--text); font-weight: 600; padding: 0 0.35rem; }
  label.radio { flex-direction: row; align-items: flex-start; gap: 0.6rem; padding: 0.6rem; border: 1px solid var(--line-control); border-radius: var(--r-sm); cursor: pointer; margin-top: 0.5rem; font-weight: 400; }
  label.radio span { font-size: var(--fs-sm); color: var(--text-muted); line-height: 1.45; }
  label.radio strong { color: var(--text); }
  label.radio em { font-style: normal; font-weight: 600; color: var(--accent-on-soft); }
  label.radio input { width: auto; margin-top: 0.15rem; }
  label.radio.sel { border-color: var(--accent); background: var(--accent-soft); }
  .hint { font-size: var(--fs-sm); color: var(--status-info-fg); background: var(--status-info-bg); border: 1px solid var(--status-info-line); padding: 0.5rem 0.75rem; border-radius: var(--r-sm); margin-top: 0.75rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  input, select {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text); background: var(--surface-card);
  }
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
  input:disabled { background: var(--surface-sunken); color: var(--text-muted); }
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
