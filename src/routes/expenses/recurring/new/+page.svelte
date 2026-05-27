<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import PageBar from "$lib/PageBar.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import {
    contactsList,
    recurringCreate,
    recurringUpdate,
    recurringGet,
  } from "$lib/api";
  import type { Contact, ExpenseCategory, Frequency, RecurringInputDto } from "$lib/types";
  import { EXPENSE_CATEGORIES, FREQUENCIES } from "$lib/labels";
  import { euro } from "$lib/format";
  import { flash } from "$lib/toast.svelte";

  // Edit-Modus, wenn ?id=… gesetzt ist.
  let editId = $state<string | null>(null);
  let loaded = $state(false);

  let vendors = $state<Contact[]>([]);
  let busy = $state(false);

  // Formularfelder
  let label = $state("");
  let vendorContactId = $state<string>("");
  let frequency = $state<Frequency>("monthly");
  let dayOfPeriod = $state<number>(1);
  let nextDueDate = $state(new Date().toISOString().slice(0, 10));
  let expectedEuros = $state<number>(0);
  let category = $state<ExpenseCategory>("software");
  let descriptionTemplate = $state("");
  // "auto" = Klein.Buch bucht selbstständig; "manual" = nur Erinnerung.
  let autoMode = $state<"auto" | "manual">("manual");
  let reverseCharge13bDefault = $state(false);

  let expectedCents = $derived(Math.round(expectedEuros * 100));

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");

      const id = $page.url.searchParams.get("id");
      if (id) {
        editId = id;
        const s = await recurringGet(id);
        if (!s) {
          flash("Abo nicht gefunden.", "error");
        } else {
          label = s.label;
          vendorContactId = s.vendorContactId ?? "";
          frequency = s.frequency as Frequency;
          dayOfPeriod = s.dayOfPeriod;
          nextDueDate = s.nextDueDate.slice(0, 10);
          expectedEuros = s.expectedAmountCents / 100;
          category = s.category as ExpenseCategory;
          descriptionTemplate = s.descriptionTemplate;
          autoMode = s.autoCreateExpense === 1 ? "auto" : "manual";
          reverseCharge13bDefault = s.reverseCharge13bDefault === 1;
        }
      }
    } catch (e) {
      flash("Laden fehlgeschlagen: " + String(e), "error");
    } finally {
      loaded = true;
    }
  });

  function onVendorSelect() {
    const c = vendors.find((v) => v.id === vendorContactId);
    if (c && !label.trim()) label = c.name;
  }

  async function save() {
    if (!label.trim()) {
      flash("Bezeichnung ist erforderlich.", "error");
      return;
    }
    if (!descriptionTemplate.trim()) {
      flash("Beschreibungs-Vorlage ist erforderlich.", "error");
      return;
    }
    if (dayOfPeriod < 1 || dayOfPeriod > 31) {
      flash("Stichtag muss zwischen 1 und 31 liegen.", "error");
      return;
    }
    if (expectedCents <= 0) {
      flash("Erwarteter Betrag muss größer als 0 sein.", "error");
      return;
    }
    busy = true;
    try {
      const input: RecurringInputDto = {
        label: label.trim(),
        vendorContactId: vendorContactId || null,
        frequency,
        dayOfPeriod,
        nextDueDate,
        expectedAmountCents: expectedCents,
        category,
        descriptionTemplate: descriptionTemplate.trim(),
        autoCreateExpense: autoMode === "auto",
        reverseCharge13bDefault,
      };
      if (editId) {
        await recurringUpdate(editId, input);
        flash("Abo gespeichert.");
      } else {
        await recurringCreate(input);
        flash("Abo angelegt.");
      }
      await goto("/expenses/recurring");
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/expenses/recurring" backLabel="Abos" title={editId ? "Abo bearbeiten" : "Neues Abo"}>
  {#snippet actions()}
    {#if loaded}
      <a class="btn-secondary" href="/expenses/recurring">Abbrechen</a>
      <button class="btn-primary" onclick={save} disabled={busy}>
        {busy ? "Speichere …" : editId ? "Speichern" : "Abo anlegen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if !loaded}
  <p class="muted">Lade …</p>
{:else}
  <section class="card">
    <div class="grid">
      <label class="span2">
        Bezeichnung
        <input type="text" bind:value={label} placeholder="z. B. Microsoft 365 Business" />
      </label>
      <label>
        Lieferant (Kontakt)
        <select bind:value={vendorContactId} onchange={onVendorSelect}>
          <option value="">— ohne Kontakt —</option>
          {#each vendors as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
        </select>
      </label>
      <label>
        Kategorie (EÜR)
        <select bind:value={category}>
          {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
        </select>
      </label>
      <label>
        Frequenz
        <select bind:value={frequency}>
          {#each FREQUENCIES as f}<option value={f.value}>{f.label}</option>{/each}
        </select>
      </label>
      <label>
        Stichtag (Tag im Monat, 1–31)
        <input type="number" min="1" max="31" bind:value={dayOfPeriod} />
      </label>
      <label>
        Nächste Fälligkeit
        <input type="date" bind:value={nextDueDate} />
      </label>
      <label>
        Erwarteter Betrag (Brutto, €)
        <input type="number" step="0.01" min="0" bind:value={expectedEuros} />
      </label>
      <label class="span2">
        Beschreibungs-Vorlage
        <input type="text" bind:value={descriptionTemplate} placeholder="Text der erzeugten Kosten-Position" />
      </label>
      <fieldset class="span2 modepick">
        <legend>Was soll am Stichtag passieren?</legend>
        <label class="radio" class:sel={autoMode === "auto"}>
          <input type="radio" name="autoMode" value="auto" bind:group={autoMode} />
          <span>
            <strong>Automatisch als Kosten buchen.</strong>
            Klein.Buch legt die Kostenposition am Stichtag von selbst an — auch
            rückwirkend für jede verpasste Periode, falls die App länger zu war.
            Die Position ist zunächst <em>„noch nicht bezahlt"</em>; du bestätigst
            später nur die echte Abbuchung.
          </span>
        </label>
        <label class="radio" class:sel={autoMode === "manual"}>
          <input type="radio" name="autoMode" value="manual" bind:group={autoMode} />
          <span>
            <strong>Nur an die Fälligkeit erinnern.</strong>
            Das Abo erscheint am Stichtag als <em>„fällig"</em> in der Liste —
            gebucht wird nichts automatisch. Die Kostenposition legst du selbst
            per Knopf <em>„Jetzt erfassen"</em> an.
          </span>
        </label>
      </fieldset>
      <div class="span2">
        <Toggle
          bind:checked={reverseCharge13bDefault}
          label="§13b Reverse-Charge für erzeugte Kosten vorgeben (Hinweis-Flag)"
        />
      </div>
    </div>

    <p class="hint">
      Erzeugte Kosten setzen Netto = Brutto, USt = 0 (das Abo kennt keinen
      USt-Split). Weicht der echte Beleg ab, korrigierst du per Storno +
      Neuerfassung. Vorschau Betrag: <strong>{euro(expectedCents)}</strong>.
    </p>

  </section>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  input, select { padding: 0.45rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  .modepick { border: 1px solid #e5e7eb; border-radius: 6px; padding: 0.75rem 1rem 1rem; margin: 0; }
  .modepick legend { font-size: 0.85rem; color: #4b5563; font-weight: 600; padding: 0 0.35rem; }
  label.radio { flex-direction: row; align-items: flex-start; gap: 0.6rem; padding: 0.6rem; border: 1px solid #e5e7eb; border-radius: 5px; cursor: pointer; margin-top: 0.5rem; }
  label.radio span { font-size: 0.85rem; color: #4b5563; line-height: 1.35; }
  label.radio strong { color: #1f2937; }
  label.radio em { font-style: normal; color: #3730a3; }
  label.radio input { width: auto; margin-top: 0.15rem; padding: 0; }
  label.radio.sel { border-color: #2563eb; background: #eff6ff; }
  .hint { font-size: 0.82rem; color: #3730a3; background: #eef2ff; border: 1px solid #c7d2fe; padding: 0.5rem 0.75rem; border-radius: 4px; }
  /* Lokale btn-Definitionen entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
</style>
