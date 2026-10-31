<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { paymentAccountsList, privateMovementsCreate, uploadFile } from "#lib/api.ts";
  import type { PaymentAccount, MovementType, PrivateMovementInputDto } from "#lib/types.ts";
  import { MOVEMENT_TYPES, movementTypeLabel } from "#lib/labels.ts";
  import { date, euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Banner from "#lib/Banner.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";

  let accounts = $state<PaymentAccount[]>([]);
  let busy = $state(false);

  let movementDate = $state(todayBerlin());
  let movementType = $state<MovementType>("entnahme");
  // Betrag in Cent (MoneyInput, Lauf 2026.11 U1); keine negativen Werte (ADR 0041 Nr. 16).
  let amountCents = $state<number | null>(null);
  let accountId = $state<string>("");
  let description = $state("");
  let notes = $state("");
  let receiptFile = $state<File | null>(null);

  let errors = $state<{ description?: string; amount?: string; date?: string }>({});
  let amountInput: MoneyInput | undefined = $state();
  let descEl: HTMLInputElement | undefined = $state();
  let dateEl: HTMLInputElement | undefined = $state();

  onMount(async () => {
    try {
      accounts = await paymentAccountsList(false);
      const def = accounts.find((a) => a.isDefault === 1);
      if (def) accountId = def.id;
    } catch (e) {
      flash("Konten konnten nicht geladen werden: " + String(e), "error");
    }
  });

  async function validate(): Promise<boolean> {
    const e: typeof errors = {};
    if (!movementDate) e.date = "Bitte das Datum angeben.";
    const amountOk = amountInput?.validate() ?? true;
    if (amountOk && (amountCents ?? 0) <= 0) e.amount = "Der Betrag muss größer als 0 sein.";
    if (!description.trim()) e.description = "Bitte kurz beschreiben, wofür das Geld war.";
    errors = e;
    await tick();
    if (e.date) dateEl?.focus();
    else if (!amountOk || e.amount) amountInput?.focus();
    else if (e.description) descEl?.focus();
    return amountOk && Object.keys(e).length === 0;
  }

  async function save() {
    if (busy) return;
    if (!(await validate())) return;
    // U4 (V1-05 analog, design.md §5): Privatbewegungen sind ab dem Erfassen
    // festgeschrieben (GoBD) — Rückfrage mit den Eckwerten wie bei den Kosten.
    const ok = await confirmDialog({
      title: "Privatbewegung festschreiben?",
      body:
        "Danach lässt sich nichts mehr ändern. Einen Fehler korrigierst du mit einer " +
        "Gegenbuchung.",
      bullets: [
        `Datum: ${date(movementDate)}`,
        `Art: ${movementTypeLabel(movementType)}`,
        `Betrag: ${euro(amountCents ?? 0)}`,
        receiptFile ? `Beleg: ${receiptFile.name}` : "Beleg: keiner",
      ],
      confirmLabel: "Festschreiben",
      cancelLabel: "Abbrechen",
    });
    if (!ok) return;
    busy = true;
    try {
      // Plan W7: Beleg zuerst hochladen (Größe/Typ prüft der Server).
      const receiptUploadId = receiptFile ? (await uploadFile("beleg", receiptFile)).uploadId : null;
      const input: PrivateMovementInputDto = {
        movementDate,
        movementType,
        amountCents: amountCents ?? 0,
        accountId: accountId || null,
        description: description.trim(),
        notes: notes.trim() || null,
      };
      const row = await privateMovementsCreate({
        input,
        receiptUploadId,
      });
      // design.md §5: nach dem Speichern zur Detailseite.
      flash("Privatbewegung festgeschrieben: " + row.movementNumber);
      await goto(`/private-movements/${row.id}`);
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<!-- U4 (V2-11/V2-15): Beschriftung nach dem Ergebnis (sofort festgeschrieben, wie
     „Kosten festschreiben“), Objekt „Privatbewegung“ wie im Glossar. -->
<PageBar back="/private-movements" backLabel="Privatbewegungen" title="Neue Privatbewegung">
  {#snippet actions()}
    <a class="btn-secondary" href="/private-movements">Abbrechen</a>
    <button type="button" class="btn-primary" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>
      {busy ? "Schreibt fest …" : "Privatbewegung festschreiben"}
    </button>
    <HelpAnchor slug="privatbewegungen" />
  {/snippet}
</PageBar>
<Banner kind="warning">
  Das ist keine Einnahme oder Ausgabe und zählt nicht für die Steuer. Einmal
  festgeschrieben, lässt es sich nicht mehr ändern — korrigieren kannst du mit einer
  Gegenbuchung.
</Banner>

<section class="card">
  <form novalidate onsubmit={(e) => { e.preventDefault(); save(); }}>
    <p class="legend-req">Felder mit <span class="req" aria-hidden="true">*</span> sind Pflicht.</p>
    <div class="grid">
      <div class="field">
        <label for="pm-date">Datum<span class="req" aria-hidden="true">*</span></label>
        <input
          id="pm-date"
          class="kb-input kb-w-date"
          type="date"
          bind:this={dateEl}
          bind:value={movementDate}
          max={todayBerlin()}
          required
          aria-required="true"
          aria-invalid={errors.date ? "true" : undefined}
          aria-describedby={errors.date ? "pm-date-err" : undefined}
        />
        {#if errors.date}<p id="pm-date-err" class="msg err">{errors.date}</p>{/if}
      </div>
      <div class="field">
        <label for="pm-type">Art</label>
        <select id="pm-type" class="kb-input" bind:value={movementType}>
          {#each MOVEMENT_TYPES as t}<option value={t.value}>{t.label}</option>{/each}
        </select>
      </div>
      <MoneyInput
        bind:this={amountInput}
        bind:cents={amountCents}
        fieldClass="kb-w-amount"
        label="Betrag"
        required
        error={errors.amount}
        onchange={() => (errors.amount = undefined)}
      />
      <div class="field">
        <label for="pm-account">Konto</label>
        <select id="pm-account" class="kb-input" bind:value={accountId}>
          <option value="">— keines —</option>
          {#each accounts as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
        </select>
      </div>
      <div class="field span2">
        <label for="pm-desc">Beschreibung<span class="req" aria-hidden="true">*</span></label>
        <input
          id="pm-desc"
          class="kb-input"
          type="text"
          autocomplete="off"
          bind:this={descEl}
          bind:value={description}
          oninput={() => (errors.description = undefined)}
          required
          aria-required="true"
          aria-invalid={errors.description ? "true" : undefined}
          aria-describedby={errors.description ? "pm-desc-err" : "pm-desc-hint"}
        />
        {#if errors.description}
          <p id="pm-desc-err" class="msg err">{errors.description}</p>
        {:else}
          <p id="pm-desc-hint" class="msg hint">z. B. „Privatentnahme für Miete“</p>
        {/if}
      </div>
      <div class="field span2">
        <label for="pm-receipt">Beleg (PDF oder Bild, optional)</label>
        <input
          id="pm-receipt"
          type="file"
          accept="application/pdf,image/*"
          onchange={(e) => (receiptFile = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
        />
      </div>
      <div class="field span2">
        <label for="pm-notes">Notiz (optional)</label>
        <textarea id="pm-notes" class="kb-input" rows="3" bind:value={notes}></textarea>
      </div>
    </div>

    <p class="preview" aria-live="polite">
      {movementType === "entnahme" ? "Entnahme" : "Einlage"}: <strong class="kb-num">{euro(amountCents ?? 0)}</strong>
    </p>
    <!-- Hauptaktion am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end">
      <p id="pm-end-note" class="end-note">Danach nicht mehr änderbar — Korrektur nur per Gegenbuchung.</p>
      <button type="button" class="btn-primary" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy} aria-describedby="pm-end-note">{busy ? "Schreibt fest …" : "Privatbewegung festschreiben"}</button>
    </div>
  </form>
</section>

<style>
  /* .card / .kb-input: globale Definitionen aus tokens.css; Hinweise über Banner (V3-18). */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); align-items: start; }
  @media (max-width: 600px) { .grid { grid-template-columns: 1fr; } }
  .span2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .field > label:first-child { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .legend-req { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--sp-3); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .msg { font-size: var(--fs-sm); margin: 0; line-height: 1.4; }
  .msg.err { color: var(--status-danger-fg); font-weight: 600; }
  .msg.hint { color: var(--text-muted); }
  .grid :global(.money-field.with-label) { margin-bottom: 0; }
  .grid :global(.money-field label) { color: var(--text); }
  .preview { margin-top: var(--sp-4); font-size: var(--fs-md); }
  .end-note { flex-basis: 100%; margin: 0; font-size: var(--type-small); color: var(--text-muted); text-align: right; }
</style>
