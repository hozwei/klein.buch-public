<script lang="ts">
  // Block P1: Anfahrtspauschale — Kilometersatz pflegen.
  // U1: Betrag über MoneyInput (Integer-Cent, deutsche Leseregel, ADR 0041 Nr. 15/16).
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import { travelSettingsGet, travelSettingsSet } from "#lib/api.ts";
  import { flash } from "#lib/toast.svelte.ts";

  let loading = $state(true);
  let saving = $state(false);
  let rateCents = $state<number | null>(0); // Cent je km
  let roundTripDefault = $state(false);
  let rateInput: MoneyInput | undefined = $state();
  let rateError = $state("");

  onMount(async () => {
    try {
      const s = await travelSettingsGet();
      rateCents = s.costPerKmCents;
      roundTripDefault = s.roundTripDefault;
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loading = false;
    }
  });

  async function save() {
    rateError = "";
    if (rateInput && !rateInput.validate()) {
      rateInput.focus();
      return;
    }
    if (rateCents == null) {
      rateError = "Bitte gib einen Satz ein (0 für keine Pauschale).";
      rateInput?.focus();
      return;
    }
    saving = true;
    try {
      await travelSettingsSet(rateCents, roundTripDefault);
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
  <p class="muted" aria-busy="true">Lädt …</p>
{:else}
  <form id="travel-form" onsubmit={(e) => { e.preventDefault(); save(); }} novalidate>
    <section class="card">
      <h2>Kilometersatz</h2>
      <p class="muted">
        Lege fest, was ein gefahrener Kilometer kostet. In Angeboten und Rechnungen
        kannst du dann „Anfahrt“ wählen und die Kilometer eingeben — der Betrag wird
        automatisch als Position eingetragen.
      </p>
      <div class="grid">
        <MoneyInput
          bind:this={rateInput}
          bind:cents={rateCents}
          label="Preis pro Kilometer"
          hint="Betrag in Euro je km, z. B. 0,30"
          error={rateError || undefined}
          required
          enterkeyhint="done"
        />
        <Toggle
          bind:checked={roundTripDefault}
          label="Standardmäßig Hin & Rück (×2) vorschlagen"
        />
      </div>
    </section>
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; align-items: start; margin-top: 0.5rem; }
  .muted { color: var(--text-muted); }
</style>
