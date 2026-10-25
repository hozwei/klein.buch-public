<script lang="ts">
  import { todayBerlin, addDaysIso } from "#lib/date.ts";
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import {
    contactsList,
    invoicesCreateDraft,
    invoicesGet,
    invoicesUpdateDraft,
    sellerProfileGet,
    documentTermsGet,
  } from "#lib/api.ts";
  import type {
    Contact,
    CreateDraftArgs,
    InvoiceItemInput,
    SellerProfile,
    TaxCategoryCode,
    TravelLine,
    MaterializedPackageItem,
  } from "#lib/types.ts";
  import { euro } from "#lib/format.ts";
  import { applyTaxRules, documentTotals, REGEL_TAX_CODES } from "#lib/tax.ts";
  import Banner from "#lib/Banner.svelte";
  import TravelLineAdder from "#lib/TravelLineAdder.svelte";
  import PackageItemAdder from "#lib/PackageItemAdder.svelte";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  // Form-Position = InvoiceItemInput, aber descriptionMarkup als String (für den
  // bind:value-Editor) + transienter Paket-Name fürs Badge (Backend ignoriert ihn).
  type FormItem = InvoiceItemInput & {
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
    return !isRich(it) && !it.description.trim() && it.unitPriceCents === 0;
  }

  // ---- State ----
  let seller = $state<SellerProfile | null>(null);
  let contacts = $state<Contact[]>([]);
  let loading = $state(true);
  let saving = $state(false);
  // Lauf F5 (Neuer Fund F2a): `?edit=<id>` bearbeitet einen gespeicherten Entwurf
  // mit demselben Formular (Gestaltung im UI/UX-Lauf). Die Nummer bleibt.
  const editId = page.url.searchParams.get("edit");
  let editNumber = $state<string | null>(null);

  // Heute (Europe/Berlin-nah via lokalem Datum) — Obergrenze für Rechnungs- und
  // Leistungsdatum: beide dürfen nicht in der Zukunft liegen.
  const today = todayBerlin();
  let contactId = $state("");
  let invoiceDate = $state(today);
  let deliveryDate = $state("");
  let dueDate = $state("");
  let currency = $state("EUR");
  let pdfTemplate = $state("default");
  let notes = $state("");
  let buyerReference = $state("N/A");

  // Bezahlt-Hinweis: reiner PDF-Text je Rechnung. KEINE EÜR-Buchung — die
  // Zahlung wird weiterhin separat über „Zahlung erfassen" gebucht.
  let alreadyPaid = $state(false);
  let paymentNote = $state("");
  const todayDe = today.split("-").reverse().join(".");
  const paymentTemplates = [
    `Betrag dankend bar erhalten am ${todayDe}`,
    "Rechnungsbetrag bereits in bar beglichen",
    `Bezahlt am ${todayDe} in bar`,
    `Teilzahlung in bar bereits am ${todayDe}`,
  ];

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
    applyTaxRules(items, seller ? isKlein : null);
  });

  // ---- Totals (live, wie der Server: USt je Steuergruppe) ----
  let totals = $derived(documentTotals(items));

  onMount(async () => {
    loading = true;
    try {
      [seller, contacts] = await Promise.all([
        sellerProfileGet(),
        contactsList(false),
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

  async function save() {
    if (!contactId) {
      flash("Bitte Empfänger wählen.", "error");
      return;
    }
    if (!invoiceDate) {
      flash("Rechnungsdatum ist Pflicht.", "error");
      return;
    }
    if (!deliveryDate) {
      flash("Leistungsdatum ist Pflicht.", "error");
      return;
    }
    if (items.some(labelMissing)) {
      flash("Jede Position braucht eine Beschreibung bzw. einen Titel.", "error");
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
          items,
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

  function eurosFromInput(v: number): number {
    return Math.round(v * 100);
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
    Deine Firmendaten fehlen. <a href="/settings/seller">Jetzt ausfüllen</a>.
  </Banner>
{:else}
  {#if isKlein}
    <Banner kind="info">
      Du bist als Kleinunternehmer (§19) eingestellt — auf der Rechnung wird keine
      Mehrwertsteuer ausgewiesen. Die MwSt-Felder sind deshalb ausgeblendet.
    </Banner>
  {/if}

  <form id="invoice-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
    <section class="card">
      <h2>Allgemein</h2>
      <div class="grid">
        <label>
          Empfänger *
          <select bind:value={contactId} required>
            <option value="" disabled>— wählen —</option>
            {#each contacts as c (c.id)}
              <option value={c.id}>{c.name}{c.city ? ` (${c.city})` : ""}</option>
            {/each}
          </select>
        </label>
        <label>
          Rechnungsdatum *
          <input type="date" bind:value={invoiceDate} max={today} required />
        </label>
        <label>
          Leistungsdatum *
          <input type="date" bind:value={deliveryDate} max={invoiceDate || today} required />
        </label>
        <label>
          Fälligkeit
          <input type="date" bind:value={dueDate} />
        </label>
        <label>
          Währung
          <input type="text" bind:value={currency} maxlength="3" />
        </label>
        <label>
          Layout
          <input type="text" bind:value={pdfTemplate} />
        </label>
        <label>
          Referenz (nur für Behörden)
          <input type="text" bind:value={buyerReference} placeholder="N/A oder Leitweg-ID" />
        </label>
      </div>
    </section>

    <section class="card">
      <header class="card-hdr">
        <h2>Positionen</h2>
        <button type="button" onclick={addItem} class="btn-secondary">+ Position</button>
      </header>
      <table class="cards">
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
                <td data-label="Pos.">{it.position}</td>
                <td class="cards-full" data-label="Beschreibung">
                  {#if linked}
                    <span class="pkg-badge">aus Paket: {it.packageName ?? "Paket"} (V{it.sourcePackageRevision})</span>
                    <div class="pkg-title">{it.descriptionTitle}</div>
                  {:else}
                    <input type="text" bind:value={it.descriptionTitle} placeholder="Titel der Position" required />
                  {/if}
                </td>
                {#if linked}
                  <td class="right" data-label="Menge">{it.quantity}</td>
                  <td data-label="Einheit">{it.unitCode}</td>
                  <td class="right" data-label="Einzelpreis (€)">{euro(it.unitPriceCents)}</td>
                {:else}
                  <td data-label="Menge"><input type="number" step="any" min="0" bind:value={it.quantity} required /></td>
                  <td data-label="Einheit"><input type="text" bind:value={it.unitCode} placeholder="C62" maxlength="3" /></td>
                  <td data-label="Einzelpreis (€)">
                    <input type="number" step="0.01" value={it.unitPriceCents / 100}
                      oninput={(e) => { it.unitPriceCents = eurosFromInput(+(e.target as HTMLInputElement).value); }} required />
                  </td>
                {/if}
                {#if !isKlein}
                  <!-- Regelbesteuerung: Satz und Steuerart je Position (KB-0047); sie gehören
                       nicht zum Paket-Inhalt, die Provenienz bleibt. -->
                  <td data-label="USt-Satz %">
                    <input type="number" step="0.01" bind:value={it.taxRatePercent} />
                  </td>
                  <td data-label="USt-Art">
                    <select bind:value={it.taxCategoryCode}>
                      {#each REGEL_TAX_CODES as code (code)}<option value={code}>{code}</option>{/each}
                    </select>
                  </td>
                {/if}
                <td class="right" data-label="Netto">{euro(Math.round(it.quantity * it.unitPriceCents))}</td>
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
                <td colspan="99" class="cards-full">
                  {#if linked}
                    <div class="pkg-desc">{it.description}</div>
                  {:else}
                    <MarkdownEditor bind:value={it.descriptionMarkup} rows={5} placeholder="Beschreibung (Markdown) — erscheint formatiert auf dem PDF" />
                  {/if}
                </td>
              </tr>
            {:else}
              <tr>
                <td data-label="Pos.">{it.position}</td>
                <td class="cards-full" data-label="Beschreibung">
                  <input type="text" bind:value={it.description} required />
                </td>
                <td data-label="Menge">
                  <input type="number" step="any" min="0" bind:value={it.quantity} required />
                </td>
                <td data-label="Einheit">
                  <input type="text" bind:value={it.unitCode} placeholder="C62" maxlength="3" />
                </td>
                <td data-label="Einzelpreis (€)">
                  <input
                    type="number"
                    step="0.01"
                    value={it.unitPriceCents / 100}
                    oninput={(e) => { it.unitPriceCents = eurosFromInput(+(e.target as HTMLInputElement).value); }}
                    required
                  />
                </td>
                {#if !isKlein}
                  <td data-label="USt-Satz %">
                    <input type="number" step="0.01" bind:value={it.taxRatePercent} />
                  </td>
                  <td data-label="USt-Art">
                    <select bind:value={it.taxCategoryCode}>
                      {#each REGEL_TAX_CODES as code (code)}<option value={code}>{code}</option>{/each}
                    </select>
                  </td>
                {/if}
                <td class="right" data-label="Netto">{euro(Math.round(it.quantity * it.unitPriceCents))}</td>
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
          <tr>
            <td colspan={isKlein ? 5 : 7} class="right sum-label"><strong>Netto</strong></td>
            <td class="right" data-label="Netto"><strong>{euro(totals.net)}</strong></td>
            <td></td>
          </tr>
          {#if !isKlein}
            <tr>
              <td colspan="7" class="right sum-label">USt</td>
              <td class="right" data-label="USt">{euro(totals.tax)}</td>
              <td></td>
            </tr>
          {/if}
          <tr class="grand">
            <td colspan={isKlein ? 5 : 7} class="right sum-label"><strong>Brutto</strong></td>
            <td class="right" data-label="Brutto"><strong>{euro(totals.gross)}</strong></td>
            <td></td>
          </tr>
        </tfoot>
      </table>
      <TravelLineAdder onAdd={addTravelLine} />
      <PackageItemAdder onAdd={addPackage} />
    </section>

    <section class="card">
      <h2>Notiz</h2>
      <textarea rows="3" bind:value={notes} placeholder="z. B. Zahlungshinweis oder die Bestellnummer des Kunden"></textarea>
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
        <div class="paid-chips">
          {#each paymentTemplates as t}
            <button type="button" class="chip" onclick={() => (paymentNote = t)}>{t}</button>
          {/each}
        </div>
        <textarea rows="2" bind:value={paymentNote} placeholder={`z. B. Betrag dankend bar erhalten am ${todayDe}`}></textarea>
      {/if}
    </section>

  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .paid-toggle { display: flex; align-items: center; gap: 0.5rem; }
  .paid-hint { color: var(--c-text-muted); line-height: 1.5; margin: 0.6rem 0; }
  .paid-chips { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 0.6rem; }
  .chip { cursor: pointer; font-size: 0.8rem; padding: 4px 10px; border: 1px solid var(--c-border-strong); border-radius: 999px; background: var(--c-surface); color: var(--c-primary-700); }
  .chip:hover { background: var(--c-primary-50); border-color: var(--c-primary-300); }
  .card-hdr { display: flex; justify-content: space-between; align-items: center; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; }
  input, select, textarea {
    padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px;
    font-size: 0.95rem; font-family: inherit;
  }
  textarea { resize: vertical; width: 100%; box-sizing: border-box; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; vertical-align: middle; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.85rem; }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.grand td { border-top: 2px solid #1a1a1a; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): überschrieb
     padding/radius/font/color der globalen .btn-*-Klassen aus tokens.css
     und brach app-weite Button-Konsistenz. Die globalen Tokens greifen jetzt. */
  .muted { color: var(--c-text-muted); }
  .pkg-badge { display: inline-block; font-size: 0.72rem; font-weight: 600; color: #0f5d6e; background: #e0f2f7; border: 1px solid #b6e0ea; border-radius: 999px; padding: 0.05rem 0.5rem; }
  .pkg-desc { font-size: 0.82rem; color: #4b5563; margin-top: 0.2rem; white-space: pre-line; }
  .pkg-title { font-weight: 600; margin-top: 0.25rem; }
  .pkg-actions { display: flex; flex-wrap: wrap; gap: 0.4rem; align-items: center; }
  .pkg-row td { background: #f8fbfc; border-bottom: 0; }
  .pkg-detail td { background: #f8fbfc; padding-top: 0; }
  /* Plan W10 (Karten unter 820 px): die Summen-Beschriftung steht im
     data-label der Betragszelle; Paket-Kopf und -Beschreibung bilden eine Karte. */
  @media (max-width: 820px) {
    td.sum-label { display: none; }
    tr.pkg-row { margin-bottom: 0; border-bottom: 0; border-radius: var(--r-lg) var(--r-lg) 0 0; }
    tr.pkg-detail { border-top: 0; border-radius: 0 0 var(--r-lg) var(--r-lg); }
  }
</style>
