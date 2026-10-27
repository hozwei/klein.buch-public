<script lang="ts">
  import { notificationRulesList, notificationRulesSetEnabled } from "#lib/api.ts";
  import type { NotificationRule } from "#lib/types.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import PageBar from "#lib/PageBar.svelte";

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
      return "Hinweis in der Hinweis-Liste nach einer Sicherung: immer, wenn eine lokale oder externe Sicherung fehlschlägt, und nach einem erfolgreichen manuellen Backup.";
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
      case "recurring_due":
        return "Hinweis am Stichtag eines Kosten-Abos im Modus „Erinnerung“ – die Kosten-Position legst du dann mit „Buchen“ an.";
      default:
        return rule.label;
    }
  }

</script>

<PageBar back="/notifications" backLabel="Hinweise" title="Erinnerungen einstellen" />

<p class="lead">
  Lege fest, woran Klein.Buch dich automatisch erinnert. Erinnerungen erscheinen
  in der Hinweis-Liste.
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else}
  <ul class="list">
    {#each rules as rule (rule.id)}
      <li class="item">
        <div class="body">
          <strong>{rule.label}</strong>
          <p class="text">{describe(rule)}</p>
        </div>
        <span class="switch">
          <Toggle
            checked={rule.enabled === 1}
            ariaLabel={rule.label}
            onchange={(next) => toggle(rule, next)}
          />
          <span class="state" aria-hidden="true">{rule.enabled === 1 ? "An" : "Aus"}</span>
        </span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  /* .lead global aus tokens.css. U1: PageBar statt eigenem Kopf.
     U4 (design.md §2.4 „Lineatur statt Kästen“): eine Karte, Regeln durch Linien getrennt. */
  .muted {
    color: var(--text-muted);
  }
  .list {
    list-style: none;
    padding: 0 var(--sp-4);
    margin: 0;
    background: var(--surface-card);
    border: var(--rule);
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-card);
    max-width: 56rem;
  }
  .item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-3) 0;
    border-bottom: var(--rule);
  }
  .item:last-child { border-bottom: 0; }
  .text {
    margin: var(--sp-1) 0 0;
    font-size: var(--type-body);
    color: var(--text-muted);
    line-height: var(--lh-body);
    max-width: 40rem;
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    flex-shrink: 0;
  }
  .switch .state {
    font-size: var(--type-small);
    color: var(--text-muted);
    min-width: 2.4rem;
  }
  .body {
    min-width: 0;
    flex: 1 1 auto;
    overflow-wrap: anywhere;
  }
  /* U1-Sichtprüfung: Auf dem Handy steht der Schalter unter dem Text, sonst bleibt
     für die Beschreibung bei 320 px nur eine Spalte von ~120 px. */
  @media (max-width: 600px) {
    .item { flex-direction: column; align-items: flex-start; gap: var(--sp-1); }
  }
</style>
