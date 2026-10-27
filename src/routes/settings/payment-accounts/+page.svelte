<script lang="ts">
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import {
    paymentAccountsList,
    paymentAccountsCreate,
    paymentAccountsUpdate,
    paymentAccountsSetActive,
  } from "#lib/api.ts";
  import type { PaymentAccount, PaymentAccountInput, PaymentAccountType } from "#lib/types.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import ActionMenu from "#lib/ActionMenu.svelte";
  import Table from "#lib/Table.svelte";
  import { iban as fmtIban } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";

  // U4 (V2-06/V2-10/V2-11/V2-12/V2-19): Titel wie im Einstellungs-Hub („Konten“), Liste im
  // Table-Baustein, „Abbrechen“ vor der Hauptaktion, Knöpfe während einer Aktion
  // `ariaDisabled` (Fokus bleibt), Deaktivieren ohne Rückfrage mit „Rückgängig“ im Toast.

  let accounts = $state<PaymentAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let showInactive = $state(false);

  const TYPES: { value: PaymentAccountType; label: string }[] = [
    { value: "bank", label: "Bankkonto" },
    { value: "cash", label: "Bargeld-Kasse" },
    { value: "paypal", label: "PayPal" },
    { value: "stripe", label: "Stripe" },
    { value: "other", label: "Sonstiges" },
  ];

  function typeLabel(t: string): string {
    return TYPES.find((x) => x.value === t)?.label ?? t;
  }

  // Neu-Formular
  let form = $state<PaymentAccountInput>({
    label: "",
    accountType: "bank",
    iban: null,
    bic: null,
    isDefault: false,
    showOnInvoice: false,
    details: null,
  });

  // Inline-Edit
  let editId = $state<string | null>(null);
  let editForm = $state<PaymentAccountInput>({
    label: "",
    accountType: "bank",
    iban: null,
    bic: null,
    isDefault: false,
    showOnInvoice: false,
    details: null,
  });

  async function load() {
    loading = true;
    error = null;
    try {
      // Reines Lesen: Standardkonten legt der Server einmalig bei der
      // Einrichtung bzw. beim ersten Entsperren an (Lauf F, D-14).
      accounts = await paymentAccountsList(showInactive);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function reload() {
    try {
      accounts = await paymentAccountsList(showInactive);
    } catch (e) {
      flash(`Konten neu laden fehlgeschlagen: ${String(e)}`, "error");
    }
  }

  let labelError = $state("");
  let labelInput: HTMLInputElement | undefined = $state();
  let editLabelError = $state("");

  async function create() {
    if (busy) return;
    labelError = "";
    if (!form.label.trim()) {
      labelError = "Bitte gib eine Bezeichnung ein, z. B. „Geschäftskonto“.";
      labelInput?.focus();
      return;
    }
    busy = true;
    try {
      await paymentAccountsCreate({
        ...form,
        iban: form.iban?.trim() || null,
        bic: form.bic?.trim() || null,
        details: form.details?.trim() || null,
      });
      form = {
        label: "",
        accountType: "bank",
        iban: null,
        bic: null,
        isDefault: false,
        showOnInvoice: false,
        details: null,
      };
      flash("Konto angelegt.");
      await reload();
    } catch (e) {
      flash("Konto anlegen fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  function startEdit(a: PaymentAccount) {
    editId = a.id;
    editLabelError = "";
    editForm = {
      label: a.label,
      accountType: a.accountType as PaymentAccountType,
      iban: a.iban,
      bic: a.bic,
      isDefault: a.isDefault === 1,
      showOnInvoice: a.showOnInvoice === 1,
      details: a.details,
    };
  }

  async function saveEdit() {
    if (!editId || busy) return;
    editLabelError = "";
    if (!editForm.label.trim()) {
      editLabelError = "Bitte gib eine Bezeichnung ein.";
      document.getElementById(`pa-edit-label-${editId}`)?.focus();
      return;
    }
    busy = true;
    try {
      await paymentAccountsUpdate(editId, {
        ...editForm,
        iban: editForm.iban?.trim() || null,
        bic: editForm.bic?.trim() || null,
        details: editForm.details?.trim() || null,
      });
      editId = null;
      flash("Konto gespeichert.");
      await reload();
    } catch (e) {
      flash("Konto speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  /** Aktivieren/Deaktivieren ist umkehrbar: keine Rückfrage, „Rückgängig“ im Toast (design.md §5). */
  async function setActive(a: PaymentAccount, active: boolean, undo = false) {
    if (busy) return;
    busy = true;
    try {
      await paymentAccountsSetActive(a.id, active);
      if (undo) {
        flash(active ? `Konto „${a.label}“ wieder aktiv.` : `Konto „${a.label}“ wieder deaktiviert.`);
      } else {
        flash(active ? `Konto „${a.label}“ aktiviert.` : `Konto „${a.label}“ deaktiviert.`, "ok", {
          action: { label: "Rückgängig", run: () => void setActive(a, !active, true) },
        });
      }
      await reload();
    } catch (e) {
      flash(`Konto ${active ? "aktivieren" : "deaktivieren"} fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Konten" />
<p class="lead">
  Deine Konten für Zahlungen — Bankkonto, Bargeld, PayPal und so weiter. Du wählst
  sie bei Kosten und Privatbewegungen aus. Konten werden nicht gelöscht, sondern
  deaktiviert. Genau ein Konto ist der <strong>Standard</strong>. Konten mit
  <strong>„Auf Beleg anzeigen“</strong> erscheinen auf Angeboten und Rechnungen
  (mehrere möglich) — Bankkonten zusätzlich als Überweisungsweg in der XRechnung.
</p>

<section class="card" aria-labelledby="pa-new-h">
  <h2 id="pa-new-h">Neues Konto</h2>
  <form class="grid" novalidate onsubmit={(e) => { e.preventDefault(); create(); }}>
    <div class="f">
      <label for="pa-label">Bezeichnung<span class="req" aria-hidden="true">*</span> <span class="kb-visually-hidden">(Pflicht)</span></label>
      <input
        id="pa-label"
        bind:this={labelInput}
        type="text"
        autocomplete="off"
        bind:value={form.label}
        required
        aria-required="true"
        aria-invalid={labelError ? "true" : undefined}
        aria-describedby={labelError ? "pa-label-err" : undefined}
        placeholder="z. B. Geschäftskonto Sparkasse"
      />
      {#if labelError}<span id="pa-label-err" class="err">{labelError}</span>{/if}
    </div>
    <div class="f">
      <label for="pa-type">Typ</label>
      <select id="pa-type" bind:value={form.accountType}>
        {#each TYPES as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
      </select>
    </div>
    <div class="f">
      <label for="pa-iban">IBAN (optional)</label>
      <input
        id="pa-iban"
        type="text"
        bind:value={form.iban}
        autocomplete="off"
        inputmode="text"
        autocapitalize="characters"
        spellcheck="false"
        aria-describedby="pa-iban-hint"
      />
      <span id="pa-iban-hint" class="hint">z. B. DE89 3704 0044 0532 0130 00 – Leerzeichen sind erlaubt</span>
    </div>
    <div class="f">
      <label for="pa-bic">BIC (optional)</label>
      <input
        id="pa-bic"
        type="text"
        bind:value={form.bic}
        autocomplete="off"
        inputmode="text"
        autocapitalize="characters"
        spellcheck="false"
        aria-describedby="pa-bic-hint"
      />
      <span id="pa-bic-hint" class="hint">8 oder 11 Zeichen, z. B. COBADEFFXXX</span>
    </div>
    <div class="f wide">
      <label for="pa-details">Zahlungs-Details (optional)</label>
      <input id="pa-details" type="text" autocomplete="off" bind:value={form.details} aria-describedby="pa-details-hint" />
      <span id="pa-details-hint" class="hint">z. B. paypal.me/deinname</span>
    </div>
    <Toggle bind:checked={form.isDefault} label="Als Standard-Konto" />
    <Toggle bind:checked={form.showOnInvoice} label="Auf Beleg anzeigen" />
    <div class="submit">
      <Button type="submit" ariaDisabled={busy}>{busy ? "Legt an …" : "Konto anlegen"}</Button>
    </div>
  </form>

</section>

<label class="chk show-inactive">
  <input type="checkbox" bind:checked={showInactive} onchange={reload} /> Inaktive Konten anzeigen
</label>

{#if loading}
  <LoadingState />
{:else if error}
  <Banner title="Die Konten konnten nicht geladen werden.">{error}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if accounts.length === 0}
  <p class="muted">Noch keine Konten. Lege oben unter „Neues Konto“ dein erstes an.</p>
{:else}
  <!-- U4 (V2-10, design.md §4.5): Liste im Table-Baustein (weiße Karte), nicht frei auf dem Hintergrund. -->
  <Table cards cardGrid>
    <caption class="kb-visually-hidden">Deine Konten</caption>
    <thead>
      <tr><th>Bezeichnung</th><th class="kb-col-opt2">Typ</th><th>IBAN / BIC</th><th class="kb-col-opt1">Details</th><th>Auf Beleg</th><th>Standard</th><th>Status</th><th><span class="kb-visually-hidden">Aktionen</span></th></tr>
    </thead>
    <tbody>
      {#each accounts as a (a.id)}
        {#if editId === a.id}
          <!-- U3: Bearbeiten als Formularzeile über alle Spalten mit sichtbaren Labels
               (vorher je Spalte ein schmales Feld: Bezeichnung abgeschnitten, IBAN und BIC
               ohne sichtbare Bezeichnung untereinander). -->
          <tr class="edit-row">
            <td colspan="8" class="cards-full edit-cell">
              <div class="grid edit-grid" role="group" aria-label="Konto {a.label} bearbeiten">
                <div class="f">
                  <label for="pa-edit-label-{a.id}">Bezeichnung<span class="req" aria-hidden="true">*</span></label>
                  <input
                    id="pa-edit-label-{a.id}"
                    type="text"
                    autocomplete="off"
                    bind:value={editForm.label}
                    aria-label="Bezeichnung, Konto {a.label}"
                    aria-required="true"
                    aria-invalid={editLabelError ? "true" : undefined}
                    aria-describedby={editLabelError ? `pa-edit-err-${a.id}` : undefined}
                  />
                  {#if editLabelError}<span id="pa-edit-err-{a.id}" class="err">{editLabelError}</span>{/if}
                </div>
                <div class="f">
                  <label for="pa-edit-type-{a.id}">Typ</label>
                  <select id="pa-edit-type-{a.id}" bind:value={editForm.accountType} aria-label="Typ, Konto {a.label}">
                    {#each TYPES as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
                  </select>
                </div>
                <div class="f">
                  <label for="pa-edit-iban-{a.id}">IBAN</label>
                  <input
                    id="pa-edit-iban-{a.id}"
                    type="text"
                    bind:value={editForm.iban}
                    aria-label="IBAN, Konto {a.label}"
                    autocomplete="off"
                    inputmode="text"
                    autocapitalize="characters"
                    spellcheck="false"
                  />
                </div>
                <div class="f">
                  <label for="pa-edit-bic-{a.id}">BIC</label>
                  <input
                    id="pa-edit-bic-{a.id}"
                    class="bic"
                    type="text"
                    bind:value={editForm.bic}
                    aria-label="BIC, Konto {a.label}"
                    autocomplete="off"
                    inputmode="text"
                    autocapitalize="characters"
                    spellcheck="false"
                  />
                </div>
                <div class="f wide">
                  <label for="pa-edit-details-{a.id}">Zahlungs-Details</label>
                  <input id="pa-edit-details-{a.id}" type="text" autocomplete="off" bind:value={editForm.details} aria-label="Zahlungs-Details, Konto {a.label}" />
                </div>
                <div class="f edit-checks">
                  <label class="chk cell-chk">
                    <input type="checkbox" bind:checked={editForm.showOnInvoice} />
                    Auf Beleg anzeigen<span class="kb-visually-hidden">, Konto {a.label}</span>
                  </label>
                  <label class="chk cell-chk">
                    <input type="checkbox" bind:checked={editForm.isDefault} />
                    Standard-Konto<span class="kb-visually-hidden">, Konto {a.label}</span>
                  </label>
                </div>
                <p class="muted edit-status">Status: {a.active === 1 ? "aktiv" : "inaktiv"}</p>
                <div class="kb-actions kb-start edit-actions">
                  <Button variant="secondary" onclick={() => (editId = null)} ariaLabel="Abbrechen, Konto {a.label}">Abbrechen</Button>
                  <Button onclick={saveEdit} ariaDisabled={busy} ariaLabel="Änderungen speichern, Konto {a.label}">Änderungen speichern</Button>
                </div>
              </div>
            </td>
          </tr>
        {:else}
          <tr class:inactive={a.active !== 1}>
            <td data-label="Bezeichnung"><strong>{a.label}</strong></td>
            <td class="kb-col-opt2" data-label="Typ">{typeLabel(a.accountType)}</td>
            <td class="muted" data-label="IBAN / BIC">
              {#if a.iban || a.bic}
                {#if a.iban}<span class="mono">{fmtIban(a.iban)}</span>{/if}
                {#if a.bic}<span class="mono bic-show">{a.bic}</span>{/if}
              {:else}—{/if}
            </td>
            <td class="muted kb-col-opt1" data-label="Details">{a.details ?? "—"}</td>
            <td data-label="Auf Beleg">{a.showOnInvoice === 1 ? "ja" : "nein"}</td>
            <td data-label="Standard">{#if a.isDefault === 1}<Badge kind="tag">Standard</Badge>{:else}—{/if}</td>
            <td data-label="Status">{#if a.active === 1}<Badge tone="success">Aktiv</Badge>{:else}<Badge>Inaktiv</Badge>{/if}</td>
            <td>
              <!-- U4 (U3-036, design.md §4.5): eine sichtbare Aktion + Menü „Mehr“. -->
              <div class="kb-actions">
                <Button variant="secondary" onclick={() => startEdit(a)} ariaDisabled={busy} ariaLabel="Bearbeiten, Konto {a.label}">Bearbeiten</Button>
                <ActionMenu
                  variant="ghost"
                  ariaLabel={`Mehr zu Konto ${a.label}`}
                  items={[{ label: a.active === 1 ? "Deaktivieren" : "Aktivieren", onSelect: () => setActive(a, a.active !== 1), disabled: busy }]}
                />
              </div>
            </td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  /* .card / .lead — globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: 2fr 1fr 1.5fr 1fr; gap: 0.75rem; align-items: start; }
  .grid .wide { grid-column: span 2; }
  .f { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .f > label { font-weight: 600; }
  .grid .submit { display: flex; align-items: flex-end; }
  label { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text); gap: 0.25rem; min-width: 0; }
  label.chk { flex-direction: row; align-items: center; gap: 0.6rem; font-weight: 400; }
  .show-inactive { margin: 0.5rem 0; min-height: var(--target); width: fit-content; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .hint { font-weight: 400; color: var(--text-muted); font-size: var(--fs-xs); }
  .err { font-weight: 600; color: var(--status-danger-fg); font-size: var(--fs-sm); display: block; }
  input:not([type="checkbox"]), select { padding: 0.4rem 0.5rem; border: 1px solid var(--line-control); border-radius: var(--r-sm); font: inherit; font-weight: 400; min-height: var(--target); background: var(--surface-card); }
  th, td { text-align: left; font-size: var(--fs-sm); }
  tr.inactive td { color: var(--text-muted); }
  .edit-row input:not([type="checkbox"]), .edit-row select { width: 100%; }
  .edit-cell { text-align: left; }
  .edit-grid { margin: 0.25rem 0; }
  .edit-checks { flex-direction: row; flex-wrap: wrap; align-items: center; gap: 0.25rem 1rem; align-self: end; }
  .edit-checks .cell-chk { justify-content: flex-start; }
  .edit-status { margin: 0; align-self: end; min-height: var(--target); display: flex; align-items: center; }
  .edit-actions { grid-column: 1 / -1; }
  .cell-chk { display: inline-flex; flex-direction: row; align-items: center; justify-content: center; min-width: var(--target); min-height: var(--target); }
  .mono { font-family: var(--font-mono); white-space: nowrap; }
  .bic-show { display: block; font-size: var(--fs-xs); }
  .muted { color: var(--text-muted); }
  @media (max-width: 900px) {
    .grid { grid-template-columns: 1fr 1fr; }
  }
  @media (max-width: 600px) {
    .grid { grid-template-columns: 1fr; }
    .grid .wide { grid-column: auto; }
  }
  @media screen and (max-width: 820px) {
    td { overflow-wrap: anywhere; }
    .mono { white-space: normal; overflow-wrap: anywhere; }
  }
</style>
