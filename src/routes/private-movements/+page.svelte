<script lang="ts">
  import { onMount } from "svelte";
  import { privateMovementsList } from "#lib/api.ts";
  import type { PrivateMovementListItem } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { movementTypeLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";

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
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if items.length === 0}
  <div class="empty">
    <p class="muted">Noch keine Privatbewegungen erfasst.</p>
    <Button variant="primary" href="/private-movements/new">Erste Privat-Buchung erfassen</Button>
  </div>
{:else}
  <Table caption="Privatentnahmen und -einlagen" cards>
    <thead>
      <tr><th>Beleg-Nr.</th><th>Datum</th><th>Art</th><th>Konto</th><th>Beschreibung</th><th class="num">Betrag</th></tr>
    </thead>
    <tbody>
      {#each items as m (m.id)}
        <tr>
          <td data-label="Beleg-Nr."><span class="doc-no">{m.movementNumber}</span></td>
          <td data-label="Datum">{date(m.movementDate)}</td>
          <td data-label="Art">
            <Badge tone={m.movementType === "entnahme" ? "danger" : "success"}>
              {movementTypeLabel(m.movementType)}
            </Badge>
          </td>
          <td data-label="Konto">{m.accountLabel ?? "—"}</td>
          <td class="cards-full wrap" data-label="Beschreibung">{m.description}</td>
          <td class="num kb-num {m.movementType === 'entnahme' ? 'neg' : 'pos'}" data-label="Betrag">
            {m.movementType === "entnahme" ? "−" : "+"}{euro(m.amountCents)}
          </td>
        </tr>
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.neg { color: var(--status-danger-fg); }
  td.pos { color: var(--status-success-fg); }
  td.wrap { overflow-wrap: anywhere; }
  .doc-no { font-family: var(--font-mono); white-space: nowrap; }
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-3); margin: var(--sp-3) 0; }
  .empty p { margin: 0; }
  .muted { color: var(--text-muted); }
</style>
