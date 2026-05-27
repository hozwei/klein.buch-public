<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import PageBar from "$lib/PageBar.svelte";
  import {
    contactsList,
    sellerProfileGet,
    recurringInvoicesCreate,
    recurringInvoicesUpdate,
    recurringInvoicesGet,
  } from "$lib/api";
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
  } from "$lib/types";
  import { FREQUENCIES } from "$lib/labels";
  import { euro } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import TravelLineAdder from "$lib/TravelLineAdder.svelte";
  import PackageItemAdder from "$lib/PackageItemAdder.svelte";
  import MarkdownEditor from "$lib/MarkdownEditor.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import { flash } from "$lib/toast.svelte";

  // Wie in invoices/new: Form-Position = InvoiceItemInput + Markup/Titel als
  // String (für bind:value) + transienter Paket-Name fürs Badge.
  type FormItem = InvoiceItemInput & {
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
    return !isRich(it) && !it.description.trim() && it.unitPriceCents === 0;
  }

  let editId = $state<string | null>(null);
  let loaded = $state(false);
  let busy = $state(false);

  let seller = $state<SellerProfile | null>(null);
  let customers = $state<Contact[]>([]);

  // Kopf
  let label = $state("");
  let contactId = $state<string>("");
  let frequency = $state<Frequency>("monthly");
  let dayOfPeriod = $state<number>(1);
  let nextDueDate = $state(new Date().toISOString().slice(0, 10));
  let endDate = $state<string>("");
  let paymentTermsDays = $state<number>(14);
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
  $effect(() => {
    if (!isKlein) return;
    for (const it of items) {
      if (it.taxRatePercent !== 0) it.taxRatePercent = 0;
      if (it.taxCategoryCode !== "E") it.taxCategoryCode = "E" as TaxCategoryCode;
    }
  });

  let totals = $derived.by(() => {
    let net = 0, tax = 0;
    for (const it of items) {
      const itNet = Math.round(it.quantity * it.unitPriceCents);
      net += itNet;
      tax += Math.round((itNet * it.taxRatePercent) / 100);
    }
    return { net, tax, gross: net + tax };
  });

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

      const id = $page.url.searchParams.get("id");
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

  function eurosFromInput(v: number): number {
    return Math.round(v * 100);
  }

  async function save() {
    if (!label.trim()) return flash("Bezeichnung ist erforderlich.", "error");
    if (!contactId) return flash("Bitte einen Kunden auswählen.", "error");
    if (dayOfPeriod < 1 || dayOfPeriod > 31) return flash("Stichtag muss zwischen 1 und 31 liegen.", "error");
    if (paymentTermsDays < 0) return flash("Zahlungsziel darf nicht negativ sein.", "error");
    if (endDate && endDate < nextDueDate) return flash("Laufzeit-Ende liegt vor dem nächsten Stichtag.", "error");
    if (items.some(labelMissing)) return flash("Jede Position braucht eine Beschreibung bzw. einen Titel.", "error");
    if (totals.gross <= 0) return flash("Gesamtbetrag muss größer als 0 sein.", "error");

    const input: RecurringInvoiceInputDto = {
      label: label.trim(),
      contactId,
      frequency,
      dayOfPeriod,
      nextDueDate,
      startDate: null,
      endDate: endDate || null,
      autoMode,
      paymentTermsDays,
      pdfTemplate: pdfTemplate || "default",
      servicePeriodNote,
      notes: null,
      items,
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
    <section class="card">
      <h2>Allgemein</h2>
      <div class="grid">
        <label class="span2">
          Bezeichnung (intern)
          <input type="text" bind:value={label} placeholder="z. B. Wartung Server – Müller GmbH" />
        </label>
        <label>
          Kunde *
          <select bind:value={contactId} onchange={onCustomerSelect} required>
            <option value="" disabled>— wählen —</option>
            {#each customers as c (c.id)}
              <option value={c.id}>{c.name}{c.city ? ` (${c.city})` : ""}</option>
            {/each}
          </select>
        </label>
        <label>
          Frequenz
          <select bind:value={frequency}>
            {#each FREQUENCIES as f}<option value={f.value}>{f.label}</option>{/each}
          </select>
        </label>
        <label>
          Stichtag (Tag im Monat, 1–31)
          <input type="number" min="1" max="31" bind:value={dayOfPeriod} />
        </label>
        <label>
          Nächste Fälligkeit
          <input type="date" bind:value={nextDueDate} />
        </label>
        <label>
          Laufzeit-Ende (optional)
          <input type="date" bind:value={endDate} />
        </label>
        <label>
          Zahlungsziel (Tage)
          <input type="number" min="0" bind:value={paymentTermsDays} />
        </label>
      </div>
    </section>

    <section class="card">
      <header class="card-hdr">
        <h2>Positionen</h2>
        <button type="button" onclick={addItem} class="btn-secondary">+ Position</button>
      </header>
      <table>
        <thead>
          <tr>
            <th>#</th>
            <th>Beschreibung</th>
            <th>Menge</th>
            <th>Einheit</th>
            <th>Einzelpreis (€)</th>
            {#if !isKlein}
              <th>USt-Satz %</th>
              <th>USt-Art</th>
            {/if}
            <th>Netto</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each items as it, i (i)}
            {#if isRich(it)}
              {@const linked = !!it.sourcePackageId}
              <tr class="pkg-row">
                <td>{it.position}</td>
                <td>
                  {#if linked}
                    <span class="pkg-badge">aus Paket: {it.packageName ?? "Paket"} (V{it.sourcePackageRevision})</span>
                    <div class="pkg-title">{it.descriptionTitle}</div>
                  {:else}
                    <input type="text" bind:value={it.descriptionTitle} placeholder="Titel der Position" required />
                  {/if}
                </td>
                {#if linked}
                  <td class="right">{it.quantity}</td>
                  <td>{it.unitCode}</td>
                  <td class="right">{euro(it.unitPriceCents)}</td>
                {:else}
                  <td><input type="number" step="any" min="0" bind:value={it.quantity} required /></td>
                  <td><input type="text" bind:value={it.unitCode} placeholder="C62" maxlength="3" /></td>
                  <td>
                    <input type="number" step="0.01" value={it.unitPriceCents / 100}
                      oninput={(e) => { it.unitPriceCents = eurosFromInput(+(e.target as HTMLInputElement).value); }} required />
                  </td>
                {/if}
                {#if !isKlein}
                  <td class="right">{it.taxRatePercent}</td>
                  <td>{it.taxCategoryCode}</td>
                {/if}
                <td class="right">{euro(Math.round(it.quantity * it.unitPriceCents))}</td>
                <td>
                  <div class="pkg-actions">
                    {#if linked}
                      <button type="button" class="btn-secondary" onclick={() => detachPackage(i)}>Paket anpassen</button>
                    {/if}
                    {#if items.length > 1}
                      <button type="button" class="btn-danger" onclick={() => removeItem(i)}>×</button>
                    {/if}
                  </div>
                </td>
              </tr>
              <tr class="pkg-detail">
                <td colspan="99">
                  {#if linked}
                    <div class="pkg-desc">{it.description}</div>
                  {:else}
                    <MarkdownEditor bind:value={it.descriptionMarkup} rows={5} placeholder="Beschreibung (Markdown) — erscheint formatiert auf dem PDF" />
                  {/if}
                </td>
              </tr>
            {:else}
              <tr>
                <td>{it.position}</td>
                <td><input type="text" bind:value={it.description} required /></td>
                <td><input type="number" step="any" min="0" bind:value={it.quantity} required /></td>
                <td><input type="text" bind:value={it.unitCode} placeholder="C62" maxlength="3" /></td>
                <td>
                  <input type="number" step="0.01" value={it.unitPriceCents / 100}
                    oninput={(e) => { it.unitPriceCents = eurosFromInput(+(e.target as HTMLInputElement).value); }} required />
                </td>
                {#if !isKlein}
                  <td><input type="number" step="0.01" bind:value={it.taxRatePercent} /></td>
                  <td>
                    <select bind:value={it.taxCategoryCode}>
                      <option value="S">S</option>
                      <option value="Z">Z</option>
                      <option value="E">E</option>
                      <option value="AE">AE</option>
                      <option value="K">K</option>
                      <option value="G">G</option>
                      <option value="O">O</option>
                      <option value="L">L</option>
                      <option value="M">M</option>
                    </select>
                  </td>
                {/if}
                <td class="right">{euro(Math.round(it.quantity * it.unitPriceCents))}</td>
                <td>
                  {#if items.length > 1}
                    <button type="button" class="btn-danger" onclick={() => removeItem(i)}>×</button>
                  {/if}
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
        <tfoot>
          <tr class="grand">
            <td colspan={isKlein ? 5 : 7} class="right"><strong>{isKlein ? "Summe" : "Netto"}</strong></td>
            <td class="right"><strong>{euro(totals.net)}</strong></td>
            <td></td>
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
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .card-hdr { display: flex; justify-content: space-between; align-items: center; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; }
  input, select { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; vertical-align: middle; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.85rem; }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.grand td { border-top: 2px solid #1a1a1a; }
  .modepick { border: 1px solid var(--c-border); }
  .modepick legend { font-size: 0.9rem; color: #4b5563; font-weight: 600; padding: 0 0.35rem; }
  label.radio { flex-direction: row; align-items: flex-start; gap: 0.6rem; padding: 0.6rem; border: 1px solid #e5e7eb; border-radius: 5px; cursor: pointer; margin-top: 0.5rem; }
  label.radio span { font-size: 0.85rem; color: #4b5563; line-height: 1.35; }
  label.radio strong { color: #1f2937; }
  label.radio em { font-style: normal; color: #3730a3; }
  label.radio input { width: auto; margin-top: 0.15rem; }
  label.radio.sel { border-color: var(--c-primary-500, #176b87); background: #ecfeff; }
  .hint { font-size: 0.82rem; color: #3730a3; background: #eef2ff; border: 1px solid #c7d2fe; padding: 0.5rem 0.75rem; border-radius: 4px; margin-top: 0.75rem; }
  /* Lokale btn-Definitionen entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
  .pkg-badge { display: inline-block; font-size: 0.72rem; font-weight: 600; color: #0f5d6e; background: #e0f2f7; border: 1px solid #b6e0ea; border-radius: 999px; padding: 0.05rem 0.5rem; }
  .pkg-desc { font-size: 0.82rem; color: #4b5563; margin-top: 0.2rem; white-space: pre-line; }
  .pkg-title { font-weight: 600; margin-top: 0.25rem; }
  .pkg-actions { display: flex; gap: 0.4rem; align-items: center; }
  .pkg-row td { background: #f8fbfc; border-bottom: 0; }
  .pkg-detail td { background: #f8fbfc; padding-top: 0; }
</style>
