<script lang="ts">
  // Block P4 — Paket-Katalog-Broschüre. KEIN §14-Beleg: Pakete auswählen →
  // PDF drucken/öffnen ODER an einen Kontakt mailen.
  // U4: Empfänger über `ContactSelect` (Warteliste (15): durchsuchbar, deutsche
  // Sortierung); „PDF erstellen“ und „Broschüre senden“ bleiben bedienbar und
  // nennen den Grund am Auswahlbereich, wenn noch kein Paket gewählt ist (U1-067);
  // die Broschüre folgt der Reihenfolge der Kategorien.
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Button from "#lib/Button.svelte";
  import Banner from "#lib/Banner.svelte";
  import ContactSelect from "#lib/ContactSelect.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import {
    packagesList,
    packageCategoriesList,
    contactsList,
    mailAccountsList,
    packageCatalogRender,
    downloadUrl,
    packageCatalogSend,
  } from "#lib/api.ts";
  import type { Package, PackageCategory, Contact, MailAccount } from "#lib/types.ts";

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let contacts = $state<Contact[]>([]);
  let accounts = $state<MailAccount[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  // Auswahl der Pakete (nur aktive mit einer Version kommen in die Broschüre).
  let selected = $state<Record<string, boolean>>({});
  let printing = $state(false);
  let sending = $state(false);
  /** Grund am Auswahlbereich nach einem Versuch ohne Auswahl. */
  let selectionError = $state("");
  let allEl: HTMLInputElement | undefined = $state();

  // Versand-Felder.
  let accountId = $state("");
  let contactId = $state("");
  let subject = $state("");
  let body = $state("");
  // U1: Warnung des Versands (z. B. Versandprotokoll unvollständig) als Banner
  // statt Fehler-Toast; Empfänger-Fehler am Feld.
  let sendWarning = $state<string | null>(null);
  let sendTried = $state(false);
  let contactEl: ContactSelect | undefined = $state();

  async function load() {
    loading = true;
    loadError = null;
    try {
      [packages, categories, contacts, accounts] = await Promise.all([
        packagesList(),
        packageCategoriesList(),
        contactsList(false),
        mailAccountsList(),
      ]);
      const def = accounts.find((a) => a.isDefault === 1) ?? accounts[0];
      if (def) accountId = def.id;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // Nur aktive Pakete mit einer Version lassen sich in die Broschüre nehmen.
  let usable = $derived(
    packages.filter((p) => p.status === "active" && p.currentRevision != null),
  );

  let groups = $derived.by(() => {
    const g: { id: string; name: string; items: Package[] }[] = [];
    for (const c of categories) {
      const items = usable.filter((p) => p.categoryId === c.id);
      if (items.length) g.push({ id: c.id, name: c.name, items });
    }
    const orphans = usable.filter(
      (p) => !p.categoryId || !categories.some((c) => c.id === p.categoryId),
    );
    if (orphans.length) g.push({ id: "none", name: "Ohne Kategorie", items: orphans });
    return g;
  });

  // Reihenfolge wie angezeigt (Kategorien in ihrer Reihenfolge); der Server
  // übernimmt sie für die Broschüre.
  let selectedIds = $derived(groups.flatMap((g) => g.items).filter((p) => selected[p.id]).map((p) => p.id));
  let allSelected = $derived(usable.length > 0 && selectedIds.length === usable.length);

  // Nur Kontakte mit E-Mail können Empfänger sein.
  let emailContacts = $derived(contacts.filter((c) => (c.email ?? "").trim().length > 0));

  $effect(() => {
    if (selectedIds.length > 0) selectionError = "";
  });

  function toggleAll() {
    const next = !allSelected;
    const map: Record<string, boolean> = {};
    for (const p of usable) map[p.id] = next;
    selected = map;
  }

  /** Ohne Auswahl: Grund am Auswahlbereich zeigen und dorthin springen. */
  async function requireSelection(): Promise<boolean> {
    if (selectedIds.length > 0) return true;
    selectionError = "Bitte wähl mindestens ein Paket aus.";
    await tick();
    allEl?.focus();
    return false;
  }

  async function printBrochure() {
    if (printing || !(await requireSelection())) return;
    printing = true;
    try {
      const ticket = await packageCatalogRender(selectedIds);
      openPdfPreview(downloadUrl(ticket), "Paket-Broschüre", ticket.fileName);
    } catch (e) {
      flash(`PDF erstellen fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      printing = false;
    }
  }

  async function sendBrochure() {
    if (sending || !(await requireSelection())) return;
    sendTried = true;
    if (!accountId) {
      flash("Bitte wähl ein Absender-Postfach aus.", "error");
      return;
    }
    if (!contactEl?.validate()) {
      contactEl?.focus();
      return;
    }
    sendWarning = null;
    sending = true;
    try {
      const res = await packageCatalogSend({
        accountId,
        contactId,
        packageIds: selectedIds,
        subject: subject.trim() || null,
        body: body.trim() || null,
      });
      flash(
        `Broschüre an ${res.to} gesendet (${res.packageCount === 1 ? "1 Paket" : `${res.packageCount} Pakete`}).`,
      );
      if (res.warning) sendWarning = res.warning;
    } catch (e) {
      flash(`Broschüre senden fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      sending = false;
    }
  }
</script>

<PageBar back="/packages" backLabel="Pakete" title="Paket-Broschüre">
  {#snippet actions()}
    {#if usable.length > 0}
      <Button
        variant="primary"
        busy={printing}
        onclick={printBrochure}
        ariaDescribedby="brochure-count"
      >
        {printing ? "Erstelle …" : "PDF erstellen"}
      </Button>
    {/if}
  {/snippet}
</PageBar>

<p class="lead">
  Stelle eine unverbindliche Paket-Übersicht für Kund:innen zusammen: Pakete
  auswählen, als PDF drucken oder direkt an einen Kontakt mailen. Die Broschüre ist
  <strong>kein Rechnungsbeleg</strong> — sie bekommt keine Belegnummer und wird nicht
  archiviert; ein Versand wird nur im E-Mail-Protokoll vermerkt.
</p>

{#if loading}
  <LoadingState />
{:else if loadError}
  <Banner title="Pakete, Kontakte und Postfächer konnten nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if usable.length === 0}
  <div class="empty">
    <p>Es gibt noch keine aktiven Pakete. Leg zuerst ein Paket an.</p>
    <Button variant="primary" href="/packages/new">+ Neues Paket</Button>
  </div>
{:else}
  <section class="pick" aria-labelledby="brochure-pick-h">
    <h2 id="brochure-pick-h" class="kb-visually-hidden">Pakete auswählen</h2>
    <div class="select-head">
      <label class="check">
        <input
          type="checkbox"
          bind:this={allEl}
          checked={allSelected}
          onchange={toggleAll}
          aria-describedby={selectionError ? "brochure-select-err" : undefined}
        />
        Alle auswählen
      </label>
      <span class="muted" id="brochure-count" aria-live="polite">
        {selectedIds.length} von {usable.length} ausgewählt{selectedIds.length === 0 ? " — wähl mindestens ein Paket für PDF und Versand." : ""}
      </span>
    </div>
    {#if selectionError}<p id="brochure-select-err" class="err" role="alert">{selectionError}</p>{/if}

    {#each groups as group (group.id)}
      <fieldset class="group">
        <legend class="cat-h">{group.name}</legend>
        <div class="grid">
          {#each group.items as p (p.id)}
            <label class="pkg" class:sel={selected[p.id]}>
              <input type="checkbox" bind:checked={selected[p.id]} />
              <span class="pkg-body">
                <span class="pkg-name">{p.name}</span>
                <span class="pkg-meta">Version {p.currentRevision}</span>
              </span>
            </label>
          {/each}
        </div>
      </fieldset>
    {/each}
  </section>

  <section class="card send" aria-labelledby="brochure-send-h">
    <h2 id="brochure-send-h">Per E-Mail senden</h2>
    {#if accounts.length === 0}
      <p class="muted">
        Kein Absender-Postfach eingerichtet. Lege eines unter
        <a class="kb-link" href="/settings/mail">Einstellungen → E-Mail-Versand</a> an.
      </p>
    {:else if emailContacts.length === 0}
      <p class="muted">
        Kein Kontakt mit hinterlegter E-Mail-Adresse vorhanden. Ergänze die Adresse unter
        <a class="kb-link" href="/contacts">Kontakte</a>.
      </p>
    {:else}
      <div class="field">
        <label for="brochure-account">Absender-Postfach</label>
        <select id="brochure-account" class="kb-input" bind:value={accountId}>
          {#each accounts as a (a.id)}
            <option value={a.id}>{a.label} ({a.fromEmail})</option>
          {/each}
        </select>
      </div>
      <ContactSelect
        bind:this={contactEl}
        bind:value={contactId}
        id="brochure-contact"
        contacts={emailContacts}
        label="Empfänger (Kontakt)"
        required
        requiredMessage="Bitte wähl einen Empfänger aus."
        placeholder="— Kontakt wählen —"
        detail={(c) => c.email ?? ""}
        showErrors={sendTried}
      />
      <div class="field">
        <label for="brochure-subject">Betreff (optional)</label>
        <input id="brochure-subject" class="kb-input" type="text" autocomplete="off" placeholder="Unsere Leistungspakete" bind:value={subject} />
      </div>
      <div class="field">
        <label for="brochure-body">Nachricht (optional)</label>
        <textarea id="brochure-body" class="kb-input" rows="4" aria-describedby="brochure-body-hint" bind:value={body}></textarea>
        <span id="brochure-body-hint" class="hint">Leer lassen, dann wird ein Standardtext erzeugt.</span>
      </div>
      <div class="kb-actions kb-start">
        <Button
          variant="primary"
          busy={sending}
          onclick={sendBrochure}
          ariaDescribedby="brochure-count"
        >
          {sending ? "Sende …" : "Broschüre senden"}
        </Button>
      </div>
      {#if sendWarning}
        <div class="send-warning"><Banner kind="warning">{sendWarning}</Banner></div>
      {/if}
    {/if}
  </section>
{/if}

<style>
  /* .lead global aus tokens.css. */
  .muted { color: var(--text-muted); }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-3);
  }
  .empty p { margin: 0; color: var(--text-muted); }
  .select-head {
    display: flex;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
    flex-wrap: wrap;
  }
  .check {
    display: inline-flex;
    align-items: center;
    min-height: var(--target);
    gap: var(--sp-2);
    font-weight: 600;
    cursor: pointer;
  }
  .err { margin: var(--sp-1) 0 0; font-size: var(--type-small); font-weight: 600; color: var(--status-danger-fg); }
  .group { border: 0; padding: 0; margin: var(--sp-5) 0 0; min-width: 0; }
  .cat-h {
    font-size: var(--type-heading);
    font-weight: 700;
    padding: 0;
    margin: 0 0 var(--sp-3);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 17rem), 1fr));
    gap: var(--sp-3);
  }
  /* Auswahl-Kachel: ganze Fläche ist Trefferfläche; Haarlinie statt Schatten (design.md §4.4). */
  .pkg {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    background: var(--surface-card);
    border: var(--rule-control) var(--line);
    border-radius: var(--radius-card);
    padding: var(--sp-3) var(--sp-4);
    cursor: pointer;
  }
  .pkg.sel {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .pkg input { margin-top: 0.2rem; }
  .pkg-body { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .pkg-name { font-weight: 600; color: var(--text); overflow-wrap: anywhere; }
  .pkg-meta { font-size: var(--type-small); color: var(--text-muted); }
  .pkg.sel .pkg-meta { color: var(--accent-on-soft); }
  .send { margin-top: var(--sp-6); max-width: 46rem; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); margin-bottom: var(--sp-4); min-width: 0; }
  .field > label { font-size: var(--type-small); font-weight: 600; color: var(--text); }
  .hint { font-size: var(--type-small); color: var(--text-muted); }
  .field select, .field input, .field textarea { scroll-margin-block: var(--sp-6); }
  .send-warning { margin-top: var(--sp-3); }
</style>
