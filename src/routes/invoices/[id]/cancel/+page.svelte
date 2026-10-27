<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { invoicesGet, invoicesCancel } from "#lib/api.ts";
  import type { InvoiceDetail } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import DocNo from "#lib/DocNo.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import { parsePaymentHistory } from "../../paymentHistory.ts";
  import PageBar from "#lib/PageBar.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let submitting = $state(false);
  let error = $state<string | null>(null);
  let reason = $state("");
  let stornoDate = $state(todayBerlin());
  const today = todayBerlin();
  // Nach dem ersten Absende-Versuch zeigen die Felder ihre Meldungen.
  let submitted = $state(false);

  let id = $derived(page.params.id ?? "");
  let canCancel = $derived(
    !!detail &&
      detail.invoice.status !== "canceled" &&
      !!detail.invoice.lockedAt &&
      !detail.invoice.isStornoFor,
  );
  // Lauf F8 (§ 11 EStG): Die Erstattung kann erst nach dem Zufluss erfolgen —
  // frühestes Storno-Datum ist der späteste von Rechnungsdatum und letztem
  // Zahlungseingang (der Server prüft dasselbe).
  let minStornoDate = $derived(detail ? earliestStornoDate(detail) : "");

  function earliestStornoDate(d: InvoiceDetail): string {
    let min = d.invoice.invoiceDate.slice(0, 10);
    if (d.invoice.paidAmountCents <= 0) return min;
    const dates: string[] = [];
    // Beschädigte Historie (null): der Server lehnt mit klarer Meldung ab.
    for (const e of parsePaymentHistory(d.invoice.paymentHistoryJson) ?? []) {
      if (e.amountCents !== 0) dates.push(e.paidDate.slice(0, 10));
    }
    if (dates.length === 0 && d.invoice.paidAt) dates.push(d.invoice.paidAt.slice(0, 10));
    for (const x of dates) if (x > min) min = x;
    return min;
  }

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

  // Lauf F8: Meldung am Datumsfeld statt Toast; der Server prüft dasselbe.
  let errDate = $derived(
    !stornoDate
      ? "Bitte das Storno-Datum angeben."
      : minStornoDate && stornoDate < minStornoDate
        ? `Das Storno-Datum liegt frühestens am ${date(minStornoDate)} — am Rechnungsdatum bzw. am letzten Zahlungseingang.`
        : stornoDate > today
          ? "Das Storno-Datum darf nicht in der Zukunft liegen."
          : "",
  );
  let errReason = $derived(reason.trim() ? "" : "Bitte einen Grund angeben — er ist gesetzlich vorgeschrieben und steht auf dem Storno-Beleg.");

  async function submit() {
    if (!detail || submitting) return;
    submitted = true;
    if (errDate || errReason) {
      await tick();
      document.querySelector<HTMLElement>('#cancel-form [aria-invalid="true"]')?.focus();
      return;
    }
    // V1-17 (design.md §5): Stornieren ist endgültig — Rückfrage wie beim Ausstellen.
    const nr = detail.invoice.invoiceNumber;
    if (
      !(await confirmDialog({
        title: `Rechnung ${nr} stornieren?`,
        bullets: [
          `Es entsteht ein Storno-Beleg mit eigener Nummer (ST-…) und Datum ${date(stornoDate)}, der die Rechnung ausgleicht.`,
          "Das lässt sich nicht rückgängig machen.",
          "Die Rechnung wird nicht gelöscht: Sie bleibt erhalten und erscheint als storniert.",
          "Schick den Storno-Beleg danach an den Kunden.",
        ],
        confirmLabel: "Stornieren",
        danger: true,
      }))
    )
      return;
    submitting = true;
    try {
      const resp = await invoicesCancel({
        invoiceId: detail.invoice.id,
        reason: reason.trim(),
        stornoDate,
      });
      // Springe zum frisch erzeugten Storno-Beleg; dort ist „Senden“ der nächste Schritt (V1-07).
      flash(`Storno-Beleg ${resp.storno.invoice.invoiceNumber} erstellt. Schick ihn jetzt an den Kunden.`);
      goto(`/invoices/${resp.storno.invoice.id}`);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      submitting = false;
    }
  }
</script>

<PageBar
  back={`/invoices/${id}`}
  backLabel="Rechnung"
  title={detail ? `Rechnung ${detail.invoice.invoiceNumber} stornieren` : "Rechnung stornieren"}
