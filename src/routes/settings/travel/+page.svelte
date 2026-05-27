<script lang="ts">
  // Block P1: Anfahrtspauschale — Kilometersatz pflegen.
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import { travelSettingsGet, travelSettingsSet } from "$lib/api";
  import { flash } from "$lib/toast.svelte";

  let loading = $state(true);
  let saving = $state(false);
  let rateEuro = $state(0); // €/km im Eingabefeld
  let roundTripDefault = $state(false);

  onMount(async () => {
    try {
      const s = await travelSettingsGet();
      rateEuro = s.costPerKmCents / 100;
      roundTripDefault = s.roundTripDefault;
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loading = false;
    }
  });

  async function save() {
    const cents = Math.round(rateEuro * 100);
    if (cents < 0) {
      flash("Der Satz darf nicht negativ sein.", "error");
      return;
    }
    saving = true;
    try {
      await travelSettingsSet(cents, roundTripDefault);
      flash("Gespeichert.");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      saving = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Anfahrtspauschale">
  {#snippet actions()}
    <button type="submit" form="travel-form" class="btn-primary" disabled={saving}>
      {saving ? "Speichere …" : "Speichern"}
    </button>
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else}
  <form id="travel-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
    <section class="card">
      <h2>Kilometersatz</h2>
      <p class="muted">
        Lege fest, was ein gefahrener Kilometer kostet. In Angeboten und Rechnungen
        kannst du dann „Anfahrt" wählen und die Kilometer eingeben — der Betrag wird
        automatisch als Position eingetragen.
      </p>
      <div class="grid">
        <label>
          Preis pro Kilometer (€)
          <input type="number" step="0.01" min="0" bind:value={rateEuro} />
        </label>
        <Toggle
          bind:checked={roundTripDefault}
          label="Standardmäßig Hin &amp; Rück (×2) vorschlagen"
        />
      </div>
    </section>
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; align-items: end; margin-top: 0.5rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  input { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font: inherit; }
  .muted { color: #6b7280; }
</style>
