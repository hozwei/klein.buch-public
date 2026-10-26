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
  <p class="muted" role="status" aria-busy="true">Lädt …</p>
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
  /* .intro entfernt — globale .lead aus tokens.css. U1: PageBar statt eigenem Kopf. */
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
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 0.85rem 1rem;
  }
  .text {
    margin: 0.3rem 0;
    font-size: 0.9rem;
    color: var(--text);
    line-height: 1.45;
    max-width: 40rem;
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }
  .switch .state {
    font-size: 0.85rem;
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
