<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { invoicesGet, invoicesRecordPayment, paymentAccountsList } from "#lib/api.ts";
  import type { InvoiceDetail, PaymentAccount } from "#lib/types.ts";
  import { euro, iban } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // R5-002: `today` als Obergrenze für das Zahldatum (Zufluss-Prinzip §11
  // EStG — kein Zukunftsdatum). Beim Mount fix; reicht für eine Page, die
  // pro Vorgang einmal aufgerufen wird. Backend setzt denselben Floor.
  const today = todayBerlin();

  // Betrag in Cent (MoneyInput, U1); `null` = leer oder ungültig.
  let amountCents = $state<number | null>(0);
  let amountRef: MoneyInput | undefined = $state();
  // Nach dem ersten Absende-Versuch zeigen die Felder ihre Meldungen.
  let submitted = $state(false);
  let paidDate = $state(today);
  let note = $state("");
  let payInFull = $state(true);
  // R2-011: Konto des Zahlungseingangs (DATEV-Geldkonto); Standardkonto vorbelegt.
  let accounts = $state<PaymentAccount[]>([]);
  let accountId = $state("");

  let id = $derived(page.params.id ?? "");
  let outstanding = $derived(detail
    ? detail.invoice.grossAmountCents - detail.invoice.paidAmountCents
    : 0);
  let canPay = $derived(
    !!detail &&
      !!detail.invoice.lockedAt &&
      detail.invoice.status !== "canceled" &&
      detail.invoice.status !== "paid",
  );

  async function load() {
    loading = true;
    try {
      detail = await invoicesGet(id);
      if (!detail) error = "Rechnung nicht gefunden.";
      else amountCents = outstanding;
      accounts = await paymentAccountsList(false);
      const def = accounts.find((a) => a.isDefault === 1);
      if (def) accountId = def.id;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  $effect(() => {
    if (payInFull && detail) amountCents = outstanding;
  });

  // Feldfehler (der Server prüft dasselbe).
  let errAmount = $derived(
    amountCents == null
      ? ""
      : amountCents <= 0
        ? "Der Betrag muss größer als 0 € sein."
        : amountCents > outstanding
          ? `Der Betrag ist höher als der offene Betrag (${euro(outstanding)}).`
          : "",
  );
  let errDate = $derived(
    !paidDate
      ? "Bitte das Zahlungsdatum angeben."
      : paidDate > today
        ? "Das Zahlungsdatum darf nicht in der Zukunft liegen (Zufluss-Prinzip, § 11 EStG)."
        : "",
  );
  let selectedAccount = $derived(accounts.find((a) => a.id === accountId) ?? null);

  async function submit() {
    if (!detail) return;
    submitted = true;
    const amountOk = amountRef?.validate() ?? true;
    // R5-002: Future-Datum auch im Submit abfangen (max-Attribut allein wird
    // bei manueller Eingabe umgangen). Backend setzt denselben Floor.
    if (!amountOk || amountCents == null || errAmount || errDate) {
      await tick();
      document.querySelector<HTMLElement>('#payment-form [aria-invalid="true"]')?.focus();
      return;
    }
    const cents = amountCents;
    submitting = true;
    try {
      await invoicesRecordPayment({
        invoiceId: detail.invoice.id,
        amountCents: cents,
        paidDate,
        note: note.trim() || null,
        accountId: accountId || null,
      });
      flash("Zahlung gebucht.");
      goto(`/invoices/${detail.invoice.id}`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      submitting = false;
    }
  }
</script>

<PageBar back={`/invoices/${id}`} backLabel="Rechnung" title="Zahlung erfassen">
  {#snippet actions()}
    {#if canPay}
      <a href={`/invoices/${id}`} class="btn-secondary">Abbrechen</a>
      <button type="submit" form="payment-form" class="btn-primary" disabled={submitting}>
        {submitting ? "Speichere …" : "Zahlung buchen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if !detail}
  <Banner>{error ?? "Rechnung nicht gefunden."}</Banner>
{:else}
  {@const inv = detail.invoice}
  {#if !inv.lockedAt}
    <Banner kind="warning">Zahlungen können nur auf ausgestellten Rechnungen erfasst werden.</Banner>
  {:else if inv.status === "canceled"}
    <Banner kind="warning">Stornierte Rechnungen können keine Zahlungen mehr aufnehmen.</Banner>
  {:else if inv.status === "paid"}
    <Banner kind="info">Rechnung ist bereits vollständig bezahlt.</Banner>
  {:else}
    <section class="card">
      <h2>Rechnung</h2>
      <dl>
        <dt>Nummer</dt><dd class="doc-no">{inv.invoiceNumber}</dd>
        <dt>Brutto</dt><dd>{euro(inv.grossAmountCents)}</dd>
        <dt>Bisher bezahlt</dt><dd>{euro(inv.paidAmountCents)}</dd>
        <dt>Offen</dt><dd><strong>{euro(outstanding)}</strong></dd>
      </dl>
    </section>

    <section class="card">
      <h2>Zahlung</h2>
      <p class="info">
        <strong>Wichtig fürs Finanzamt:</strong> Trag das Datum ein, an dem das Geld
        tatsächlich da war (laut Kontoauszug). Danach richtet sich das Steuerjahr.
      </p>
      <form id="payment-form" onsubmit={(e) => { e.preventDefault(); submit(); }} novalidate>
        <p class="req-legend">Felder mit <span aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
        <MoneyInput
          bind:this={amountRef}
          bind:cents={amountCents}
          label="Betrag"
          required
          readonly={payInFull}
          showErrors={submitted}
          error={submitted && errAmount ? errAmount : undefined}
          hint={payInFull ? "Der komplette offene Betrag. Für eine Teilzahlung das Häkchen unten entfernen." : undefined}
          class="pay-field"
        />
        <label class="chk">
          <input type="checkbox" bind:checked={payInFull} />
          Kompletter offener Betrag ({euro(outstanding)})
        </label>
        <label>
          <span>Zahlungsdatum <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            bind:value={paidDate}
            max={today}
            required
            aria-invalid={submitted && errDate ? "true" : undefined}
            aria-describedby={submitted && errDate ? "paid-date-err" : "paid-date-hint"}
          />
          {#if submitted && errDate}
            <span id="paid-date-err" class="field-err">{errDate}</span>
          {:else}
            <span id="paid-date-hint" class="field-hint">Der Tag, an dem das Geld laut Kontoauszug eingegangen ist.</span>
          {/if}
        </label>
        <label>
          Zahlungs-Konto
          <select bind:value={accountId} aria-describedby="pay-account-hint">
            <option value="">Kein Konto zuordnen</option>
            {#each accounts as a (a.id)}
              <option value={a.id}>{a.label}{a.isDefault === 1 ? " (Standard)" : ""}</option>
            {/each}
          </select>
          <span id="pay-account-hint" class="field-hint">
            {#if selectedAccount?.iban}
              Eingang auf {iban(selectedAccount.iban)}.
            {/if}
            Das Konto bestimmt das Geldkonto im DATEV-Export.
            {#if accounts.length === 0}Konten legst du unter <a class="txt-link" href="/settings/payment-accounts">Einstellungen → Zahlungskonten</a> an.{/if}
          </span>
        </label>
        <label>
          Notiz (optional)
          <input type="text" bind:value={note} autocomplete="off" aria-describedby="pay-note-hint" />
          <span id="pay-note-hint" class="field-hint">Zum Beispiel Verwendungszweck oder Position im Kontoauszug.</span>
        </label>
      </form>
    </section>
  {/if}
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 9rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: var(--text-muted); font-size: var(--fs-sm); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; font-variant-numeric: tabular-nums; }
  .doc-no { font-family: var(--font-mono); }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); margin-bottom: 0.75rem; max-width: 32rem; }
  :global(.pay-field) { max-width: 32rem; }
  .chk { flex-direction: row; align-items: center; gap: 0.4rem; min-height: var(--target); font-weight: 400; color: var(--text); }
  input:not([type="checkbox"]), select { padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md); font: inherit; font-weight: 400; color: var(--text); }
  .req-legend { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .field-err { font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  .txt-link { text-decoration: underline; text-underline-offset: 2px; }
  .info { background: var(--status-info-bg); color: var(--text); padding: 0.75rem 1rem; border-left: 4px solid var(--status-info-line); border-radius: var(--r-sm); font-size: var(--fs-base); }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .muted { color: var(--text-muted); }
</style>
