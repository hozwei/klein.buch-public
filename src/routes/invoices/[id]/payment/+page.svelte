<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { invoicesGet, invoicesRecordPayment } from "$lib/api";
  import type { InvoiceDetail } from "$lib/types";
  import { euro, date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // R5-002: `today` als Obergrenze für das Zahldatum (Zufluss-Prinzip §11
  // EStG — kein Zukunftsdatum). Beim Mount fix; reicht für eine Page, die
  // pro Vorgang einmal aufgerufen wird. Backend setzt denselben Floor.
  const today = new Date().toISOString().slice(0, 10);

  let amountEuros = $state(0);
  let paidDate = $state(today);
  let note = $state("");
  let payInFull = $state(true);

  let id = $derived($page.params.id ?? "");
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
      else amountEuros = outstanding / 100;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  $effect(() => {
    if (payInFull && detail) amountEuros = outstanding / 100;
  });

  async function submit() {
    if (!detail) return;
    const cents = Math.round(amountEuros * 100);
    if (cents <= 0) {
      flash("Betrag muss größer als 0 € sein.", "error");
      return;
    }
    if (!paidDate) {
      flash("Zahlungsdatum ist Pflicht.", "error");
      return;
    }
    // R5-002: Future-Datum auch im Submit abfangen (max-Attribut allein wird
    // bei manueller Eingabe umgangen). Backend setzt denselben Floor.
    if (paidDate > today) {
      flash("Das Zahldatum darf nicht in der Zukunft liegen (Zufluss-Prinzip §11 EStG).", "error");
      return;
    }
    if (cents > outstanding) {
      flash(`Der Betrag (${euro(cents)}) ist höher als der offene Betrag (${euro(outstanding)}).`, "error");
      return;
    }
    submitting = true;
    try {
      await invoicesRecordPayment({
        invoiceId: detail.invoice.id,
        amountCents: cents,
        paidDate,
        note: note.trim() || null,
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
        <dt>Nummer</dt><dd>{inv.invoiceNumber}</dd>
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
        <label>
          Betrag (€) *
          <input type="number" step="0.01" min="0.01" bind:value={amountEuros}
                 disabled={payInFull} required />
        </label>
        <label class="chk">
          <input type="checkbox" bind:checked={payInFull} />
          Kompletter offener Betrag ({euro(outstanding)})
        </label>
        <label>
          Zahlungsdatum *
          <input type="date" bind:value={paidDate} max={today} required />
        </label>
        <label>
          Notiz (optional)
          <input type="text" bind:value={note}
                 placeholder="z. B. Verwendungszweck oder Position im Kontoauszug" />
        </label>
      </form>
    </section>
  {/if}
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 9rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: #6b7280; font-size: 0.85rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; margin-bottom: 0.75rem; }
  .chk { flex-direction: row; align-items: center; gap: 0.4rem; }
  input { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; }
  input:disabled { background: #f3f4f6; color: #6b7280; }
  .info { background: #dbeafe; padding: 0.75rem 1rem; border-left: 4px solid #2563eb; border-radius: 4px; font-size: 0.9rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .muted { color: #6b7280; }
</style>
