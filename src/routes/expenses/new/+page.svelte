<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import { goto } from "$app/navigation";
  import { contactsList, paymentAccountsList, expensesCreate, uploadFile } from "#lib/api.ts";
  import type { Contact, PaymentAccount, ExpenseCategory, ExpenseInputDto } from "#lib/types.ts";
  import { EXPENSE_CATEGORIES } from "#lib/labels.ts";
  import { euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import {
    REVERSE_CHARGE_13B_HINT,
    reverseChargeTaxError,
    roundHalfAwayFromZero,
  } from "#lib/tax.ts";

  let vendors = $state<Contact[]>([]);
  let accounts = $state<PaymentAccount[]>([]);
  let busy = $state(false);

  // Formularfelder
  let expenseDate = $state(todayBerlin());
  let paidDate = $state<string>(todayBerlin());
  let notPaidYet = $state(false);
  let vendorContactId = $state<string>("");
  let vendorName = $state("");
  let vendorInvoiceNumber = $state("");
  let category = $state<ExpenseCategory>("office");
  let description = $state("");
  // Beträge in Cent (MoneyInput, Lauf 2026.11 U1); null = leer oder ungültig.
  let netCents = $state<number | null>(null);
  // USt-Satz: feste Sätze rechnen den Betrag automatisch; "custom" = manuell
  // (für cent-genaue Übernahme vom Beleg, falls der Lieferant abweichend rundet).
  let vatRate = $state<"19" | "7" | "0" | "custom">("19");
  let taxCents = $state<number | null>(0);
  let paidFromAccountId = $state<string>("");
  let reverseCharge13b = $state(false);
  let notes = $state("");
  let receiptFile = $state<File | null>(null);

  // Feldfehler (3.3.1): am Feld, nicht als Toast.
  let errors = $state<{ vendorName?: string; description?: string; net?: string; tax?: string; expenseDate?: string }>({});

  let netInput: MoneyInput | undefined = $state();
  let taxInput: MoneyInput | undefined = $state();
  let vendorNameEl: HTMLInputElement | undefined = $state();
  let descriptionEl: HTMLInputElement | undefined = $state();
  let expenseDateEl: HTMLInputElement | undefined = $state();

  // Kein zukünftiges Datum erlaubt (Beleg-/Zahldatum); Backend erzwingt es auch.
  const maxDate = todayBerlin();

  const grossCents = $derived((netCents ?? 0) + (taxCents ?? 0));

  // Übergabe F5 (R1-019): Zahldatum vor dem Belegdatum ist erlaubt (z. B.
  // Vorkasse), aber auffällig — Hinweis, kein Fehler.
  const paidBeforeExpense = $derived(
    !notPaidYet && !!paidDate && !!expenseDate && paidDate < expenseDate,
  );

  // Bei festem Satz USt aus Netto berechnen (ganzzahlig in Cent, Halbe weg von 0
  // wie der Server). Liest bewusst NICHT taxCents → keine Effekt-Schleife.
  $effect(() => {
    if (vatRate !== "custom") {
      const rate = Number(vatRate);
      taxCents = roundHalfAwayFromZero(((netCents ?? 0) * rate) / 100);
    }
  });

  // §13b: Lieferant weist keine USt aus → Satz 0 % und USt-Betrag 0 (gesperrt).
  function onReverseChargeChange() {
    if (reverseCharge13b) {
      vatRate = "0";
      taxCents = 0;
    }
    errors.tax = undefined;
  }

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");
      accounts = await paymentAccountsList(false);
      const def = accounts.find((a) => a.isDefault === 1);
      if (def) paidFromAccountId = def.id;
    } catch (e) {
      flash("Daten konnten nicht geladen werden: " + String(e), "error");
    }
  });

  // Bei Auswahl eines Lieferanten-Kontakts den Snapshot-Namen vorbefüllen.
  function onVendorSelect() {
    const c = vendors.find((v) => v.id === vendorContactId);
    if (c) {
      vendorName = c.name;
      errors.vendorName = undefined;
    }
  }

  /** Prüft alle Felder, zeigt die Meldungen am Feld und fokussiert das erste fehlerhafte. */
  async function validate(): Promise<boolean> {
    const e: typeof errors = {};
    if (!expenseDate) e.expenseDate = "Bitte das Datum des Belegs angeben.";
    if (!vendorName.trim()) e.vendorName = "Bitte den Namen des Lieferanten angeben.";
    if (!description.trim()) e.description = "Bitte kurz beschreiben, wofür du gezahlt hast.";
    const netOk = netInput?.validate() ?? true;
    const taxOk = taxInput?.validate() ?? true;
    if (netOk && grossCents <= 0) e.net = "Der Betrag muss größer als 0 sein.";
    const rc = reverseChargeTaxError(reverseCharge13b, taxCents ?? 0);
    if (rc) e.tax = rc;
    errors = e;
    await tick();
    if (e.expenseDate) expenseDateEl?.focus();
    else if (e.vendorName) vendorNameEl?.focus();
    else if (e.description) descriptionEl?.focus();
    else if (!netOk || e.net) netInput?.focus();
    else if (!taxOk || e.tax) taxInput?.focus();
    return !e.expenseDate && !e.vendorName && !e.description && !e.net && !e.tax && netOk && taxOk;
  }

  async function save() {
    if (!(await validate())) return;
    busy = true;
    try {
      // Plan W7: Beleg zuerst hochladen (Größe/Typ prüft der Server).
      const receiptUploadId = receiptFile ? (await uploadFile("beleg", receiptFile)).uploadId : null;
      const input: ExpenseInputDto = {
        expenseDate,
        paidDate: notPaidYet ? null : paidDate || null,
        paidFromAccountId: paidFromAccountId || null,
        vendorContactId: vendorContactId || null,
        vendorName: vendorName.trim(),
        vendorInvoiceNumber: vendorInvoiceNumber.trim() || null,
        category,
        description: description.trim(),
        netAmountCents: netCents ?? 0,
        taxAmountCents: taxCents ?? 0,
        grossAmountCents: grossCents,
        currencyCode: "EUR",
        reverseCharge13b,
        notes: notes.trim() || null,
      };
      const detail = await expensesCreate({
        input,
        receiptUploadId,
      });
      flash("Kosten erfasst: " + detail.expense.expenseNumber);
      await goto(`/expenses/${detail.expense.id}`);
    } catch (e) {
      const msg = String(e);
      // Übergabe F7: Meldung `ReverseChargeWithTax` des Servers am USt-Feld.
      if (msg.includes("§13b (Reverse-Charge)")) {
        errors = { ...errors, tax: msg.replace(/^Error:\s*/, "") };
        await tick();
        taxInput?.focus();
      } else {
        flash("Speichern fehlgeschlagen: " + msg, "error");
      }
    } finally {
      busy = false;
    }
  }

  function pickReceipt(e: Event) {
    receiptFile = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title="Neue Kosten erfassen">
  {#snippet actions()}
    <a class="btn-secondary" href="/expenses">Abbrechen</a>
    <button type="button" class="btn-primary" onclick={save} disabled={busy} aria-busy={busy}>
      {busy ? "Speichere …" : "Kosten speichern"}
    </button>
  {/snippet}
</PageBar>
<p class="caveat">
  Einmal gespeichert lässt sich eine Kostenposition <strong>nicht mehr ändern</strong>
  — das schreibt das Gesetz so vor. Vertippt? Dann stornierst du sie und legst
  sie neu an. Also vorher kurz prüfen.
</p>

<section class="card">
  <form novalidate onsubmit={(e) => { e.preventDefault(); save(); }}>
    <p class="legend">Felder mit <span class="req" aria-hidden="true">*</span> sind Pflicht.</p>
    <div class="grid">
      <div class="field">
        <label for="exp-date">Beleg-Datum<span class="req" aria-hidden="true">*</span></label>
        <input
          id="exp-date"
          class="kb-input"
          type="date"
          bind:this={expenseDateEl}
          bind:value={expenseDate}
          max={maxDate}
          required
          aria-required="true"
          aria-invalid={errors.expenseDate ? "true" : undefined}
          aria-describedby={errors.expenseDate ? "exp-date-err" : undefined}
        />
        {#if errors.expenseDate}<p id="exp-date-err" class="msg err">{errors.expenseDate}</p>{/if}
      </div>
      <div class="field">
        <label for="exp-paid">Bezahlt am <span class="lbl-hint">(zählt fürs Finanzamt)</span></label>
        <input
          id="exp-paid"
          class="kb-input"
          type="date"
          bind:value={paidDate}
          max={maxDate}
          disabled={notPaidYet}
          aria-describedby={paidBeforeExpense ? "exp-paid-note" : undefined}
        />
        <label class="chk-inline">
          <input type="checkbox" bind:checked={notPaidYet} /> noch nicht bezahlt
        </label>
        {#if paidBeforeExpense}
          <p id="exp-paid-note" class="msg note">
            Das Zahldatum liegt vor dem Belegdatum (z. B. Vorkasse). Das ist erlaubt —
            prüfe nur kurz, ob beide Daten stimmen.
          </p>
        {/if}
      </div>
      <div class="field">
        <label for="exp-vendor-contact">Lieferant (Kontakt)</label>
        <select id="exp-vendor-contact" class="kb-input" bind:value={vendorContactId} onchange={onVendorSelect}>
          <option value="">— ohne Kontakt (Freitext) —</option>
          {#each vendors as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
        </select>
      </div>
      <div class="field">
        <label for="exp-vendor-name">Lieferanten-Name<span class="req" aria-hidden="true">*</span></label>
        <input
          id="exp-vendor-name"
          class="kb-input"
          type="text"
          autocomplete="off"
          bind:this={vendorNameEl}
          bind:value={vendorName}
          oninput={() => (errors.vendorName = undefined)}
          required
          aria-required="true"
          aria-invalid={errors.vendorName ? "true" : undefined}
          aria-describedby={errors.vendorName ? "exp-vendor-name-err" : undefined}
        />
        {#if errors.vendorName}<p id="exp-vendor-name-err" class="msg err">{errors.vendorName}</p>{/if}
      </div>
      <div class="field">
        <label for="exp-vendor-no">Rechnungs-Nr. des Lieferanten</label>
        <input id="exp-vendor-no" class="kb-input" type="text" autocomplete="off" bind:value={vendorInvoiceNumber} />
      </div>
      <div class="field">
        <label for="exp-category">Kategorie (EÜR)</label>
        <select id="exp-category" class="kb-input" bind:value={category}>
          {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
        </select>
      </div>
      <div class="field span2">
        <label for="exp-desc">Beschreibung<span class="req" aria-hidden="true">*</span></label>
        <input
          id="exp-desc"
          class="kb-input"
          type="text"
          autocomplete="off"
          bind:this={descriptionEl}
          bind:value={description}
          oninput={() => (errors.description = undefined)}
          required
          aria-required="true"
          aria-invalid={errors.description ? "true" : undefined}
          aria-describedby={errors.description ? "exp-desc-err" : "exp-desc-hint"}
        />
        {#if errors.description}
          <p id="exp-desc-err" class="msg err">{errors.description}</p>
        {:else}
          <p id="exp-desc-hint" class="msg hint">Wofür wurde gezahlt?</p>
        {/if}
      </div>
      <MoneyInput
        bind:this={netInput}
        bind:cents={netCents}
        label="Netto"
        required
        error={errors.net}
        onchange={() => (errors.net = undefined)}
      />
      <div class="field">
        <label for="exp-vat-rate">USt-Satz</label>
        <select id="exp-vat-rate" class="kb-input" bind:value={vatRate} aria-describedby={reverseCharge13b ? "exp-rc-hint" : undefined}>
          <!-- §13b: nur netto (ADR 0039 Nr. 143), andere Sätze gesperrt -->
          <option value="19" disabled={reverseCharge13b}>19 % (Regelsatz)</option>
          <option value="7" disabled={reverseCharge13b}>7 % (ermäßigt)</option>
          <option value="0">0 % (steuerfrei / §13b / Kleinunternehmer)</option>
          <option value="custom" disabled={reverseCharge13b}>individuell (Betrag vom Beleg)</option>
        </select>
      </div>
      <MoneyInput
        bind:this={taxInput}
        bind:cents={taxCents}
        label="USt-Betrag"
        readonly={vatRate !== "custom" || reverseCharge13b}
        error={errors.tax}
        hint={reverseCharge13b
          ? "Bei §13b immer 0 — gesperrt."
          : vatRate !== "custom"
            ? "Wird aus Netto und Satz berechnet."
            : undefined}
        onchange={() => (errors.tax = undefined)}
      />
      <div class="field">
        <label for="exp-gross">Brutto</label>
        <output id="exp-gross" class="kb-input ro kb-num" aria-live="off">{euro(grossCents)}</output>
      </div>
      <p class="info-banner span2">
        Der Beleg ist maßgeblich: Weicht die ausgewiesene USt vom berechneten Wert
        ab (Rundung beim Lieferanten), wähle „individuell“ und übernimm den
        USt-Betrag genau vom Beleg.
      </p>
      <div class="field">
        <label for="exp-account">Zahlungs-Konto</label>
        <select id="exp-account" class="kb-input" bind:value={paidFromAccountId}>
          <option value="">— keines —</option>
          {#each accounts as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
        </select>
      </div>
      <label class="chk span2">
        <input
          type="checkbox"
          bind:checked={reverseCharge13b}
          onchange={onReverseChargeChange}
          aria-describedby={reverseCharge13b ? "exp-rc-hint" : undefined}
        />
        <span>§13b Reverse-Charge (Steuerschuldnerschaft des Leistungsempfängers)</span>
      </label>
      {#if reverseCharge13b}
        <div id="exp-rc-hint" class="info-banner span2 rc-hint">
          <strong>§13b aktiv:</strong> {REVERSE_CHARGE_13B_HINT}
          <span class="rc-facts">USt-Satz 0 % · USt-Betrag 0 · Brutto = Netto ({euro(netCents ?? 0)})</span>
        </div>
      {/if}
      <div class="span2 receipt">
        <div class="field grow">
          <label for="exp-receipt">Beleg (PDF oder Bild, optional)</label>
          <input
            id="exp-receipt"
            type="file"
            accept="application/pdf,image/*"
            onchange={pickReceipt}
          />
        </div>
        <!-- Plan W10/§8.5: zusätzlicher, ausdrücklich beschrifteter Kamera-Knopf;
             das Datei-Feld daneben bietet Kamera, Galerie und Dateien an. -->
        <label class="btn-secondary camera">
          Foto aufnehmen
          <input
            class="kb-visually-hidden"
            type="file"
            accept="image/*"
            capture="environment"
            onchange={pickReceipt}
          />
        </label>
        {#if receiptFile}<span class="receipt-name" role="status">Ausgewählt: {receiptFile.name}</span>{/if}
      </div>
      <div class="field span2">
        <label for="exp-notes">Notiz (optional)</label>
        <textarea id="exp-notes" class="kb-input" rows="3" bind:value={notes}></textarea>
      </div>
    </div>
    <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end"><button type="button" class="btn-primary" onclick={save} disabled={busy} aria-busy={busy}>{busy ? "Speichere …" : "Kosten speichern"}</button></div>
  </form>
</section>

<style>
  /* .caveat / .card / .info-banner / .kb-input: globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); align-items: start; }
  @media (max-width: 600px) { .grid { grid-template-columns: 1fr; } }
  .span2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .field > label:first-child { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .legend { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--sp-3); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .lbl-hint { font-weight: 400; color: var(--text-muted); font-size: var(--fs-xs); }
  .msg { font-size: var(--fs-sm); margin: 0; line-height: 1.4; }
  .msg.err { color: var(--status-danger-fg); font-weight: 600; }
  .msg.hint { color: var(--text-muted); }
  .msg.note { color: var(--status-warning-fg); background: var(--status-warning-bg); border-left: 3px solid var(--status-warning-line); padding: var(--sp-1) var(--sp-2); }
  .receipt { display: flex; flex-wrap: wrap; align-items: flex-end; gap: var(--sp-3); }
  .grow { flex: 1 1 16rem; }
  /* position: relative hält das versteckte Kamera-Feld im Knopf (kein Seitenüberlauf). */
  label.camera { position: relative; min-height: var(--target); cursor: pointer; }
  label.camera:focus-within { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); }
  .receipt-name { flex-basis: 100%; font-size: var(--fs-sm); color: var(--text-muted); overflow-wrap: anywhere; }
  label.chk { display: flex; flex-direction: row; align-items: center; gap: var(--sp-2); min-height: var(--target); font-size: var(--fs-base); color: var(--text); }
  .chk-inline { display: inline-flex; align-items: center; gap: var(--sp-2); min-height: var(--target); font-size: var(--fs-sm); color: var(--text-muted); }
  .ro { background: var(--surface-sunken); font-weight: 600; display: block; }
  .info-banner { margin: 0; }
  .rc-hint strong { margin-right: var(--sp-1); }
  .rc-facts { display: block; margin-top: var(--sp-1); font-weight: 600; font-variant-numeric: tabular-nums; }
  /* MoneyInput bringt eigenen Abstand mit; im Grid übernimmt gap. */
  .grid :global(.money-field.with-label) { margin-bottom: 0; }
  .grid :global(.money-field label) { color: var(--text); }
</style>
