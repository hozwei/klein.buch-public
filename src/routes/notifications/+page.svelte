<script lang="ts">
  import { goto } from "$app/navigation";
  import {
    notificationsList,
    notificationsDismiss,
    notificationsDismissAll,
    notificationsRunChecks,
  } from "#lib/api.ts";
  import type { AppNotification } from "#lib/types.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { date, dateTimeShort } from "#lib/format.ts";

  let items = $state<AppNotification[]>([]);
  let includeDismissed = $state(false);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      items = await notificationsList(includeDismissed);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Lädt beim Mount und bei jedem Wechsel von includeDismissed.
  $effect(() => {
    includeDismissed;
    load();
  });

  async function dismiss(id: string) {
    try {
      await notificationsDismiss(id);
      await load();
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function dismissAll() {
    try {
      const n = await notificationsDismissAll();
      flash(n > 0 ? `${n} Hinweis(e) abgehakt.` : "Nichts abzuhaken.");
      await load();
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function runChecks() {
    busy = true;
    try {
      const n = await notificationsRunChecks();
      flash(n > 0 ? `${n} neue Hinweis(e).` : "Keine neuen Hinweise.");
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  function open(item: AppNotification) {
    if (item.actionUrl) goto(item.actionUrl);
  }

  function severityLabel(s: string): string {
    return s === "urgent" ? "Dringend" : s === "warning" ? "Achtung" : "Info";
  }

  // Anzeige (U1, DIN 5008): Manche Hinweistexte des Servers nennen Kalenderdaten
  // als JJJJ-MM-TT; hier erscheinen sie als TT.MM.JJJJ. Nur gültige Daten
  // werden ersetzt, alles andere bleibt unverändert.
  function displayText(text: string): string {
    return text.replace(/(?<![\w-])(\d{4}-\d{2}-\d{2})(?![\w-])/g, (iso) => {
      const d = date(iso);
      return d === "—" ? iso : d;
    });
  }

</script>

<PageBar title="Hinweise">
  {#snippet actions()}
    <label class="chk">
      <input type="checkbox" bind:checked={includeDismissed} />
      Abgehakte zeigen
    </label>
    <Button variant="secondary" href="/notifications/rules">Erinnerungen einstellen</Button>
    {#if items.some((i) => !i.dismissedAt)}
      <button type="button" class="btn-ghost" onclick={dismissAll}>Alle abhaken</button>
    {/if}
    <button type="button" class="btn-primary" onclick={runChecks} disabled={busy} aria-busy={busy}>
      {busy ? "Prüfe …" : "Jetzt prüfen"}
    </button>
  {/snippet}
</PageBar>

<p class="lead">
  Klein.Buch erinnert dich an wiederkehrende Aufgaben (Belege erfassen,
  Geschäftsjahr abschließen, Backup) und meldet kritische Ereignisse wie eine
  gestörte Archiv-Integrität. Erledigte Hinweise hakst du einfach ab.
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted" role="status" aria-busy="true">Lädt …</p>
{:else if items.length === 0}
  <Banner kind="info">
    {includeDismissed
      ? "Noch keine Hinweise vorhanden."
      : "Keine offenen Hinweise — alles erledigt."}
  </Banner>
{:else}
  <ul class="list">
    {#each items as item (item.id)}
      <li class="item sev-{item.severity}" class:done={item.dismissedAt}>
        <div class="body">
          <div class="line">
            <span class="badge sev-{item.severity}">{severityLabel(item.severity)}</span>
            <strong>{displayText(item.title)}</strong>
            <span class="ts">{dateTimeShort(item.triggeredAt)}</span>
          </div>
          <p class="text">{displayText(item.body)}</p>
        </div>
        <div class="actions">
          {#if item.actionUrl}
            <button
              type="button"
              class="btn-secondary"
              onclick={() => open(item)}
              aria-label={`Öffnen: ${item.title}`}
            >Öffnen</button>
          {/if}
          {#if !item.dismissedAt}
            <button
              type="button"
              class="btn-ghost"
              onclick={() => dismiss(item.id)}
              aria-label={`Abhaken: ${item.title}`}
            >Abhaken</button>
          {:else}
            <span class="muted">abgehakt {date(item.dismissedAt)}</span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{/if}


<style>
  .chk {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    min-height: var(--target);
    color: var(--text-muted);
    cursor: pointer;
  }
  /* .intro entfernt — globale .lead aus tokens.css. */
  .muted {
    color: var(--c-text-muted);
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 1rem 0 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .item {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-left: 4px solid var(--line-control);
    border-radius: var(--r-md);
    padding: 0.75rem 1rem;
    /* 2.4.11: fokussierte Aktionen nicht unter der klebenden PageBar. */
    scroll-margin-block: calc(var(--pagebar-h, 0px) + var(--sp-4)) var(--sp-6);
  }
  .item.sev-warning {
    border-left-color: var(--status-warning-line);
  }
  .item.sev-urgent {
    border-left-color: var(--status-danger-line);
  }
  /* Abgehakt: zurückgenommene Fläche statt Transparenz (Kontrast 1.4.3). */
  .item.done {
    background: var(--surface-page);
    border-style: dashed;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .text {
    margin: 0.35rem 0 0;
    font-size: 0.9rem;
    color: var(--text);
    line-height: 1.45;
  }
  .ts {
    color: var(--text-muted);
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
  }
  .badge {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 0.1rem 0.4rem;
    border-radius: var(--r-sm);
    background: var(--surface-page);
    color: var(--text-muted);
  }
  .badge.sev-warning {
    background: var(--status-warning-bg);
    color: var(--status-warning-fg);
  }
  .badge.sev-urgent {
    background: var(--status-danger-bg);
    color: var(--status-danger-fg);
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    flex-shrink: 0;
  }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */

  .body {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  /* Handy (Plan W10): Aktionen unter den Text statt daneben. */
  @media (max-width: 600px) {
    .item {
      flex-wrap: wrap;
    }
    .actions {
      flex-direction: row;
      flex-wrap: wrap;
    }
  }
</style>
