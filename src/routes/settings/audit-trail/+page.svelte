<script lang="ts">
  import {
    auditTrailList,
    archiveIntegrityRun,
    archiveIntegrityHistory,
  } from "$lib/api";
  import type { AuditEntry, IntegrityCheckRow } from "$lib/types";
  import { flash } from "$lib/toast.svelte";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";

  let entries = $state<AuditEntry[]>([]);
  let history = $state<IntegrityCheckRow[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let scanning = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      const [e, h] = await Promise.all([
        auditTrailList(300),
        archiveIntegrityHistory(20),
      ]);
      entries = e;
      history = h;
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  async function runScan() {
    scanning = true;
    try {
      const s = await archiveIntegrityRun();
      if (s.filesFailed > 0) {
        flash(
          `${s.filesFailed} von ${s.filesChecked} Archiv-Datei(en) beschädigt!`,
          "error",
        );
      } else {
        flash(`${s.filesChecked} Datei(en) geprüft — alle unverändert.`);
      }
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      scanning = false;
    }
  }

  function fmtDateTime(ts: string | null): string {
    if (!ts) return "—";
    const iso = ts.includes("T") ? ts : ts.replace(" ", "T") + "Z";
    const d = new Date(iso);
    return isNaN(d.getTime()) ? ts : d.toLocaleString("de-DE");
  }

  const ACTION_LABELS: Record<string, string> = {
    "invoice.lock": "Rechnung festgeschrieben",
    "invoice.payment_recorded": "Zahlung erfasst",
    "invoice.cancel": "Rechnung storniert",
    "quote.sent": "Angebot versendet",
    "quote.converted": "Angebot in Rechnung umgewandelt",
    "expense.import_einvoice": "E-Rechnung importiert",
    "fiscal_year.close": "Geschäftsjahr abgeschlossen",
    "archive.store": "Datei archiviert",
    "archive.integrity_pass": "Integrität geprüft (ok)",
    "archive.integrity_fail": "Integrität gestört",
    "backup.restore.applied": "Backup zurückgespielt",
    "depreciation.lock": "Abschreibung gebucht",
    "depreciation.reset": "Abschreibung zurückgesetzt",
  };
  function actionLabel(a: string): string {
    return ACTION_LABELS[a] ?? a;
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Protokoll & Datensicherheit" />

<p class="lead">
  Klein.Buch protokolliert jede festschreibende Aktion lückenlos und unveränderbar
  (GoBD). Hier siehst du das Protokoll und kannst die Unversehrtheit deines
  Beleg-Archivs prüfen.
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted">Lade …</p>
{:else}
  <section class="card">
    <div class="card-hdr">
      <h2>Archiv-Integrität</h2>
      <button class="btn-secondary sm" onclick={runScan} disabled={scanning}>
        {scanning ? "Prüfe …" : "Jetzt prüfen"}
      </button>
    </div>
    <p class="muted small">
      Jede archivierte Datei (Rechnungs-PDF/XML, Belege) trägt eine Prüfsumme.
      Der Check vergleicht sie neu — so fällt jede stille Veränderung auf. Läuft
      automatisch einmal im Monat.
    </p>
    {#if history.length === 0}
      <p class="muted small">Noch keine Prüfung gelaufen.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Zeitpunkt</th>
            <th class="num">geprüft</th>
            <th class="num">ok</th>
            <th class="num">Fehler</th>
          </tr>
        </thead>
        <tbody>
          {#each history as h (h.id)}
            <tr class:bad={h.filesFailed > 0}>
              <td>{fmtDateTime(h.startedAt)}</td>
              <td class="num">{h.filesChecked}</td>
              <td class="num">{h.filesPassed}</td>
              <td class="num">{h.filesFailed}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <section class="card">
    <h2>Audit-Protokoll</h2>
    <p class="muted small">Die {entries.length} jüngsten Einträge (neueste zuerst).</p>
    {#if entries.length === 0}
      <p class="muted small">Noch keine Einträge.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Zeitpunkt (UTC)</th>
            <th>Aktion</th>
            <th>Bezug</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as e (e.id)}
            <tr>
              <td class="ts">{fmtDateTime(e.timestampUtc)}</td>
              <td>
                {actionLabel(e.action)}
                <span class="raw">{e.action}</span>
              </td>
              <td class="ref">
                {#if e.entityType}{e.entityType}{/if}
                {#if e.entityId}<span class="muted">· {e.entityId.slice(0, 8)}</span>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/if}

<style>
  /* .intro / .card / .card h2 entfernt — globale .lead bzw. .card aus tokens.css. */
  .muted {
    color: var(--c-text-muted);
  }
  .small {
    font-size: 0.8rem;
  }
  .card-hdr {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 0.5rem;
  }
  th,
  td {
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid #f3f4f6;
    font-size: 0.85rem;
    text-align: left;
  }
  th {
    font-size: 0.74rem;
    color: #6b7280;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  tr.bad td {
    color: #b91c1c;
    font-weight: 600;
  }
  .ts {
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  .raw {
    display: block;
    font-size: 0.7rem;
    color: #9ca3af;
    font-family: ui-monospace, monospace;
  }
  .ref {
    font-size: 0.8rem;
    color: #4b5563;
  }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
  /* .sm entfernt — Buttons app-weit gleich groß (Manuel-Hardline 2026-05-26). */
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