>
  {#snippet actions()}
    {#if canCancel}
      <a href={`/invoices/${id}`} class="btn-secondary">Abbrechen</a>
      <!-- V2-06: während des Stornierens fokussierbar (aria-disabled), Klick ohne Wirkung. -->
      <Button variant="danger" type="submit" form="cancel-form" ariaDisabled={submitting}>
        {submitting ? "Storniere …" : "Storno-Beleg erstellen"}
      </Button>
    {/if}
    <HelpAnchor slug="storno-statt-loeschung" />
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if !detail}
  <Banner>{error ?? "Rechnung nicht gefunden."}</Banner>
{:else}
  {@const inv = detail.invoice}
  {#if inv.status === "canceled"}
    <Banner kind="warning">Diese Rechnung ist bereits storniert.</Banner>
  {:else if !inv.lockedAt}
    <Banner kind="warning">
      Einen Entwurf stornierst du nicht: Du kannst ihn bearbeiten oder auf der
      <a class="kb-link" href={`/invoices/${id}`}>Rechnungsseite</a> mit Grund verwerfen.
    </Banner>
  {:else if inv.isStornoFor}
    <Banner kind="warning">Ein Storno-Beleg lässt sich nicht noch einmal stornieren.</Banner>
  {:else}
    <section class="card">
      <h2>Original-Rechnung</h2>
      <dl>
        <dt>Nummer</dt><dd><DocNo value={inv.invoiceNumber} /></dd>
        <dt>Datum</dt><dd>{date(inv.invoiceDate)}</dd>
        <dt>Brutto</dt><dd class="kb-num-l">{euro(inv.grossAmountCents)}</dd>
        <dt>Empfänger</dt><dd>{detail.buyer?.name ?? "—"}</dd>
      </dl>
    </section>

    <section class="card">
      <h2>Storno erstellen</h2>
      <p class="info">
        Es entsteht ein <strong>neuer</strong> Beleg, der Storno-Beleg: eigene Nummer (ST-…),
        negativer Betrag, er gleicht die Original-Rechnung aus. Die Original-Rechnung wird
        <strong>nicht gelöscht</strong> — sie bleibt erhalten und gilt ab jetzt als storniert. So
        verlangt es das Gesetz. Das Stornieren lässt sich nicht rückgängig machen; schick den
        Storno-Beleg danach an den Kunden.
      </p>
      <form id="cancel-form" onsubmit={(e) => { e.preventDefault(); submit(); }} novalidate>
        <p class="req-legend">Felder mit <span aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
        <label>
          <span>Storno-Datum <span class="req" aria-hidden="true">*</span></span>
          <input
            type="date"
            class="kb-w-date"
            bind:value={stornoDate}
            min={minStornoDate}
            max={today}
            required
            aria-invalid={submitted && errDate ? "true" : undefined}
            aria-describedby={submitted && errDate ? "storno-date-err" : "storno-date-hint"}
          />
          {#if submitted && errDate}
            <span id="storno-date-err" class="field-err">{errDate}</span>
          {:else}
            <span id="storno-date-hint" class="field-hint">Frühestens am {date(minStornoDate)} (Rechnungsdatum bzw. letzter Zahlungseingang), spätestens heute.</span>
          {/if}
        </label>
        <label>
          <span>Grund <span class="req" aria-hidden="true">*</span> (steht später auf dem Storno-Beleg)</span>
          <textarea
            rows="4"
            bind:value={reason}
            required
            aria-required="true"
            aria-invalid={submitted && errReason ? "true" : undefined}
            aria-describedby={submitted && errReason ? "storno-reason-err" : "storno-reason-hint"}
          ></textarea>
          {#if submitted && errReason}
            <span id="storno-reason-err" class="field-err">{errReason}</span>
          {:else}
            <span id="storno-reason-hint" class="field-hint">Zum Beispiel: falscher Empfänger, Rechnung doppelt ausgestellt, Konditionen neu vereinbart.</span>
          {/if}
        </label>
        <!-- V1-06 (ADR 0041 Nr. 35): Hauptaktion am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
        <div class="kb-form-end">
          <Button variant="danger" type="submit" ariaDisabled={submitting}>{submitting ? "Storniere …" : "Storno-Beleg erstellen"}</Button>
        </div>
      </form>
    </section>
  {/if}
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  /* Labelspalte so breit wie das längste Label (U3-037). */
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 0.25rem 0.75rem; }
  dt { color: var(--text-muted); font-size: var(--fs-sm); white-space: nowrap; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  .kb-num-l { font-variant-numeric: tabular-nums; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); margin-bottom: 1rem; }
  input, textarea {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text);
  }
  textarea { resize: vertical; }
  .req-legend { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .field-err { font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  .info { background: var(--status-warning-bg); color: var(--text); padding: 0.75rem 1rem; border-left: 4px solid var(--status-warning-line); border-radius: var(--r-sm); font-size: var(--fs-base); }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .muted { color: var(--text-muted); }
</style>
