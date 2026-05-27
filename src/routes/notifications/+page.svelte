<script lang="ts">
  import { goto } from "$app/navigation";
  import {
    notificationsList,
    notificationsDismiss,
    notificationsDismissAll,
    notificationsRunChecks,
  } from "$lib/api";
  import type { AppNotification } from "$lib/types";
  import { flash } from "$lib/toast.svelte";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import PageBar from "$lib/PageBar.svelte";

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

  function fmtDateTime(ts: string | null): string {
    if (!ts) return "—";
    const iso = ts.includes("T") ? ts : ts.replace(" ", "T") + "Z";
    const d = new Date(iso);
    return isNaN(d.getTime()) ? ts : d.toLocaleString("de-DE");
  }
</script>

<PageBar title="Hinweise">
  {#snippet actions()}
    <label class="chk">
      <input type="checkbox" bind:checked={includeDismissed} />
      Abgehakte zeigen
    </label>
    <button class="btn-secondary" onclick={runChecks} disabled={busy}>
      {busy ? "Prüfe …" : "Jetzt prüfen"}
    </button>
    {#if items.some((i) => !i.dismissedAt)}
      <button class="btn-ghost" onclick={dismissAll}>Alle abhaken</button>
    {/if}
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
  <p class="muted">Lade …</p>
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
            <strong>{item.title}</strong>
            <span class="ts">{fmtDateTime(item.triggeredAt)}</span>
          </div>
          <p class="text">{item.body}</p>
        </div>
        <div class="actions">
          {#if item.actionUrl}
            <button class="btn-secondary sm" onclick={() => open(item)}>Öffnen</button>
          {/if}
          {#if !item.dismissedAt}
            <button class="btn-ghost sm" onclick={() => dismiss(item.id)}>Abhaken</button>
          {:else}
            <span class="muted sm">abgehakt</span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{/if}

<p class="actions-foot">
  <Button variant="secondary" size="sm" href="/notifications/rules">Erinnerungen einstellen →</Button>
</p>

<style>
  .chk {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85rem;
    color: #4b5563;
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
    background: #fff;
    border: 1px solid #e5e7eb;
    border-left: 4px solid #9ca3af;
    border-radius: 8px;
    padding: 0.75rem 1rem;
  }
  .item.sev-warning {
    border-left-color: #d97706;
  }
  .item.sev-urgent {
    border-left-color: #dc2626;
  }
  .item.done {
    opacity: 0.55;
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
    color: #374151;
    line-height: 1.45;
  }
  .ts {
    color: #9ca3af;
    font-size: 0.78rem;
  }
  .badge {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    background: #f3f4f6;
    color: #4b5563;
  }
  .badge.sev-warning {
    background: #fef3c7;
    color: #92400e;
  }
  .badge.sev-urgent {
    background: #fee2e2;
    color: #991b1b;
  }
  .actions {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    flex-shrink: 0;
  }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .actions-foot {
    margin-top: 1.5rem;
  }
</style>
