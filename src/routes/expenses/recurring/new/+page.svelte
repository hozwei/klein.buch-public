<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import PageBar from "#lib/PageBar.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import Banner from "#lib/Banner.svelte";
  import ContactSelect from "#lib/ContactSelect.svelte";
  import {
    contactsList,
    recurringCreate,
    recurringUpdate,
    recurringGet,
  } from "#lib/api.ts";
  import type { Contact, ExpenseCategory, Frequency, RecurringInputDto } from "#lib/types.ts";
  import { EXPENSE_CATEGORIES, FREQUENCIES } from "#lib/labels.ts";
  import { euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";

  // Edit-Modus, wenn ?id=… gesetzt ist.
  let editId = $state<string | null>(null);
  let loaded = $state(false);
  let notFound = $state(false);

  let vendors = $state<Contact[]>([]);
  let busy = $state(false);

  // Formularfelder
  let label = $state("");
  let vendorContactId = $state<string>("");
  let frequency = $state<Frequency>("monthly");
  let dayOfPeriod = $state<number | null>(1);
  let nextDueDate = $state(todayBerlin());
  // Betrag in Cent (MoneyInput, Lauf 2026.11 U1).
  let expectedCents = $state<number | null>(null);
  let category = $state<ExpenseCategory>("software");
  let descriptionTemplate = $state("");
  // "auto" = Klein.Buch bucht selbstständig; "manual" = nur Erinnerung.
  let autoMode = $state<"auto" | "manual">("manual");
  let reverseCharge13bDefault = $state(false);

  let errors = $state<{ label?: string; description?: string; day?: string; amount?: string; nextDue?: string }>({});
  let labelEl: HTMLInputElement | undefined = $state();
  let descEl: HTMLInputElement | undefined = $state();
  let nextDueEl: HTMLInputElement | undefined = $state();
  let dayInput: QuantityInput | undefined = $state();
  let amountInput: MoneyInput | undefined = $state();

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");

      const id = page.url.searchParams.get("id");
      if (id) {
        editId = id;
        const s = await recurringGet(id);
        if (!s) {
          notFound = true;
        } else {
          label = s.label;
          vendorContactId = s.vendorContactId ?? "";
          frequency = s.frequency as Frequency;
          dayOfPeriod = s.dayOfPeriod;
          nextDueDate = s.nextDueDate.slice(0, 10);
          expectedCents = s.expectedAmountCents;
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

  function onVendorSelect(id: string) {
    const c = vendors.find((v) => v.id === id);
    if (c && !label.trim()) {
      label = c.name;
      errors.label = undefined;
    }
  }

  async function validate(): Promise<boolean> {
    const e: typeof errors = {};
    if (!label.trim()) e.label = "Bitte eine Bezeichnung angeben.";
    const dayOk = dayInput?.validate() ?? true;
    if (dayOk && (dayOfPeriod === null || dayOfPeriod < 1 || dayOfPeriod > 31)) {
      e.day = "Bitte einen Tag zwischen 1 und 31 angeben.";
    }
    if (!nextDueDate) e.nextDue = "Bitte das Datum der nächsten Fälligkeit angeben.";
    const amountOk = amountInput?.validate() ?? true;
    if (amountOk && (expectedCents ?? 0) <= 0) e.amount = "Der Betrag muss größer als 0 sein.";
    if (!descriptionTemplate.trim()) e.description = "Bitte den Text der erzeugten Kosten angeben.";
    errors = e;
    await tick();
    if (e.label) labelEl?.focus();
    else if (!dayOk || e.day) dayInput?.focus();
    else if (e.nextDue) nextDueEl?.focus();
    else if (!amountOk || e.amount) amountInput?.focus();
    else if (e.description) descEl?.focus();
    return dayOk && amountOk && Object.keys(e).length === 0;
  }

  async function save() {
    if (busy) return;
    if (!(await validate())) return;
    busy = true;
    try {
      const input: RecurringInputDto = {
        label: label.trim(),
        vendorContactId: vendorContactId || null,
        frequency,
        dayOfPeriod: dayOfPeriod ?? 1,
        nextDueDate,
        expectedAmountCents: expectedCents ?? 0,
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

<!-- U4 (V2-19/V2-11): Zurück wie der Listentitel; beim Bearbeiten „Änderungen speichern“;
     während des Speicherns aria-disabled statt disabled (Fokus bleibt). -->
<PageBar back="/expenses/recurring" backLabel="Kosten-Abos" title={editId ? "Abo bearbeiten" : "Neues Abo"}>
  {#snippet actions()}
    {#if loaded && !notFound}
      <a class="btn-secondary" href="/expenses/recurring">Abbrechen</a>
      <button type="button" class="btn-primary" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>
        {busy ? "Speichert …" : editId ? "Änderungen speichern" : "Abo anlegen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if !loaded}
  <p class="muted" role="status">Wird geladen …</p>
{:else if notFound}
  <Banner>
    Abo nicht gefunden. <a class="kb-link kb-target" href="/expenses/recurring">Zur Abo-Liste</a>
  </Banner>
{:else}
  <section class="card">
    <form novalidate onsubmit={(e) => { e.preventDefault(); save(); }}>
      <p class="legend-req">Felder mit <span class="req" aria-hidden="true">*</span> sind Pflicht.</p>
      <div class="grid">
        <div class="field span2">
          <label for="rec-label">Bezeichnung<span class="req" aria-hidden="true">*</span></label>
          <input
            id="rec-label"
            class="kb-input"
            type="text"
            autocomplete="off"
            bind:this={labelEl}
            bind:value={label}
            oninput={() => (errors.label = undefined)}
            required
            aria-required="true"
            aria-invalid={errors.label ? "true" : undefined}
            aria-describedby={errors.label ? "rec-label-err" : "rec-label-hint"}
          />
          {#if errors.label}
            <p id="rec-label-err" class="msg err">{errors.label}</p>
          {:else}
            <p id="rec-label-hint" class="msg hint">z. B. „Microsoft 365 Business“</p>
          {/if}
        </div>
        <!-- U4 (Warteliste 15): durchsuchbar ab 8 Lieferanten, deutsche Sortierung. -->
        <ContactSelect
          bind:value={vendorContactId}
          contacts={vendors}
          label="Lieferant (Kontakt)"
          id="rec-vendor"
          placeholder="— ohne Kontakt —"
          onchange={onVendorSelect}
        />
        <div class="field">
          <label for="rec-category">Kategorie (EÜR)</label>
          <select id="rec-category" class="kb-input" bind:value={category}>
            {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
          </select>
        </div>
        <div class="field">
          <label for="rec-frequency">Frequenz</label>
          <select id="rec-frequency" class="kb-input" bind:value={frequency}>
            {#each FREQUENCIES as f}<option value={f.value}>{f.label}</option>{/each}
          </select>
        </div>
        <QuantityInput
          bind:this={dayInput}
          bind:value={dayOfPeriod}
          fieldClass="kb-w-short"
          integer
          grouping={false}
          max={31}
          noun="Tag"
          label="Stichtag (Tag im Monat)"
          hint="1 bis 31; in kürzeren Monaten gilt der letzte Tag."
          required
          error={errors.day}
          onchange={() => (errors.day = undefined)}
        />
        <div class="field">
          <label for="rec-next">Nächste Fälligkeit<span class="req" aria-hidden="true">*</span></label>
          <input
            id="rec-next"
            class="kb-input kb-w-date"
            type="date"
            bind:this={nextDueEl}
            bind:value={nextDueDate}
            required
            aria-required="true"
            aria-invalid={errors.nextDue ? "true" : undefined}
            aria-describedby={errors.nextDue ? "rec-next-err" : undefined}
          />
          {#if errors.nextDue}<p id="rec-next-err" class="msg err">{errors.nextDue}</p>{/if}
        </div>
        <MoneyInput
          bind:this={amountInput}
          bind:cents={expectedCents}
          fieldClass="kb-w-amount"
          label="Erwarteter Betrag (Brutto)"
          required
          error={errors.amount}
          onchange={() => (errors.amount = undefined)}
        />
        <div class="field span2">
          <label for="rec-desc">Beschreibungs-Vorlage<span class="req" aria-hidden="true">*</span></label>
          <input
            id="rec-desc"
            class="kb-input"
            type="text"
            autocomplete="off"
            bind:this={descEl}
            bind:value={descriptionTemplate}
            oninput={() => (errors.description = undefined)}
            required
            aria-required="true"
            aria-invalid={errors.description ? "true" : undefined}
            aria-describedby={errors.description ? "rec-desc-err" : "rec-desc-hint"}
          />
          {#if errors.description}
            <p id="rec-desc-err" class="msg err">{errors.description}</p>
          {:else}
            <p id="rec-desc-hint" class="msg hint">Text der erzeugten Kostenposition.</p>
          {/if}
        </div>
        <fieldset class="span2 modepick">
          <legend>Was soll am Stichtag passieren?</legend>
          <label class="radio" class:sel={autoMode === "auto"}>
            <input type="radio" name="autoMode" value="auto" bind:group={autoMode} />
            <span>
              <strong>Automatisch als Kosten buchen.</strong>
              Klein.Buch legt die Kostenposition am Stichtag von selbst an. Waren
              Termine offen (z. B. weil die Instanz gesperrt war), holt es bis zu drei
              selbst nach; bei mehr fragt es vorher nach (Hinweis).
              Die Position ist zunächst <em>„noch nicht bezahlt“</em>; du bestätigst
              später nur die echte Abbuchung.
            </span>
          </label>
          <label class="radio" class:sel={autoMode === "manual"}>
            <input type="radio" name="autoMode" value="manual" bind:group={autoMode} />
            <span>
              <strong>Nur an die Fälligkeit erinnern.</strong>
              Am Stichtag erscheint ein Hinweis, und das Abo steht als
              <em>„fällig“</em> in der Liste — gebucht wird nichts automatisch. Die
              Kostenposition legst du selbst per Knopf <em>„Buchen“</em> an.
            </span>
          </label>
        </fieldset>
        <div class="span2">
          <!-- Übergabe F7: Wortlaut statt „(Hinweis-Flag)“ -->
          <Toggle
            bind:checked={reverseCharge13bDefault}
            label="Erzeugte Kosten als §13b (Reverse-Charge) markieren"
            description="Für Lieferanten, die ohne Umsatzsteuer abrechnen (z. B. aus dem EU-Ausland). Die erzeugten Kosten tragen dann das §13b-Kennzeichen, USt 0; die selbst geschuldete Steuer meldest du in der Umsatzsteuer-Voranmeldung."
          />
        </div>
      </div>

      <p class="info-banner note">
        Erzeugte Kosten setzen Netto = Brutto, USt = 0 (das Abo kennt keine
        Aufteilung in Netto und USt). Weicht der echte Beleg ab, korrigierst du per
        Storno und Neuerfassung. Betrag je Termin: <strong class="kb-num">{euro(expectedCents ?? 0)}</strong>.
      </p>
      <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
      <div class="kb-form-end"><button type="button" class="btn-primary" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>{busy ? "Speichert …" : editId ? "Änderungen speichern" : "Abo anlegen"}</button></div>
    </form>
  </section>
{/if}

<style>
  /* .card / .info-banner / .kb-input: globale Definitionen aus tokens.css. */
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
  .grid :global(.money-field.with-label), .grid :global(.qty-field.with-label), .grid :global(.contact-select.with-label) { margin-bottom: 0; }
  .grid :global(.money-field label), .grid :global(.qty-field label), .grid :global(.contact-select label) { color: var(--text); }
  .modepick { border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--sp-3) var(--sp-4) var(--sp-4); margin: 0; min-width: 0; }
  .modepick legend { font-size: var(--fs-sm); color: var(--text); font-weight: 600; padding: 0 var(--sp-1); }
  /* U1-Sichtprüfung: Auf dem Handy kein zweiter Rahmen um die Auswahlkarten — sonst bleibt bei 320 px eine Textspalte von ~150 px. */
  @media (max-width: 600px) { .modepick { border: 0; padding: 0; } .modepick legend { padding: 0; margin-bottom: var(--sp-2); } }
  label.radio { display: flex; flex-direction: row; align-items: flex-start; gap: var(--sp-3); padding: var(--sp-3); border: 1px solid var(--line-control); border-radius: var(--r-sm); cursor: pointer; margin-top: var(--sp-2); min-height: var(--target); }
  label.radio span { font-size: var(--fs-sm); color: var(--text-muted); line-height: 1.45; }
  label.radio strong { color: var(--text); }
  label.radio em { font-style: normal; color: var(--accent-on-soft); }
  label.radio input { width: auto; margin-top: 0.15rem; padding: 0; flex: none; }
  label.radio.sel { border-color: var(--accent); background: var(--accent-soft); }
  label.radio:focus-within { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); }
  .note { margin: var(--sp-4) 0 0; }
  .muted { color: var(--text-muted); }
</style>
