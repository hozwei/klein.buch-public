<script lang="ts">
  // Block P1: wiederverwendbarer „Anfahrt"-Block für die Positions-Editoren
  // (Rechnung, Angebot, Umwandlung). km × Satz → eine normale Position.
  import { onMount } from "svelte";
  import { travelSettingsGet, travelCompute } from "$lib/api";
  import type { TravelLine } from "$lib/types";
  import { euro } from "$lib/format";
  import { flash } from "$lib/toast.svelte";

  let { onAdd }: { onAdd: (line: TravelLine) => void } = $props();

  let rateCents = $state(0);
  let roundTrip = $state(false);
  let km = $state<number | null>(null);
  let loaded = $state(false);
  let adding = $state(false);

  onMount(async () => {
    try {
      const s = await travelSettingsGet();
      rateCents = s.costPerKmCents;
      roundTrip = s.roundTripDefault;
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loaded = true;
    }
  });

  async function add() {
    if (km == null || !(km > 0)) {
      flash("Bitte eine Kilometerzahl größer als 0 eingeben.", "error");
      return;
    }
    adding = true;
    try {
      const line = await travelCompute(km, roundTrip);
      onAdd(line);
      flash("Anfahrt-Position hinzugefügt.");
      km = null;
    } catch (e) {
      flash(String(e), "error");
    } finally {
      adding = false;
    }
  }
</script>

{#if loaded}
  <div class="travel">
    {#if rateCents <= 0}
      <span class="hint">
        Kein Kilometersatz hinterlegt —
        <a href="/settings/travel">jetzt festlegen</a>, dann lässt sich die Anfahrt automatisch berechnen.
      </span>
    {:else}
      <span class="lbl">Anfahrt</span>
      <input
        type="number"
        step="any"
        min="0"
        placeholder="km"
        class="km"
        bind:value={km}
      />
      <label class="rt">
        <input type="checkbox" bind:checked={roundTrip} /> Hin &amp; Rück
      </label>
      <span class="rate">Satz: {euro(rateCents)}/km</span>
      <button type="button" class="btn-secondary" onclick={add} disabled={adding}>
        + Anfahrt
      </button>
    {/if}
  </div>
{/if}

<style>
  .travel { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; padding: 0.6rem 0 0.2rem; }
  .lbl { font-weight: 600; font-size: 0.9rem; }
  .km { width: 6rem; padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font: inherit; }
  .rt { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.9rem; color: #4b5563; }
  .rate { color: #6b7280; font-size: 0.9rem; font-variant-numeric: tabular-nums; }
  .hint { color: #6b7280; font-size: 0.9rem; }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
</style>
