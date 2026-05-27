<script lang="ts">
  import { onMount } from "svelte";
  import { privateMovementsList } from "$lib/api";
  import type { PrivateMovementListItem } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { movementTypeLabel } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import PageBar from "$lib/PageBar.svelte";

  let items = $state<PrivateMovementListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      items = await privateMovementsList();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<PageBar title="Privat-Geld (Entnahmen & Einlagen)">
  {#snippet actions()}
    <Button variant="primary" href="/private-movements/new">+ Neue Buchung</Button>
  {/snippet}
</PageBar>

<p class="muted">
  Geld, das du privat aus dem Geschäft nimmst oder einlegst. Das ist
  <strong>keine Einnahme und keine Ausgabe</strong> — es taucht in der Steuer
  nicht auf, hält aber deine Kasse vollständig. Einmal erfasst, lässt es sich
  nicht mehr ändern; korrigieren kannst du mit einer Gegenbuchung.
</p>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if items.length === 0}
  <p class="muted">Noch keine Privatbewegungen erfasst.</p>
{:else}
  <table class="kb-table">
    <thead>
      <tr><th>Beleg-Nr.</th><th>Datum</th><th>Art</th><th>Konto</th><th>Beschreibung</th><th class="num">Betrag</th></tr>
    </thead>
    <tbody>
      {#each items as m (m.id)}
        <tr>
          <td>{m.movementNumber}</td>
          <td>{date(m.movementDate)}</td>
          <td>
            <Badge tone={m.movementType === "entnahme" ? "danger" : "success"}>
              {movementTypeLabel(m.movementType)}
            </Badge>
          </td>
          <td>{m.accountLabel ?? "—"}</td>
          <td>{m.description}</td>
          <td class="num {m.movementType === 'entnahme' ? 'neg' : 'pos'}">
            {m.movementType === "entnahme" ? "−" : "+"}{euro(m.amountCents)}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  table { width: 100%; }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.neg { color: var(--c-danger-700); }
  td.pos { color: var(--c-success-700); }
  .muted { color: var(--c-text-muted); }
</style>
