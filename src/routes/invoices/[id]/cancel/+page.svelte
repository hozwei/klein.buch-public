<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { invoicesGet, invoicesCancel } from "$lib/api";
  import type { InvoiceDetail } from "$lib/types";
  import { euro, date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let submitting = $state(false);
  let error = $state<string | null>(null);
  let reason = $state("");
  let stornoDate = $state(new Date().toISOString().slice(0, 10));

  let id = $derived($page.params.id ?? "");
  let canCancel = $derived(
    !!detail &&
      detail.invoice.status !== "canceled" &&
      !!detail.invoice.lockedAt &&
      !detail.invoice.isStornoFor,
  );

  async function load() {
    loading = true;
    try {
      detail = await invoicesGet(id);
      if (!detail) error = "Rechnung nicht gefunden.";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function submit() {
    if (!detail) return;
    if (!stornoDate) {
      flash("Storno-Datum ist Pflicht.", "error");
      return;
    }
    if (!reason.trim()) {
      flash("Bitte gib einen Grund an — das ist gesetzlich vorgeschrieben.", "error");
      return;
    }
    submitting = true;
    try {
      const resp = await invoicesCancel({
        invoiceId: detail.invoice.id,
        reason: reason.trim(),
        stornoDate,
      });
      // Springe zur frisch erzeugten Storno-Rechnung.
      flash("Storno-Beleg erstellt.");
      goto(`/invoices/${resp.storno.invoice.id}`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      submitting = false;
    }
  }
</script>

<PageBar back={`/invoices/${id}`} backLabel="Rechnung" title="Rechnung stornieren">
  {#snippet actions()}
    {#if canCancel}
      <a href={`/invoices/${id}`} class="btn-secondary">Abbrechen</a>
      <button type="submit" form="cancel-form" class="btn-danger" disabled={submitting}>
        {submitting ? "Storniere …" : "Storno-Beleg erstellen"}
      </button>
    {/if}
    <HelpAnchor slug="storno-statt-loeschung" />
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if !detail}
  <Banner>{error ?? "Rechnung nicht gefunden."}</Banner>
{:else}
  {@const inv = detail.invoice}
  {#if inv.status === "canceled"}
    <Banner kind="warning">Diese Rechnung ist bereits storniert.</Banner>
  {:else if !inv.lockedAt}
    <Banner kind="warning">Entwürfe kann man nicht stornieren — lösch ihn einfach direkt.</Banner>
  {:else if inv.isStornoFor}
    <Banner kind="warning">Eine Storno-Rechnung lässt sich nicht noch einmal stornieren.</Banner>
  {:else}
    <section class="card">
      <h2>Original-Rechnung</h2>
      <dl>
        <dt>Nummer</dt><dd>{inv.invoiceNumber}</dd>
        <dt>Datum</dt><dd>{date(inv.invoiceDate)}</dd>
        <dt>Brutto</dt><dd>{euro(inv.grossAmountCents)}</dd>
        <dt>Empfänger</dt><dd>{detail.buyer?.name ?? "—"}</dd>
      </dl>
    </section>

    <section class="card">
      <h2>Storno erstellen</h2>
      <p class="info">
        Es wird eine <strong>neue</strong> Rechnung mit negativem Betrag erstellt, die
        die Original-Rechnung ausgleicht. Die Original-Rechnung wird <strong>nicht
        gelöscht</strong> — sie bleibt erhalten und gilt ab jetzt als storniert. So
        verlangt es das Gesetz.
      </p>
      <form id="cancel-form" onsubmit={(e) => { e.preventDefault(); submit(); }} novalidate>
        <label>
          Storno-Datum *
          <input type="date" bind:value={stornoDate} required />
        </label>
        <label>
          Grund * (steht später auf dem Storno-Beleg)
          <textarea
            rows="4"
            bind:value={reason}
            required
            placeholder="z. B. Falscher Empfänger / Rechnung doppelt ausgestellt / Konditionen neu vereinbart"
          ></textarea>
        </label>
      </form>
    </section>
  {/if}
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 8rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: #6b7280; font-size: 0.85rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; margin-bottom: 1rem; }
  input, textarea {
    padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px;
    font-size: 0.95rem; font-family: inherit;
  }
  textarea { resize: vertical; }
  .info { background: #fef3c7; padding: 0.75rem 1rem; border-left: 4px solid #f59e0b; border-radius: 4px; font-size: 0.9rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .muted { color: #6b7280; }
</style>
