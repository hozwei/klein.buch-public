<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import PageBar from "$lib/PageBar.svelte";
  import {
    contactsList,
    paymentAccountsList,
    expensesParseEinvoice,
    expensesCreateFromEinvoice,
  } from "$lib/api";
  import type {
    Contact,
    PaymentAccount,
    ExpenseCategory,
    ExpenseInputDto,
    EInvoiceParseResult,
    ValidationSummary,
  } from "$lib/types";
  import { EXPENSE_CATEGORIES, einvoiceSourceLabel } from "$lib/labels";
  import { euro } from "$lib/format";
  import { flash } from "$lib/toast.svelte";
  import Banner from "$lib/Banner.svelte";

  // Phasen: Datei wählen → Vorschau prüfen.
  let phase = $state<"select" | "review">("select");
  let busy = $state(false);

  let vendors = $state<Contact[]>([]);
  let accounts = $state<PaymentAccount[]>([]);

  // Datei + Parse-Ergebnis.
  let file = $state<File | null>(null);
  let originalBytes = $state<number[]>([]);
  let originalFileName = $state("");
  let sourceFormat = $state("");
  let validation = $state<ValidationSummary | null>(null);
  let parsed = $state<EInvoiceParseResult["parsed"] | null>(null);

  // Editierbare Formularfelder (aus dem Vorschlag vorbefüllt).
  let expenseDate = $state(new Date().toISOString().slice(0, 10));
  let paidDate = $state<string>(new Date().toISOString().slice(0, 10));
  let notPaidYet = $state(true);
  let vendorContactId = $state<string>("");
  let vendorName = $state("");
  let vendorInvoiceNumber = $state("");
  let category = $state<ExpenseCategory>("other");
  let description = $state("");
  let netEuros = $state<number>(0);
  let taxEuros = $state<number>(0);
  let paidFromAccountId = $state<string>("");
  let reverseCharge13b = $state(false);
  let notes = $state("");

  const maxDate = new Date().toISOString().slice(0, 10);
  let grossCents = $derived(Math.round(netEuros * 100) + Math.round(taxEuros * 100));

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");
      accounts = await paymentAccountsList(false);
    } catch (e) {
      flash("Daten konnten nicht geladen werden: " + String(e), "error");
    }
  });

  function onFile(e: Event) {
    file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
  }

  async function parse() {
    if (!file) {
      flash("Bitte zuerst eine Datei wählen (XML oder PDF).", "error");
      return;
    }
    busy = true;
    try {
      const buf = await file.arrayBuffer();
      originalBytes = Array.from(new Uint8Array(buf));
      originalFileName = file.name;
      const res: EInvoiceParseResult = await expensesParseEinvoice(originalBytes, file.name);

      parsed = res.parsed;
      sourceFormat = res.sourceFormat;
      validation = res.validation;

      // Felder aus dem Vorschlag vorbefüllen.
      const inp = res.input;
      expenseDate = inp.expenseDate;
      notPaidYet = inp.paidDate === null;
      paidDate = inp.paidDate ?? new Date().toISOString().slice(0, 10);
      vendorName = inp.vendorName;
      vendorInvoiceNumber = inp.vendorInvoiceNumber ?? "";
      category = inp.category as ExpenseCategory;
      description = inp.description;
      netEuros = inp.netAmountCents / 100;
      taxEuros = inp.taxAmountCents / 100;
      reverseCharge13b = inp.reverseCharge13b;
      notes = inp.notes ?? "";

      // Lieferanten-Kontakt automatisch vorschlagen, wenn USt-IdNr. übereinstimmt.
      if (parsed?.sellerVatId) {
        const match = vendors.find(
          (v) => (v.vatId ?? "").replace(/\s/g, "").toUpperCase() ===
            parsed!.sellerVatId!.replace(/\s/g, "").toUpperCase(),
        );
        if (match) {
          vendorContactId = match.id;
        }
      }
      const def = accounts.find((a) => a.isDefault === 1);
      if (def) paidFromAccountId = def.id;

      phase = "review";
    } catch (e) {
      flash("E-Rechnung konnte nicht gelesen werden: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  function onVendorSelect() {
    const c = vendors.find((v) => v.id === vendorContactId);
    if (c) vendorName = c.name;
  }

  async function confirm() {
    if (!vendorName.trim()) {
      flash("Lieferant (Name) ist erforderlich.", "error");
      return;
    }
    if (!description.trim()) {
      flash("Beschreibung ist erforderlich.", "error");
      return;
    }
    if (grossCents <= 0) {
      flash("Betrag muss größer als 0 sein.", "error");
      return;
    }
    busy = true;
    try {
      const input: ExpenseInputDto = {
        expenseDate,
        paidDate: notPaidYet ? null : paidDate || null,
        paidFromAccountId: paidFromAccountId || null,
        vendorContactId: vendorContactId || null,
        vendorName: vendorName.trim(),
        vendorInvoiceNumber: vendorInvoiceNumber.trim() || null,
        category,
        description: description.trim(),
        netAmountCents: Math.round(netEuros * 100),
        taxAmountCents: Math.round(taxEuros * 100),
        grossAmountCents: grossCents,
        currencyCode: "EUR",
        reverseCharge13b,
        notes: notes.trim() || null,
      };
      const detail = await expensesCreateFromEinvoice({
        input,
        fiscalYear: null,
        originalBytes,
        originalFileName,
        sourceFormat,
        validation,
      });
      flash("E-Rechnung erfasst: " + detail.expense.expenseNumber);
      await goto(`/expenses/${detail.expense.id}`);
    } catch (e) {
      flash("Festschreiben fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  function reset() {
    phase = "select";
    file = null;
    originalBytes = [];
    parsed = null;
    validation = null;
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title="E-Rechnung importieren" />

{#if phase === "select"}
  <p class="muted">
    Hast du von einem Lieferanten eine elektronische Rechnung bekommen
    (XRechnung-XML oder ZUGFeRD-PDF)? Lies sie hier ein — die Felder werden
    automatisch ausgefüllt, du prüfst sie und schreibst sie als Kosten fest.
  </p>

  <section class="card">
    <label class="span2">
      Datei (XRechnung-XML oder ZUGFeRD-PDF)
      <input type="file" accept=".xml,application/xml,text/xml,application/pdf,.pdf" onchange={onFile} />
    </label>
    {#if file}
      <p class="hint">Gewählt: <strong>{file.name}</strong></p>
    {/if}
    <div class="actions">
      <button class="btn-primary" onclick={parse} disabled={busy || !file}>
        {busy ? "Lese ein …" : "Einlesen & prüfen"}
      </button>
      <a class="btn-secondary" href="/expenses">Abbrechen</a>
    </div>
  </section>
{:else}
  <p class="caveat">
    Bitte die übernommenen Werte prüfen. Mit dem Festschreiben wird die Kosten-Position
    <strong>unveränderlich</strong> — die Original-Datei wird unverändert archiviert
    (gesetzliche Aufbewahrung). Vertippt? Dann später stornieren und neu erfassen.
  </p>

  <!-- Validierungs-Befund (beratend — blockiert nie) -->
  {#if validation === null}
    <Banner kind="info">
      Die formale Prüfung (KoSIT) war nicht möglich. Du kannst die Rechnung trotzdem erfassen.
    </Banner>
  {:else if validation.status === "passed"}
    <Banner kind="info">Formal gültige E-Rechnung (keine Beanstandungen).</Banner>
  {:else if validation.status === "warning"}
    <Banner kind="warning">
      Gültig, mit {validation.warningCount} {validation.warningCount === 1 ? "Hinweis" : "Hinweisen"}.
    </Banner>
  {:else}
    <Banner kind="error">
      Die Rechnung hat formale Mängel ({validation.errorCount}
      {validation.errorCount === 1 ? "Fehler" : "Fehler"}). Du kannst sie trotzdem erfassen
      — der Befund wird mitgespeichert.
    </Banner>
  {/if}

  {#if validation && validation.findings.length > 0}
    <details class="findings">
      <summary>Prüf-Details anzeigen ({validation.findings.length})</summary>
      <ul>
        {#each validation.findings as f}
          <li>
            <span class="sev sev-{f.severity}">{f.severity}</span>
            {#if f.ruleId}<code>{f.ruleId}</code>{/if}
            {f.message}
          </li>
        {/each}
      </ul>
    </details>
  {/if}

  <p class="src">
    Erkannt: <strong>{einvoiceSourceLabel(sourceFormat)}</strong>
    {#if parsed?.buyerName}
      · Empfänger laut Rechnung: {parsed.buyerName}
    {/if}
  </p>

  <section class="card">
    <div class="grid">
      <label>
        Beleg-Datum
        <input type="date" bind:value={expenseDate} max={maxDate} />
      </label>
      <label>
        Bezahlt am <span class="lbl-hint">(zählt fürs Finanzamt)</span>
        <input type="date" bind:value={paidDate} max={maxDate} disabled={notPaidYet} />
        <span class="chk-inline">
          <input type="checkbox" bind:checked={notPaidYet} /> noch nicht bezahlt
        </span>
      </label>
      <label>
        Lieferant (Kontakt)
        <select bind:value={vendorContactId} onchange={onVendorSelect}>
          <option value="">— ohne Kontakt (Freitext) —</option>
          {#each vendors as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
        </select>
      </label>
      <label>
        Lieferanten-Name
        <input type="text" bind:value={vendorName} placeholder="Name des Lieferanten" />
      </label>
      <label>
        Rechnungs-Nr. des Lieferanten
        <input type="text" bind:value={vendorInvoiceNumber} />
      </label>
      <label>
        Kategorie (EÜR)
        <select bind:value={category}>
          {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
        </select>
      </label>
      <label class="span2">
        Beschreibung
        <input type="text" bind:value={description} placeholder="Wofür wurde gezahlt?" />
      </label>
      <label>
        Netto (€)
        <input type="number" step="0.01" min="0" bind:value={netEuros} />
      </label>
      <label>
        USt-Betrag (€)
        <input type="number" step="0.01" min="0" bind:value={taxEuros} />
      </label>
      <label>
        Brutto (€)
        <input type="text" value={euro(grossCents)} readonly class="ro" />
      </label>
      <p class="hint span2">
        Netto und USt wurden aus der E-Rechnung übernommen. Stimmen sie nicht mit
        dem Beleg überein, hier korrigieren — der Brutto-Betrag rechnet sich neu.
      </p>
      <label>
        Zahlungs-Konto
        <select bind:value={paidFromAccountId}>
          <option value="">— keines —</option>
          {#each accounts as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
        </select>
      </label>
      <label class="chk span2">
        <input type="checkbox" bind:checked={reverseCharge13b} />
        §13b Reverse-Charge (Steuerschuldnerschaft des Leistungsempfängers)
      </label>
      <label class="span2">
        Notiz (optional)
        <textarea rows="2" bind:value={notes}></textarea>
      </label>
    </div>

    <div class="actions">
      <button class="btn-primary" onclick={confirm} disabled={busy}>
        {busy ? "Schreibe fest …" : "Als Kosten festschreiben"}
      </button>
      <button class="btn-secondary" onclick={reset} disabled={busy}>Andere Datei</button>
      <a class="btn-secondary" href="/expenses">Abbrechen</a>
    </div>
  </section>
{/if}

<style>
  .muted { color: #6b7280; }
  /* .caveat / .card entfernt — globale Definitionen aus tokens.css. */
  .src { font-size: 0.85rem; color: #4b5563; margin: 0.25rem 0 0.75rem; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  .lbl-hint { font-weight: 400; color: #9ca3af; font-size: 0.78rem; }
  label.chk { flex-direction: row; align-items: center; gap: 0.5rem; }
  .chk-inline { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.78rem; color: #6b7280; margin-top: 0.2rem; }
  input, select, textarea { padding: 0.45rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  input.ro { background: #f3f4f6; font-weight: 600; }
  .hint { font-size: 0.82rem; color: #3730a3; background: #eef2ff; border: 1px solid #c7d2fe; padding: 0.5rem 0.75rem; border-radius: 4px; margin: 0; }
  .actions { display: flex; gap: 0.75rem; margin-top: 1.25rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .findings { font-size: 0.85rem; margin: 0 0 1rem; }
  .findings summary { cursor: pointer; color: #4b5563; }
  .findings ul { margin: 0.5rem 0 0; padding-left: 1rem; }
  .findings li { margin: 0.25rem 0; line-height: 1.4; }
  .findings code { background: #f3f4f6; padding: 0 0.25rem; border-radius: 3px; font-size: 0.8rem; }
  .sev { font-size: 0.72rem; padding: 0.05rem 0.4rem; border-radius: 3px; margin-right: 0.3rem; }
  .sev-error { background: #fee2e2; color: #991b1b; }
  .sev-warning { background: #fef3c7; color: #92400e; }
  .sev-info { background: #dbeafe; color: #1e40af; }
</style>
