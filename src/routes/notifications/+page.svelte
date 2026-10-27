<script lang="ts">
  // Hinweise (Block 13). U4 (U1-068, design.md §4.1/§4.2): PageBar mit einer
  // Hauptaktion („Jetzt prüfen“) und Nebenaktionen als `secondary` (bis 600 px im
  // Menü „Mehr“); der Filter „Abgehakte zeigen“ steht über der Liste. „Abhaken“ ist
  // `ghost` nur neben „Öffnen“, allein `secondary`. Schwere als Badge mit Text.
  import {
    notificationsList,
    notificationsDismiss,
    notificationsDismissAll,
    notificationsRunChecks,
  } from "#lib/api.ts";
  import type { AppNotification } from "#lib/types.ts";
  import { tick } from "svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { date, dateTimeShort } from "#lib/format.ts";

  let items = $state<AppNotification[]>([]);
  let includeDismissed = $state(false);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  // U4 (V1-13): Abhaken mit Rückmeldung und Fokus auf dem nächsten Eintrag.
  let dismissing = $state(false);
  let listEl: HTMLUListElement | undefined = $state();
  let emptyEl: HTMLParagraphElement | undefined = $state();
  let filterEl: HTMLInputElement | undefined = $state();

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

  /**
   * Fokus nach dem Abhaken (V1-13): erste Aktion des Eintrags, der jetzt an derselben
   * Stelle steht (sonst des vorigen); ohne solche Aktion der leere Zustand bzw. der Filter.
   * Ohne Ziel fiele der Fokus auf <body>, weil der Knopf verschwindet.
   */
  function focusAfterDismiss(index: number) {
    const rows = listEl ? Array.from(listEl.querySelectorAll<HTMLElement>(":scope > li")) : [];
    const order = [...rows.slice(index), ...rows.slice(0, index).reverse()];
    for (const row of order) {
      const target = row.querySelector<HTMLElement>("a[href], button");
      if (target) {
        target.focus();
        return;
      }
    }
    (emptyEl ?? filterEl)?.focus();
  }

  // Abhaken lässt sich nicht zurücknehmen (kein Server-Command dafür); der Toast bestätigt
  // und sagt, wo der Hinweis bleibt. Keine Rückfrage beim einzelnen Eintrag.
  async function dismiss(item: AppNotification) {
    if (dismissing) return;
    const index = items.findIndex((i) => i.id === item.id);
    dismissing = true;
    try {
      await notificationsDismiss(item.id);
      await load();
      flash(`Abgehakt: ${displayText(item.title)}. Abgehakte Hinweise zeigt „Abgehakte anzeigen“.`);
      await tick();
      focusAfterDismiss(Math.max(0, index));
    } catch (e) {
      flash(String(e), "error");
    } finally {
      dismissing = false;
    }
  }

  // „Alle abhaken“: Sind dringende Hinweise dabei (z. B. Archiv-Integrität), fragt ein
  // Dialog nach und nennt ihre Zahl (design.md §5: nicht umkehrbar → Rückfrage).
  async function dismissAll() {
    if (dismissing) return;
    const open = items.filter((i) => !i.dismissedAt);
    const urgent = open.filter((i) => i.severity === "urgent").length;
    if (urgent > 0) {
      const ok = await confirmDialog({
        title: open.length === 1 ? "Den dringenden Hinweis abhaken?" : `Alle ${open.length} offenen Hinweise abhaken?`,
        body:
          (open.length === 1
            ? "Er ist als dringend markiert"
            : urgent === 1
              ? "Darunter ist ein dringender Hinweis"
              : `Darunter sind ${urgent} dringende Hinweise`) +
          " (z. B. Archiv-Integrität oder Backup). Abhaken lässt sich nicht zurücknehmen; " +
          "abgehakte Hinweise bleiben unter „Abgehakte anzeigen“ sichtbar.",
        confirmLabel: "Alle abhaken",
      });
      if (!ok) return;
    }
    dismissing = true;
    try {
      const n = await notificationsDismissAll();
      flash(n === 0 ? "Nichts abzuhaken." : n === 1 ? "1 Hinweis abgehakt." : `${n} Hinweise abgehakt.`);
      await load();
      await tick();
      focusAfterDismiss(0);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      dismissing = false;
    }
  }

  async function runChecks() {
    if (busy) return;
    busy = true;
    try {
      const n = await notificationsRunChecks();
      flash(n === 0 ? "Keine neuen Hinweise." : n === 1 ? "1 neuer Hinweis." : `${n} neue Hinweise.`);
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }


  function severityLabel(s: string): string {
    return s === "urgent" ? "Dringend" : s === "warning" ? "Achtung" : "Info";
  }
  function severityTone(s: string): "danger" | "warning" | "info" {
    return s === "urgent" ? "danger" : s === "warning" ? "warning" : "info";
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

  const hasOpen = $derived(items.some((i) => !i.dismissedAt));
</script>

<PageBar
  title="Hinweise"
  secondary={[
    ...(hasOpen ? [{ label: "Alle abhaken", onSelect: dismissAll }] : []),
    { label: "Erinnerungen einstellen", href: "/notifications/rules" },
  ]}
>
  {#snippet actions()}
    <Button variant="primary" ariaDisabled={busy} onclick={runChecks}>
      {busy ? "Prüft …" : "Jetzt prüfen"}
    </Button>
    <HelpAnchor slug="hinweise" />
  {/snippet}
</PageBar>

<p class="lead">
  Klein.Buch erinnert dich an wiederkehrende Aufgaben (Belege erfassen,
  Geschäftsjahr abschließen, Backup) und meldet kritische Ereignisse wie eine
  gestörte Archiv-Integrität. Erledigte Hinweise hakst du einfach ab.
</p>

<div class="filter">
  <label class="chk">
    <input type="checkbox" bind:this={filterEl} bind:checked={includeDismissed} />
    Abgehakte anzeigen
  </label>
</div>

<!-- U4 (V2-02/V4-07): Ladetext nur beim ersten Laden; ein erneutes Laden nach dem
     Abhaken lässt die Liste stehen (sonst ginge der Fokus verloren), aria-busy an der
     Liste. V2-09: leerer Zustand als Text statt Banner, fokussierbar als Ziel nach dem
     letzten Abhaken. -->
{#if error}
  <Banner>{error}</Banner>
{:else if loading && items.length === 0}
  <p class="muted" role="status">Wird geladen …</p>
{:else if items.length === 0}
  <p class="muted empty" tabindex="-1" bind:this={emptyEl}>
    {includeDismissed ? "Noch keine Hinweise vorhanden." : "Keine offenen Hinweise."}
  </p>
{:else}
  <ul class="list" bind:this={listEl} aria-busy={loading}>
    {#each items as item (item.id)}
      <li class="item sev-{item.severity}" class:done={item.dismissedAt}>
        <div class="body">
          <div class="line">
            <Badge tone={severityTone(item.severity)}>{severityLabel(item.severity)}</Badge>
            <strong>{displayText(item.title)}</strong>
            <span class="ts">{dateTimeShort(item.triggeredAt)}</span>
          </div>
          <p class="text">{displayText(item.body)}</p>
        </div>
        <div class="actions kb-actions">
          {#if item.actionUrl}
            <Button variant="secondary" href={item.actionUrl} ariaLabel={`Öffnen: ${item.title}`}>Öffnen</Button>
          {/if}
          {#if !item.dismissedAt}
            <!-- ghost nur neben „Öffnen“, allein secondary (design.md §4.1, U1-068). -->
            <Button
              variant={item.actionUrl ? "ghost" : "secondary"}
              onclick={() => dismiss(item)}
              ariaDisabled={dismissing}
              ariaLabel={`Abhaken: ${item.title}`}
            >Abhaken</Button>
          {:else}
            <span class="muted done-at">abgehakt {date(item.dismissedAt)}</span>
          {/if}
        </div>
      </li>
    {/each}
  </ul>
{/if}


<style>
  .filter { margin: 0 0 var(--sp-3); }
  .chk {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--target);
    color: var(--text-muted);
    cursor: pointer;
  }
  /* .lead global aus tokens.css. */
  .muted {
    color: var(--text-muted);
  }
  .empty {
    margin: var(--sp-3) 0;
  }
  /* U4 (V3-05): eine Karte für die ganze Liste, Einträge durch Lineatur getrennt (design.md
     §2 Nr. 4) statt eines Kartenstapels; die Schwere bleibt als Badge und schmale Kante. */
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    background: var(--surface-card);
    border: var(--rule);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-card);
    overflow: hidden;
  }
  .item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-4);
    border-left: 3px solid var(--line-control);
    padding: var(--sp-3) var(--sp-4);
    /* 2.4.11: fokussierte Aktionen nicht unter der klebenden PageBar. */
    scroll-margin-block: calc(var(--pagebar-h, 0px) + var(--sp-4)) var(--sp-6);
  }
  .item + .item {
    border-top: var(--rule);
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
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    flex-wrap: wrap;
  }
  .text {
    margin: var(--sp-1) 0 0;
    font-size: var(--type-body);
    color: var(--text);
    line-height: var(--lh-body);
  }
  .ts {
    color: var(--text-muted);
    font-size: var(--type-caption);
    font-variant-numeric: tabular-nums;
  }
  /* Aktionen rechts nebeneinander (.kb-actions, V3-05: halbiert die Zeilenhöhe am Desktop). */
  .item .actions { flex-shrink: 0; flex-wrap: nowrap; }
  .done-at { font-size: var(--type-small); }
  .body {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  /* Handy (Plan W10): Aktionen unter den Text statt daneben, linksbündig nebeneinander. */
  @media (max-width: 600px) {
    .item {
      flex-wrap: wrap;
      align-items: flex-start;
    }
    .item .actions {
      flex-wrap: wrap;
      justify-content: flex-start;
    }
  }
</style>
