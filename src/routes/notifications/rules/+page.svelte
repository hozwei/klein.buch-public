<script lang="ts">
  import { notificationRulesList, notificationRulesSetEnabled } from "$lib/api";
  import type { NotificationRule } from "$lib/types";
  import { flash } from "$lib/toast.svelte";
  import Banner from "$lib/Banner.svelte";
  import Toggle from "$lib/Toggle.svelte";

  let rules = $state<NotificationRule[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      rules = await notificationRulesList();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  async function toggle(rule: NotificationRule, next: boolean) {
    const previous = rule.enabled;
    rule.enabled = next ? 1 : 0; // optimistisch — Toggle hat sich visuell schon umgesetzt
    try {
      await notificationRulesSetEnabled(rule.id, next);
      flash(next ? "Erinnerung aktiviert." : "Erinnerung deaktiviert.");
    } catch (e) {
      rule.enabled = previous; // revert — bringt das Toggle visuell zurück
      flash(String(e), "error");
    }
  }

  function describe(rule: NotificationRule): string {
    if (rule.id === "rule_backup_result")
      return "Hinweis in der Hinweis-Liste nach jeder Sicherung: bei fehlgeschlagener externer Spiegelung (immer) und bei erfolgreichem manuellem Backup.";
    switch (rule.ruleType) {
      case "monthly_doc_check":
        return "Monatliche Erinnerung, alle Belege und Kosten des Vormonats zu erfassen.";
      case "fiscal_year_lock_pending":
        return "Erinnerung ab dem 1. Juni, das Vorjahr abzuschließen (Festschreibung + EÜR).";
      case "backup_overdue":
        return "Hinweis, wenn seit mehr als 7 Tagen kein erfolgreiches externes (Off-Site-)Backup lief – oder, ohne externes Ziel, wenn überhaupt keine Sicherung mehr erfolgte.";
      case "invoice_overdue":
        return "Hinweis auf festgeschriebene Rechnungen, deren Fälligkeitsdatum überschritten ist.";
      case "archive_integrity_failed":
        return "Dringender Hinweis, wenn der monatliche Archiv-Check einen Schaden findet.";
      default:
        return rule.label;
    }
  }

  function channels(rule: NotificationRule): string {
    const c: string[] = [];
    if (rule.deliverInApp) c.push("In-App");
    if (rule.deliverOsNative) c.push("System-Mitteilung");
    return c.join(" + ") || "—";
  }
</script>

<header class="hdr">
  <h1>Erinnerungen einstellen</h1>
  <a class="back" href="/notifications">← Zu den Hinweisen</a>
</header>

<p class="lead">
  Lege fest, woran Klein.Buch dich automatisch erinnert. Erinnerungen erscheinen
  in der Hinweis-Liste und – sofern aktiviert – als System-Mitteilung.
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted">Lade …</p>
{:else}
  <ul class="list">
    {#each rules as rule (rule.id)}
      <li class="item">
        <div class="body">
          <strong>{rule.label}</strong>
          <p class="text">{describe(rule)}</p>
          <span class="ch">Kanäle: {channels(rule)}</span>
        </div>
        <span class="switch">
          <Toggle
            checked={rule.enabled === 1}
            ariaLabel={rule.label}
            onchange={(next) => toggle(rule, next)}
          />
          <span class="state">{rule.enabled === 1 ? "An" : "Aus"}</span>
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .hdr {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
  }
  .back {
    color: #2563eb;
    text-decoration: none;
    font-size: 0.9rem;
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
    align-items: center;
    gap: 1rem;
    background: #fff;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    padding: 0.85rem 1rem;
  }
  .text {
    margin: 0.3rem 0;
    font-size: 0.9rem;
    color: #374151;
    line-height: 1.45;
    max-width: 40rem;
  }
  .ch {
    font-size: 0.78rem;
    color: #9ca3af;
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .switch .state {
    font-size: 0.85rem;
    color: #4b5563;
    min-width: 2.4rem;
  }
</style>
