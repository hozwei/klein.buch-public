<script lang="ts">
  // Privat-Buchung im Detail (U4, F6/Warteliste 14): liest `private_movements_get`.
  // Privat-Buchungen sind ab dem Erfassen festgeschrieben (GoBD) — die Seite zeigt nur
  // an und bietet den Beleg zum Herunterladen; korrigiert wird mit einer Gegenbuchung.
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { paymentAccountsList, privateMovementsGet, saveArchiveFile } from "#lib/api.ts";
  import type { PaymentAccount, PrivateMovement } from "#lib/types.ts";
  import { euro, date, dateTimeShort } from "#lib/format.ts";
  import { movementTypeLabel } from "#lib/labels.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";

  let movement = $state<PrivateMovement | null>(null);
  let accounts = $state<PaymentAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let notFound = $state(false);

  let id = $derived(page.params.id ?? "");

  async function load() {
    loading = true;
    error = null;
    notFound = false;
    try {
      movement = await privateMovementsGet(id);
      if (!movement) {
        notFound = true;
        error = "Privatbewegung nicht gefunden.";
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    try {
      accounts = await paymentAccountsList(true);
    } catch {
      /* Kontoname ist Zusatz; ohne Liste bleibt „—“. */
    }
    await load();
  });

  function accountLabel(accId: string | null): string {
    if (!accId) return "—";
    return accounts.find((a) => a.id === accId)?.label ?? "—";
  }

  let isWithdrawal = $derived(movement?.movementType === "entnahme");
</script>

<!-- U4 (V2-15): Objekt „Privatbewegung“ wie im Glossar; V4-08: h1 auch bei „nicht gefunden“. -->
<PageBar
  back="/private-movements"
  backLabel="Privatbewegungen"
  title={movement ? `Privatbewegung ${movement.movementNumber}` : notFound ? "Privatbewegung nicht gefunden" : "Privatbewegung"}
/>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner href="/private-movements" linkLabel="Zur Übersicht">{error}</Banner>
{:else if movement}
  {@const m = movement}
  <!-- U4 (V2-05): Art als neutrales Badge (kein Status), Richtung über das Vorzeichen. -->
  <p class="sub">
    <Badge kind="tag">{movementTypeLabel(m.movementType)}</Badge>
    {#if m.lockedAt}<Badge tone="primary">Festgeschrieben</Badge>{/if}
  </p>

  <section class="card" aria-label="Angaben">
    <dl>
      <div><dt>Datum</dt><dd>{date(m.movementDate)}</dd></div>
      <div><dt>Art</dt><dd>{movementTypeLabel(m.movementType)}</dd></div>
      <div>
        <dt>Betrag</dt>
        <dd class="amt"><strong>{isWithdrawal ? "−" : "+"}{euro(m.amountCents)}</strong></dd>
      </div>
      <div><dt>Konto</dt><dd>{accountLabel(m.accountId)}</dd></div>
      <div class="span2"><dt>Beschreibung</dt><dd>{m.description}</dd></div>
      {#if m.notes}<div class="span2"><dt>Notiz</dt><dd class="pre">{m.notes}</dd></div>{/if}
      <div><dt>Geschäftsjahr</dt><dd>{m.fiscalYear}</dd></div>
      <div><dt>Erfasst am</dt><dd>{dateTimeShort(m.createdAt)}</dd></div>
    </dl>
  </section>

  <section class="card" aria-labelledby="pm-receipt-h">
    <h2 id="pm-receipt-h">Beleg</h2>
    {#if m.receiptArchiveId}
      <div class="kb-actions kb-start">
        <button type="button" class="btn-secondary" onclick={() => saveArchiveFile(m.receiptArchiveId!)}>
          Beleg herunterladen
        </button>
      </div>
    {:else}
      <p class="muted">Zu dieser Privatbewegung ist kein Beleg hinterlegt.</p>
    {/if}
  </section>

  <p class="muted note">
    Eine Privatbewegung zählt nicht für die Steuer und lässt sich nicht ändern oder löschen.
    Einen Fehler korrigierst du mit einer
    <a class="kb-link" href="/private-movements/new">Gegenbuchung</a> (Entnahme gegen Einlage).
  </p>
{/if}

<style>
  /* .card / .card h2: globale Definitionen aus tokens.css. */
  .sub { margin: 0 0 var(--sp-4); display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  dl { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-3) var(--sp-5); margin: 0; }
  dl div { display: flex; flex-direction: column; min-width: 0; }
  .span2 { grid-column: 1 / -1; }
  dt { font-size: var(--type-small); color: var(--text-muted); }
  dd { margin: 0; overflow-wrap: break-word; }
  dd.amt { font-variant-numeric: tabular-nums; }
  dd.pre { white-space: pre-wrap; }
  @media (max-width: 600px) { dl { grid-template-columns: 1fr; } }
  .muted { color: var(--text-muted); }
  .note { max-width: 72ch; font-size: var(--type-small); line-height: var(--lh-body); }
</style>
