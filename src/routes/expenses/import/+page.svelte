<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import ContactSelect from "#lib/ContactSelect.svelte";
  import {
    contactsList,
    paymentAccountsList,
    expensesParseEinvoice,
    expensesCreateFromEinvoice,
    uploadFile,
  } from "#lib/api.ts";
  import type {
    Contact,
    PaymentAccount,
    ExpenseCategory,
    ExpenseInputDto,
    EInvoiceParseResult,
    ValidationSummary,
  } from "#lib/types.ts";
  import { EXPENSE_CATEGORIES, einvoiceSourceLabel } from "#lib/labels.ts";
  import { date, euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import { REVERSE_CHARGE_13B_HINT, reverseChargeTaxError } from "#lib/tax.ts";

  const SEVERITY_LABEL: Record<string, string> = { error: "Fehler", warning: "Warnung", info: "Hinweis" };

  // Phasen: Datei wählen → Vorschau prüfen.
  let phase = $state<"select" | "review">("select");
  let busy = $state(false);

  let vendors = $state<Contact[]>([]);
  let accounts = $state<PaymentAccount[]>([]);

  // Datei + Parse-Ergebnis.
  let file = $state<File | null>(null);
  let fileError = $state("");
  // Plan W7: Upload des Originals aus Schritt 1, in Schritt 2 archiviert.
  let uploadId = $state("");
  let sourceFormat = $state("");
  let validation = $state<ValidationSummary | null>(null);
  let parsed = $state<EInvoiceParseResult["parsed"] | null>(null);

  // Editierbare Formularfelder (aus dem Vorschlag vorbefüllt).
  let expenseDate = $state(todayBerlin());
  let paidDate = $state<string>(todayBerlin());
  let notPaidYet = $state(true);
  let vendorContactId = $state<string>("");
  let vendorName = $state("");
  let vendorInvoiceNumber = $state("");
  let category = $state<ExpenseCategory>("other");
  let description = $state("");
  // Beträge in Cent (MoneyInput, Lauf 2026.11 U1).
  let netCents = $state<number | null>(0);
  let taxCents = $state<number | null>(0);
  /** USt-Betrag vor dem Einschalten von §13b (zurück beim Ausschalten). */
  let taxBefore13b: number | null = null;
  let paidFromAccountId = $state<string>("");
  let reverseCharge13b = $state(false);
  let notes = $state("");

  let errors = $state<{ vendorName?: string; description?: string; net?: string; tax?: string; expenseDate?: string }>({});
  let netInput: MoneyInput | undefined = $state();
  let taxInput: MoneyInput | undefined = $state();
  let vendorNameEl: HTMLInputElement | undefined = $state();
  let descriptionEl: HTMLInputElement | undefined = $state();
  let expenseDateEl: HTMLInputElement | undefined = $state();
  let fileEl: HTMLInputElement | undefined = $state();

  const maxDate = todayBerlin();
  const grossCents = $derived((netCents ?? 0) + (taxCents ?? 0));

  // Übergabe F5 (R1-019): Zahldatum vor dem Belegdatum — Hinweis, kein Fehler.
  const paidBeforeExpense = $derived(
    !notPaidYet && !!paidDate && !!expenseDate && paidDate < expenseDate,
  );

  // Übergabe F5 (Warteliste 2, U4): Hinweis bei Steuerkategorie K (innergemeinschaftliche
  // Lieferung). Seit U4 liest der Parser die Kategorien der Rechnung
  // (`parsed.intraCommunity`, ADR 0041 Nr. 82) — keine Näherung über die USt-IdNr. mehr.
  // Nur ein Hinweis: Beträge und §13b-Vorschlag hängen nicht daran.
  const intraCommunity = $derived(parsed?.intraCommunity === true);

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
    fileError = "";
  }

  // U4 (V1-20, ADR 0041 Nr. 135): Der Knopf bleibt ohne Datei aktiv; der Klick nennt den
  // Grund am Datei-Feld und setzt den Fokus dorthin.
  async function parse() {
    if (busy) return;
    if (!file) {
      fileError = "Bitte eine Datei wählen (XRechnung-XML oder ZUGFeRD-PDF).";
      fileEl?.focus();
      return;
    }
    busy = true;
    try {
      uploadId = (await uploadFile("erechnung", file)).uploadId;
      const res: EInvoiceParseResult = await expensesParseEinvoice(uploadId);

      parsed = res.parsed;
      sourceFormat = res.sourceFormat;
      validation = res.validation;

      // Felder aus dem Vorschlag vorbefüllen.
      const inp = res.input;
      expenseDate = inp.expenseDate;
      notPaidYet = inp.paidDate === null;
      paidDate = inp.paidDate ?? todayBerlin();
      vendorName = inp.vendorName;
      vendorInvoiceNumber = inp.vendorInvoiceNumber ?? "";
      category = inp.category as ExpenseCategory;
      description = inp.description;
      netCents = inp.netAmountCents;
      taxCents = inp.taxAmountCents;
      reverseCharge13b = inp.reverseCharge13b;
      taxBefore13b = null;
      notes = inp.notes ?? "";
      errors = {};

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
      // V1-20: Der Fehler steht am Feld (dort fokussiert), kein zusätzlicher Toast.
      fileError = "E-Rechnung konnte nicht gelesen werden: " + String(e).replace(/^Error:\s*/, "");
      await tick();
      fileEl?.focus();
    } finally {
      busy = false;
    }
  }

  function onVendorSelect(id: string) {
    const c = vendors.find((v) => v.id === id);
    if (c) {
      vendorName = c.name;
      errors.vendorName = undefined;
    }
  }

  // Übergabe F7: §13b sperrt das USt-Feld auf 0 (Netto = Brutto); beim
  // Ausschalten kommt der vorherige Wert zurück.
  function onReverseChargeChange() {
    if (reverseCharge13b) {
      taxBefore13b = taxCents;
      taxCents = 0;
    } else if (taxBefore13b !== null) {
      taxCents = taxBefore13b;
      taxBefore13b = null;
    }
    errors.tax = undefined;
  }

  async function validate(): Promise<boolean> {
    const e: typeof errors = {};
    if (!expenseDate) e.expenseDate = "Bitte das Datum des Belegs angeben.";
    if (!vendorName.trim()) e.vendorName = "Bitte den Namen des Lieferanten angeben.";
    if (!description.trim()) e.description = "Bitte kurz beschreiben, wofür du gezahlt hast.";
    const netOk = netInput?.validate() ?? true;
    const taxOk = taxInput?.validate() ?? true;
    if (netOk && taxOk && grossCents <= 0) e.net = "Der Betrag muss größer als 0 sein.";
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

  async function confirm() {
    if (busy) return;
    if (!(await validate())) return;
    // U4 (V1-05): Rückfrage mit den Eckwerten wie bei der manuellen Erfassung.
    const ok = await confirmDialog({
      title: "Kosten festschreiben?",
      body:
        "Danach lässt sich nichts mehr ändern. Eine Korrektur geht nur per Storno und " +
        "neuer Erfassung. Die E-Rechnung wird unverändert archiviert.",
      bullets: [
        `Belegdatum: ${date(expenseDate)}`,
        `Lieferant: ${vendorName.trim()}`,
        `Brutto: ${euro(grossCents)}`,
        notPaidYet || !paidDate ? "Bezahlt: noch offen" : `Bezahlt am: ${date(paidDate)}`,
      ],
      confirmLabel: "Festschreiben",
      cancelLabel: "Abbrechen",
    });
    if (!ok) return;
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
        netAmountCents: netCents ?? 0,
        taxAmountCents: taxCents ?? 0,
        grossAmountCents: grossCents,
        currencyCode: "EUR",
        reverseCharge13b,
        notes: notes.trim() || null,
      };
      const detail = await expensesCreateFromEinvoice({
        input,
        uploadId,
        sourceFormat,
        validation,
      });
      flash("Kosten festgeschrieben: " + detail.expense.expenseNumber);
      await goto(`/expenses/${detail.expense.id}`);
    } catch (e) {
      const msg = String(e);
      // Übergabe F7: Meldung `ReverseChargeWithTax` des Servers am USt-Feld.
      if (msg.includes("§13b (Reverse-Charge)")) {
        errors = { ...errors, tax: msg.replace(/^Error:\s*/, "") };
        await tick();
        taxInput?.focus();
      } else {
        flash("Festschreiben fehlgeschlagen: " + msg, "error");
      }
    } finally {
      busy = false;
    }
  }

  function reset() {
    phase = "select";
    file = null;
    fileError = "";
    uploadId = "";
    parsed = null;
    validation = null;
    errors = {};
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title="E-Rechnung importieren">
  {#snippet actions()}
    <HelpAnchor slug="rechnungseingang-per-upload" />
  {/snippet}
</PageBar>

{#if phase === "select"}
  <p class="muted">
    Hast du von einem Lieferanten eine elektronische Rechnung bekommen
    (XRechnung-XML oder ZUGFeRD-PDF)? Lies sie hier ein — die Felder werden
    automatisch ausgefüllt, du prüfst sie und schreibst sie als Kosten fest.
  </p>

  <section class="card">
    <div class="field">
      <label for="einv-file">Datei (XRechnung-XML oder ZUGFeRD-PDF)<span class="req" aria-hidden="true">*</span></label>
      <input
        id="einv-file"
        bind:this={fileEl}
        type="file"
        accept=".xml,application/xml,text/xml,application/pdf,.pdf"
        onchange={onFile}
        aria-required="true"
        aria-invalid={fileError ? "true" : undefined}
        aria-describedby={fileError ? "einv-file-err" : undefined}
      />
      {#if fileError}<p id="einv-file-err" class="msg err">{fileError}</p>{/if}
    </div>
    {#if file}
      <p class="msg hint">Gewählt: <strong>{file.name}</strong></p>
    {/if}
    <div class="actions">
      <a class="btn-secondary" href="/expenses">Abbrechen</a>
      <button type="button" class="btn-primary" onclick={parse} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>
        {busy ? "Liest ein …" : "Einlesen und prüfen"}
      </button>
    </div>
  </section>
{:else}
  <p class="caveat">
    Bitte die übernommenen Werte prüfen. Mit „Kosten festschreiben“ wird die Kostenposition
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
      Die Rechnung hat formale Mängel ({validation.errorCount} Fehler). Du kannst sie trotzdem erfassen
      — der Befund wird mitgespeichert.
    </Banner>
  {/if}

  {#if validation && validation.findings.length > 0}
    <details class="findings">
      <summary>Prüf-Details anzeigen ({validation.findings.length})</summary>
      <ul>
        {#each validation.findings as f}
          <li>
            <span class="sev sev-{f.severity}">{SEVERITY_LABEL[f.severity] ?? f.severity}</span>
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
    <form novalidate onsubmit={(e) => { e.preventDefault(); confirm(); }}>
      <p class="legend">Felder mit <span class="req" aria-hidden="true">*</span> sind Pflicht.</p>
      <div class="grid">
        <div class="field">
          <label for="imp-date">Beleg-Datum<span class="req" aria-hidden="true">*</span></label>
          <input
            id="imp-date"
            class="kb-input kb-w-date"
            type="date"
            bind:this={expenseDateEl}
            bind:value={expenseDate}
            max={maxDate}
            required
            aria-required="true"
            aria-invalid={errors.expenseDate ? "true" : undefined}
            aria-describedby={errors.expenseDate ? "imp-date-err" : undefined}
          />
          {#if errors.expenseDate}<p id="imp-date-err" class="msg err">{errors.expenseDate}</p>{/if}
        </div>
        <div class="field">
          <label for="imp-paid">Bezahlt am <span class="lbl-hint">(zählt fürs Finanzamt)</span></label>
          <input
            id="imp-paid"
            class="kb-input kb-w-date"
            type="date"
            bind:value={paidDate}
            max={maxDate}
            disabled={notPaidYet}
            aria-describedby={paidBeforeExpense ? "imp-paid-note" : undefined}
          />
          <label class="chk-inline">
            <input type="checkbox" bind:checked={notPaidYet} /> noch nicht bezahlt
          </label>
          {#if paidBeforeExpense}
            <p id="imp-paid-note" class="msg note">
              Das Zahldatum liegt vor dem Belegdatum (z. B. Vorkasse). Das ist erlaubt —
              prüfe nur kurz, ob beide Daten stimmen.
            </p>
          {/if}
        </div>
        <!-- U4 (Warteliste 15): durchsuchbar ab 8 Lieferanten, deutsche Sortierung. -->
        <ContactSelect
          bind:value={vendorContactId}
          contacts={vendors}
          label="Lieferant (Kontakt)"
          id="imp-vendor-contact"
          placeholder="— ohne Kontakt (Freitext) —"
          onchange={onVendorSelect}
        />
        <div class="field">
          <label for="imp-vendor-name">Lieferanten-Name<span class="req" aria-hidden="true">*</span></label>
          <input
            id="imp-vendor-name"
            class="kb-input"
            type="text"
            autocomplete="off"
            bind:this={vendorNameEl}
            bind:value={vendorName}
            oninput={() => (errors.vendorName = undefined)}
            required
            aria-required="true"
            aria-invalid={errors.vendorName ? "true" : undefined}
            aria-describedby={errors.vendorName ? "imp-vendor-name-err" : undefined}
          />
          {#if errors.vendorName}<p id="imp-vendor-name-err" class="msg err">{errors.vendorName}</p>{/if}
        </div>
        <div class="field">
          <label for="imp-vendor-no">Rechnungs-Nr. des Lieferanten</label>
          <input id="imp-vendor-no" class="kb-input" type="text" autocomplete="off" bind:value={vendorInvoiceNumber} />
        </div>
        <div class="field">
          <label for="imp-category">Kategorie (EÜR)</label>
          <select id="imp-category" class="kb-input" bind:value={category}>
            {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
          </select>
        </div>
        <div class="field span2">
          <label for="imp-desc">Beschreibung<span class="req" aria-hidden="true">*</span></label>
          <input
            id="imp-desc"
            class="kb-input"
            type="text"
            autocomplete="off"
            bind:this={descriptionEl}
            bind:value={description}
            oninput={() => (errors.description = undefined)}
            required
            aria-required="true"
            aria-invalid={errors.description ? "true" : undefined}
            aria-describedby={errors.description ? "imp-desc-err" : undefined}
          />
          {#if errors.description}<p id="imp-desc-err" class="msg err">{errors.description}</p>{/if}
        </div>
        <MoneyInput
          bind:this={netInput}
          bind:cents={netCents}
          fieldClass="kb-w-amount"
          label="Netto"
          required
          error={errors.net}
          onchange={() => (errors.net = undefined)}
        />
        <MoneyInput
          bind:this={taxInput}
          bind:cents={taxCents}
          fieldClass="kb-w-amount"
          label="USt-Betrag"
          readonly={reverseCharge13b}
          error={errors.tax}
          hint={reverseCharge13b ? "Bei §13b immer 0 — gesperrt." : undefined}
          onchange={() => (errors.tax = undefined)}
        />
        <div class="field">
          <label for="imp-gross">Brutto</label>
          <output id="imp-gross" class="kb-input ro kb-num kb-w-amount">{euro(grossCents)}</output>
        </div>
        <p class="info-banner span2">
          Netto und USt wurden aus der E-Rechnung übernommen. Stimmen sie nicht mit
          dem Beleg überein, hier korrigieren — der Brutto-Betrag rechnet sich neu.
        </p>
        {#if intraCommunity}
          <div class="caveat span2" role="note">
            <strong>Steuerkategorie K (für dich ein innergemeinschaftlicher Erwerb):</strong>
            Der Lieferant aus dem EU-Ausland berechnet seine innergemeinschaftliche Lieferung ohne
            Umsatzsteuer. Für dich
            kann auch mit §19 Erwerbsteuer anfallen — abhängig von der Erwerbsschwelle und
            davon, ob du deine USt-IdNr. verwendet hast (§1a Abs. 3 und 4 UStG). Das prüfst
            du selbst; Klein.Buch erfasst nur den Betrag laut Rechnung.
          </div>
        {/if}
        <div class="field">
          <label for="imp-account">Zahlungs-Konto</label>
          <select id="imp-account" class="kb-input" bind:value={paidFromAccountId}>
            <option value="">— keines —</option>
            {#each accounts as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
          </select>
        </div>
        <label class="chk span2">
          <input
            type="checkbox"
            bind:checked={reverseCharge13b}
            onchange={onReverseChargeChange}
            aria-describedby={reverseCharge13b ? "imp-rc-hint" : undefined}
          />
          <span>§13b Reverse-Charge (Steuerschuldnerschaft des Leistungsempfängers)</span>
        </label>
        {#if reverseCharge13b}
          <div id="imp-rc-hint" class="info-banner span2 rc-hint">
            <strong>§13b aktiv:</strong> {REVERSE_CHARGE_13B_HINT}
            <span class="rc-facts">USt-Betrag 0 · Brutto = Netto ({euro(netCents ?? 0)})</span>
          </div>
        {/if}
        <div class="field span2">
          <label for="imp-notes">Notiz (optional)</label>
          <textarea id="imp-notes" class="kb-input" rows="3" bind:value={notes}></textarea>
        </div>
      </div>

      <!-- U4 (V1-05): Folge direkt über dem Knopf; Knopf heißt wie bei der manuellen Erfassung. -->
      <p id="imp-end-note" class="end-note">Danach nicht mehr änderbar — Korrektur nur per Storno.</p>
      <div class="actions">
        <a class="btn-secondary" href="/expenses">Abbrechen</a>
        <button type="button" class="btn-secondary" onclick={() => { if (!busy) reset(); }} aria-disabled={busy ? "true" : undefined}>Andere Datei</button>
        <button type="submit" class="btn-primary" aria-disabled={busy ? "true" : undefined} aria-busy={busy} aria-describedby="imp-end-note">
          {busy ? "Schreibt fest …" : "Kosten festschreiben"}
        </button>
      </div>
    </form>
  </section>
{/if}

<style>
  .muted { color: var(--text-muted); }
  /* .caveat / .card / .info-banner / .kb-input: globale Definitionen aus tokens.css. */
  .src { font-size: var(--fs-sm); color: var(--text-muted); margin: var(--sp-1) 0 var(--sp-3); overflow-wrap: anywhere; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); align-items: start; }
  @media (max-width: 600px) { .grid { grid-template-columns: 1fr; } }
  .span2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .field > label:first-child { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .legend { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--sp-3); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .lbl-hint { font-weight: 400; color: var(--text-muted); font-size: var(--fs-xs); }
  .msg { font-size: var(--fs-sm); margin: 0; line-height: 1.4; overflow-wrap: anywhere; }
  .msg.err { color: var(--status-danger-fg); font-weight: 600; }
  .msg.hint { color: var(--text-muted); margin-top: var(--sp-2); }
  .msg.note { color: var(--status-warning-fg); background: var(--status-warning-bg); border-left: 3px solid var(--status-warning-line); padding: var(--sp-1) var(--sp-2); }
  label.chk { display: flex; flex-direction: row; align-items: center; gap: var(--sp-2); min-height: var(--target); font-size: var(--fs-base); color: var(--text); }
  .chk-inline { display: inline-flex; align-items: center; gap: var(--sp-2); min-height: var(--target); font-size: var(--fs-sm); color: var(--text-muted); }
  .ro { background: var(--surface-sunken); font-weight: 600; display: block; }
  .info-banner, .caveat { margin: 0; }
  .rc-hint strong { margin-right: var(--sp-1); }
  .rc-facts { display: block; margin-top: var(--sp-1); font-weight: 600; font-variant-numeric: tabular-nums; }
  .grid :global(.money-field.with-label), .grid :global(.contact-select.with-label) { margin-bottom: 0; }
  .grid :global(.money-field label), .grid :global(.contact-select label) { color: var(--text); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--sp-3); margin-top: var(--sp-5); }
  .end-note + .actions { margin-top: var(--sp-2); }
  .end-note { margin: var(--sp-5) 0 0; font-size: var(--type-small); color: var(--text-muted); }
  .findings { font-size: var(--fs-sm); margin: 0 0 1rem; }
  .findings summary { cursor: pointer; color: var(--text-muted); min-height: var(--target); display: flex; align-items: center; }
  .findings ul { margin: var(--sp-2) 0 0; padding-left: 1rem; }
  .findings li { margin: var(--sp-1) 0; line-height: 1.4; overflow-wrap: anywhere; }
  .findings code { background: var(--surface-sunken); padding: 0 0.25rem; border-radius: 3px; font-size: var(--fs-xs); }
  .sev { font-size: var(--fs-xs); font-weight: 600; padding: 0.05rem 0.4rem; border-radius: 3px; margin-right: 0.3rem; }
  .sev-error { background: var(--status-danger-bg); color: var(--status-danger-fg); }
  .sev-warning { background: var(--status-warning-bg); color: var(--status-warning-fg); }
  .sev-info { background: var(--status-info-bg); color: var(--status-info-fg); }
</style>
