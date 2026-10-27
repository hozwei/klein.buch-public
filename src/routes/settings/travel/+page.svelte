<script lang="ts">
  // Block P1: Anfahrtspauschale — Kilometersatz pflegen.
  // U1: Betrag über MoneyInput (Integer-Cent, deutsche Leseregel, ADR 0041 Nr. 15/16).
  // U4 (V2-07): Nach einem Ladefehler kein vorbelegtes Formular (0 € und „aus“ hätten
  // beim Speichern den gespeicherten Satz überschrieben), sondern Fehler + „Erneut laden“.
  // U4 (V2-06/V2-11): „Abbrechen“ links, Hauptaktion zuletzt; während des Speicherns
  // `ariaDisabled` statt `disabled` (Fokus bleibt, ADR 0041 Nr. 33).
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import { travelSettingsGet, travelSettingsSet } from "#lib/api.ts";
  import { flash } from "#lib/toast.svelte.ts";

  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  let rateCents = $state<number | null>(0); // Cent je km
  let roundTripDefault = $state(false);
  let rateInput: MoneyInput | undefined = $state();
  let rateError = $state("");

  async function load() {
    loading = true;
    loadError = null;
    try {
      const s = await travelSettingsGet();
      rateCents = s.costPerKmCents;
      roundTripDefault = s.roundTripDefault;
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function save() {
    if (saving) return;
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
      flash("Anfahrtspauschale gespeichert.");
    } catch (e) {
      flash(`Speichern fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      saving = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Anfahrtspauschale">
  {#snippet actions()}
    {#if !loading && !loadError}
      <Button variant="secondary" href="/settings">Abbrechen</Button>
      <Button type="submit" form="travel-form" busy={saving}>
        {saving ? "Speichert …" : "Speichern"}
      </Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <LoadingState />
{:else if loadError}
  <Banner title="Die Anfahrtspauschale konnte nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
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
          fieldClass="kb-w-amount"
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
