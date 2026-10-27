<script lang="ts">
  // Block P1: wiederverwendbarer „Anfahrt"-Block für die Positions-Editoren
  // (Rechnung, Angebot, Umwandlung). km × Satz → eine normale Position.
  import { onMount, tick } from "svelte";
  import { travelSettingsGet, travelCompute } from "#lib/api.ts";
  import type { TravelLine } from "#lib/types.ts";
  import { euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { randomId } from "#lib/browser.ts";
  import QuantityInput from "#lib/QuantityInput.svelte";

  const kmId = `travel-km-${randomId()}`;

  let { onAdd }: { onAdd: (line: TravelLine) => void } = $props();

  let rateCents = $state(0);
  let roundTrip = $state(false);
  let km = $state<number | null>(null);
  let loaded = $state(false);
  let adding = $state(false);
  let kmError = $state<string | undefined>(undefined);

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
    if (adding) return;
    if (km == null || !(km > 0)) {
      kmError = "Bitte eine Kilometerzahl größer als 0 eingeben.";
      // Fokus ans Feld mit der Meldung (ADR 0041 Nr. 22).
      await tick();
      document.getElementById(kmId)?.focus();
      return;
    }
    kmError = undefined;
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
      <label class="lbl" for={kmId}>Anfahrt (km)</label>
      <span class="km">
        <QuantityInput id={kmId} bind:value={km} maxFractionDigits={1} unit="km" noun="Kilometerzahl" error={kmError} />
      </span>
      <label class="rt">
        <input type="checkbox" bind:checked={roundTrip} /> Hin &amp; Rück
      </label>
      <span class="rate">Satz: {euro(rateCents)}/km</span>
      <!-- Während des Rechnens gesperrt, aber fokussierbar (ADR 0041 Nr. 33). -->
      <button type="button" class="btn-secondary" onclick={add} aria-disabled={adding ? "true" : undefined}>
        + Anfahrt
      </button>
    {/if}
  </div>
{/if}

<style>
  .travel { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; padding: 0.6rem 0 0.2rem; }
  .lbl { font-weight: 600; font-size: var(--fs-sm); }
  .km { width: var(--w-short); }
  .rt { display: inline-flex; align-items: center; gap: 0.5rem; min-height: var(--target); font-size: var(--fs-sm); color: var(--text-muted); cursor: pointer; }
  .rate { color: var(--text-muted); font-size: var(--fs-sm); font-variant-numeric: tabular-nums; }
  .hint { color: var(--text-muted); font-size: var(--fs-sm); }
  /* Link im Satz: 2.5.8-Ausnahme „inline“ (ADR 0041 Nr. 31). Vorher inline-flex mit
     44 px Mindesthöhe — das riss auf dem Handy jede Zeile des Satzes auseinander (U1-Sichtprüfung). */
  .hint a { text-decoration: underline; }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
</style>
