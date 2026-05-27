<script lang="ts">
  // Block P4 — Paket-Katalog-Broschüre. KEIN §14-Beleg: Pakete auswählen →
  // PDF drucken/öffnen ODER per Kunden-Dropdown an einen Kontakt mailen.
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import { flash } from "$lib/toast.svelte";
  import {
    packagesList,
    packageCategoriesList,
    contactsList,
    mailAccountsList,
    packageCatalogRender,
    packageCatalogSend,
  } from "$lib/api";
  import type { Package, PackageCategory, Contact, MailAccount } from "$lib/types";

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let contacts = $state<Contact[]>([]);
  let accounts = $state<MailAccount[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  // Auswahl der Pakete (nur aktive mit veröffentlichter Version sind broschürbar).
  let selected = $state<Record<string, boolean>>({});
  let printing = $state(false);
  let sending = $state(false);

  // Versand-Felder.
  let accountId = $state("");
  let contactId = $state("");
  let subject = $state("");
  let body = $state("");

  onMount(async () => {
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
  });

  // Nur aktive Pakete mit veröffentlichter Version lassen sich in die Broschüre nehmen.
  let usable = $derived(
    packages.filter((p) => p.status === "active" && p.currentRevision != null),
  );

  let groups = $derived.by(() => {
    const g: { name: string; items: Package[] }[] = [];
    for (const c of categories) {
      const items = usable.filter((p) => p.categoryId === c.id);
      if (items.length) g.push({ name: c.name, items });
    }
    const orphans = usable.filter(
      (p) => !p.categoryId || !categories.some((c) => c.id === p.categoryId),
    );
    if (orphans.length) g.push({ name: "Ohne Kategorie", items: orphans });
    return g;
  });

  let selectedIds = $derived(usable.filter((p) => selected[p.id]).map((p) => p.id));
  let allSelected = $derived(usable.length > 0 && selectedIds.length === usable.length);

  // Nur Kontakte mit E-Mail können Empfänger sein.
  let emailContacts = $derived(contacts.filter((c) => (c.email ?? "").trim().length > 0));

  function toggleAll() {
    const next = !allSelected;
    const map: Record<string, boolean> = {};
    for (const p of usable) map[p.id] = next;
    selected = map;
  }

  async function printBrochure() {
    if (selectedIds.length === 0) {
      flash("Bitte mindestens ein Paket auswählen.", "error");
      return;
    }
    printing = true;
    try {
      await packageCatalogRender(selectedIds);
      flash("Broschüre erstellt — sie wird geöffnet.");
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      printing = false;
    }
  }

  async function sendBrochure() {
    if (selectedIds.length === 0) {
      flash("Bitte mindestens ein Paket auswählen.", "error");
      return;
    }
    if (!accountId) {
      flash("Bitte ein Absender-Postfach wählen.", "error");
      return;
    }
    if (!contactId) {
      flash("Bitte einen Empfänger (Kontakt) wählen.", "error");
      return;
    }
    sending = true;
    try {
      const res = await packageCatalogSend({
        accountId,
        contactId,
        packageIds: selectedIds,
        subject: subject.trim() || null,
        body: body.trim() || null,
      });
      flash(`Broschüre an ${res.to} gesendet (${res.packageCount} Pakete).`);
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      sending = false;
    }
  }
</script>

<PageBar back="/packages" backLabel="Pakete" title="Paket-Broschüre">
  {#snippet actions()}
    <Button
      variant="primary"
      disabled={printing || selectedIds.length === 0}
      onclick={printBrochure}
    >
      {printing ? "Erstelle …" : "PDF erstellen"}
    </Button>
  {/snippet}
</PageBar>

<p class="lead">
  Stelle eine unverbindliche Paket-Übersicht für Kund:innen zusammen: Pakete
  auswählen, als PDF drucken oder direkt an einen Kontakt mailen. Die Broschüre ist
  <strong>kein Rechnungsbeleg</strong> — sie bekommt keine Belegnummer und wird nicht
  archiviert; ein Versand wird nur im E-Mail-Protokoll vermerkt.
</p>

{#if loading}
  <p class="muted">Lädt …</p>
{:else if loadError}
  <div class="error-card">Konnte Daten nicht laden: {loadError}</div>
{:else if usable.length === 0}
  <div class="empty">
    Es gibt noch keine aktiven Pakete mit veröffentlichter Version. Lege zuerst unter
    „Pakete" ein Paket an.
  </div>
{:else}
  <section class="select-head">
    <label class="check">
      <input type="checkbox" checked={allSelected} onchange={toggleAll} />
      Alle auswählen
    </label>
    <span class="muted">{selectedIds.length} von {usable.length} ausgewählt</span>
  </section>

  {#each groups as group (group.name)}
    <h2 class="cat-h">{group.name}</h2>
    <div class="grid">
      {#each group.items as p (p.id)}
        <label class="pkg" class:sel={selected[p.id]}>
          <input type="checkbox" bind:checked={selected[p.id]} />
          <div class="pkg-body">
            <div class="pkg-name">{p.name}</div>
            <div class="pkg-meta">
              <Badge tone="neutral">Version {p.currentRevision}</Badge>
            </div>
          </div>
        </label>
      {/each}
    </div>
  {/each}

  <section class="send card">
    <h2 class="send-h">Per E-Mail senden</h2>
    {#if accounts.length === 0}
      <p class="muted">
        Kein Absender-Postfach konfiguriert. Lege eines unter Einstellungen → E-Mail an.
      </p>
    {:else if emailContacts.length === 0}
      <p class="muted">
        Kein Kontakt mit hinterlegter E-Mail-Adresse vorhanden.
      </p>
    {:else}
      <div class="form-row">
        <label class="field">
          <span>Absender-Postfach</span>
          <select class="kb-input" bind:value={accountId}>
            {#each accounts as a (a.id)}
              <option value={a.id}>{a.label} ({a.fromEmail})</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span>Empfänger (Kontakt)</span>
          <select class="kb-input" bind:value={contactId}>
            <option value="" disabled>— Kontakt wählen —</option>
            {#each emailContacts as c (c.id)}
              <option value={c.id}>{c.name} ({c.email})</option>
            {/each}
          </select>
        </label>
      </div>
      <label class="field">
        <span>Betreff (optional)</span>
        <input class="kb-input" type="text" placeholder="Unsere Leistungspakete" bind:value={subject} />
      </label>
      <label class="field">
        <span>Nachricht (optional)</span>
        <textarea class="kb-input" rows="4" placeholder="Wird automatisch erzeugt, wenn leer." bind:value={body}></textarea>
      </label>
      <div class="send-actions">
        <Button
          variant="primary"
          disabled={sending || selectedIds.length === 0 || !contactId}
          onclick={sendBrochure}
        >
          {sending ? "Sende …" : "Broschüre senden"}
        </Button>
      </div>
    {/if}
  </section>
{/if}

<style>
  /* .intro entfernt — globale .lead aus tokens.css. */
  .muted {
    color: var(--c-text-muted);
  }
  .error-card {
    color: var(--c-danger-700);
    background: var(--c-danger-50);
    border: 1px solid var(--c-danger-500);
    border-radius: var(--r-lg);
    padding: 1rem 1.2rem;
    max-width: 46rem;
  }
  .empty {
    color: var(--c-text-muted);
    background: var(--c-surface);
    border: 1px dashed var(--c-border-strong);
    border-radius: var(--r-lg);
    padding: 1.5rem;
    max-width: 46rem;
  }
  .select-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    margin-bottom: 0.5rem;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    cursor: pointer;
  }
  .cat-h {
    font-size: var(--fs-lg);
    margin: 1.25rem 0 0.7rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
    gap: 0.8rem;
  }
  .pkg {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 0.9rem 1rem;
    box-shadow: var(--sh-sm);
    cursor: pointer;
  }
  .pkg.sel {
    border-color: var(--c-primary-300);
    background: var(--c-primary-50);
  }
  .pkg input {
    margin-top: 0.2rem;
  }
  .pkg-body {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }
  .pkg-name {
    font-weight: 600;
    color: var(--c-text);
  }
  .pkg-meta {
    font-size: var(--fs-sm);
    color: var(--c-text-muted);
  }
  .send {
    margin-top: 2rem;
    max-width: 46rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 1.25rem 1.4rem;
    box-shadow: var(--sh-sm);
  }
  .send-h {
    font-size: var(--fs-lg);
    margin: 0 0 1rem;
  }
  .form-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: 1rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 0.9rem;
  }
  .field > span {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--c-text);
  }
  .send-actions {
    margin-top: 0.5rem;
  }
</style>
