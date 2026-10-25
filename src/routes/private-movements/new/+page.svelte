<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { paymentAccountsList, privateMovementsCreate, uploadFile } from "#lib/api.ts";
  import type { PaymentAccount, MovementType, PrivateMovementInputDto } from "#lib/types.ts";
  import { MOVEMENT_TYPES } from "#lib/labels.ts";
  import { euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import PageBar from "#lib/PageBar.svelte";

  let accounts = $state<PaymentAccount[]>([]);
  let busy = $state(false);

  let movementDate = $state(todayBerlin());
  let movementType = $state<MovementType>("entnahme");
  let amountEuros = $state<number>(0);
  let accountId = $state<string>("");
  let description = $state("");
  let notes = $state("");
  let receiptFile = $state<File | null>(null);

  let amountCents = $derived(Math.round(amountEuros * 100));

  onMount(async () => {
    try {
      accounts = await paymentAccountsList(false);
      const def = accounts.find((a) => a.isDefault === 1);
      if (def) accountId = def.id;
    } catch (e) {
      flash("Konten konnten nicht geladen werden: " + String(e), "error");
    }
  });

  async function save() {
    if (!description.trim()) {
      flash("Beschreibung ist erforderlich.", "error");
      return;
    }
    if (amountCents <= 0) {
      flash("Betrag muss größer als 0 sein.", "error");
      return;
    }
    busy = true;
    try {
      // Plan W7: Beleg zuerst hochladen (Größe/Typ prüft der Server).
      const receiptUploadId = receiptFile ? (await uploadFile("beleg", receiptFile)).uploadId : null;
      const input: PrivateMovementInputDto = {
        movementDate,
        movementType,
        amountCents,
        accountId: accountId || null,
        description: description.trim(),
        notes: notes.trim() || null,
      };
      const row = await privateMovementsCreate({
        input,
        receiptUploadId,
      });
      flash("Erfasst: " + row.movementNumber);
      await goto("/private-movements");
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/private-movements" backLabel="Privat-Geld" title="Neue Privat-Buchung">
  {#snippet actions()}
    <a class="btn-secondary" href="/private-movements">Abbrechen</a>
    <button class="btn-primary" onclick={save} disabled={busy}>
      {busy ? "Speichere …" : "Speichern"}
    </button>
  {/snippet}
</PageBar>
<p class="caveat">
  Das ist keine Einnahme oder Ausgabe und zählt nicht für die Steuer. Einmal
  gespeichert, lässt es sich nicht mehr ändern — korrigieren kannst du mit einer
  Gegenbuchung.
</p>

<section class="card">
  <div class="grid">
    <label>
      Datum
      <input type="date" bind:value={movementDate} max={todayBerlin()} />
    </label>
    <label>
      Art
      <select bind:value={movementType}>
        {#each MOVEMENT_TYPES as t}<option value={t.value}>{t.label}</option>{/each}
      </select>
    </label>
    <label>
      Betrag (€)
      <input type="number" step="0.01" min="0" bind:value={amountEuros} />
    </label>
    <label>
      Konto
      <select bind:value={accountId}>
        <option value="">— keines —</option>
        {#each accounts as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
      </select>
    </label>
    <label class="span2">
      Beschreibung
      <input type="text" bind:value={description} placeholder="z. B. Privatentnahme für Miete" />
    </label>
    <label class="span2">
      Beleg (optional)
      <input
        type="file"
        accept="application/pdf,image/*"
        onchange={(e) => (receiptFile = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
      />
    </label>
    <label class="span2">
      Notiz (optional)
      <textarea rows="2" bind:value={notes}></textarea>
    </label>
  </div>

  <p class="preview">
    {movementType === "entnahme" ? "Entnahme" : "Einlage"}: <strong>{euro(amountCents)}</strong>
  </p>

</section>

<style>
  /* .caveat / .card entfernt — globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  @media (max-width: 600px) { .grid { grid-template-columns: 1fr; } }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  input, select, textarea { padding: 0.45rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  .preview { margin-top: 1rem; font-size: 0.95rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
