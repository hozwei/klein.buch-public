<script lang="ts">
  import { onMount } from "svelte";
  import { privateMovementsList } from "#lib/api.ts";
  import type { PrivateMovementListItem } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { movementTypeLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
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

<!-- U4 (V2-15): Bereich „Privatbewegungen“ (Navigation, Titel, Zurück; wie das Handbuch-Kapitel), Objekt
     „Privatbewegung“ wie im Glossar. -->
<PageBar title="Privatbewegungen">
  {#snippet actions()}
    <Button variant="primary" href="/private-movements/new">+ Neue Privatbewegung</Button>
    <HelpAnchor slug="privatbewegungen" />
  {/snippet}
</PageBar>

<p class="muted">
  Privatentnahmen und Privateinlagen: Geld, das du privat aus dem Geschäft nimmst oder
  einlegst. Das ist <strong>keine Einnahme und keine Ausgabe</strong> — es taucht in der
  Steuer nicht auf, hält aber deine Kasse vollständig. Einmal festgeschrieben, lässt es
  sich nicht mehr ändern; korrigieren kannst du mit einer Gegenbuchung.
</p>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if items.length === 0}
  <!-- U4 (V2-09): Erststart mit derselben Beschriftung wie die PageBar, als secondary. -->
  <div class="empty">
    <p class="muted">Noch keine Privatbewegungen erfasst.</p>
    <Button variant="secondary" href="/private-movements/new">+ Neue Privatbewegung</Button>
  </div>
{:else}
  <Table caption="Privatentnahmen und -einlagen" cards cardGrid>
    <thead>
      <tr><th>Nummer</th><th>Datum</th><th>Art</th><th class="kb-col-opt1">Konto</th><th>Beschreibung</th><th class="num">Betrag</th><th><span class="kb-visually-hidden">Aktion</span></th></tr>
    </thead>
    <tbody>
      {#each items as m (m.id)}
        <tr>
          <!-- U4 (F6): Nummer führt zur Detailseite (Beleg, Notiz, Erfassungszeit). -->
          <td data-label="Nummer"><DocNo value={m.movementNumber} href={`/private-movements/${m.id}`} /></td>
          <td data-label="Datum">{date(m.movementDate)}</td>
          <!-- U4 (V2-05): Die Art ist kein Status — neutrales Badge, der Text trägt sie;
               der Betrag zeigt die Richtung mit Vorzeichen. -->
          <td data-label="Art">
            <Badge kind="tag">{movementTypeLabel(m.movementType)}</Badge>
          </td>
          <td class="kb-col-opt1" data-label="Konto">{m.accountLabel ?? "—"}</td>
          <td class="cards-full wrap" data-label="Beschreibung">{m.description}</td>
          <td class="num kb-num" data-label="Betrag">
            {m.movementType === "entnahme" ? "−" : "+"}{euro(m.amountCents)}
          </td>
          <td class="right">
            <Button variant="secondary" size="sm" href={`/private-movements/${m.id}`} ariaLabel={`Privatbewegung ${m.movementNumber} öffnen`}>Öffnen</Button>
          </td>
        </tr>
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.right { text-align: right; }
  /* U2-Regel (ADR 0041 Nr. 44): Beschreibungen brechen mit Silbentrennung, nicht mitten im Wort. */
  td.wrap { overflow-wrap: break-word; hyphens: auto; }
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-3); margin: var(--sp-3) 0; }
  .empty p { margin: 0; }
  .muted { color: var(--text-muted); }
</style>
