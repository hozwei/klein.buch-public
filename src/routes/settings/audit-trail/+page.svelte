<script lang="ts">
  import { onMount } from "svelte";
  import {
    auditTrailList,
    archiveIntegrityRun,
    archiveIntegrityHistory,
  } from "#lib/api.ts";
  import type { AuditEntry, IntegrityCheckRow } from "#lib/types.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import { dateTime } from "#lib/format.ts";
  import { AUDIT_ACTION_LABELS_ADDITIONAL } from "#lib/labels.ts";

  let entries = $state<AuditEntry[]>([]);
  let history = $state<IntegrityCheckRow[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let scanning = $state(false);

  async function fetchAll() {
    const [e, h] = await Promise.all([
      auditTrailList(300),
      archiveIntegrityHistory(20),
    ]);
    entries = e;
    history = h;
  }

  async function load() {
    loading = true;
    error = null;
    try {
      await fetchAll();
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }
  // U4 (V2-07/V2-02/V2-06): Fehler mit „Erneut laden“, Ladezustand als Statusmeldung,
  // „Jetzt prüfen“ während der Prüfung fokussierbar (ariaDisabled).
  onMount(load);

  async function runScan() {
    if (scanning) return;
    scanning = true;
    try {
      const s = await archiveIntegrityRun();
      if (s.filesFailed > 0) {
        flash(
          `${s.filesFailed} von ${s.filesChecked} Archiv-Dateien beschädigt oder fehlend. Einzelheiten unter „Archiv-Integrität“.`,
          "error",
        );
      } else if (s.filesUnreadable > 0) {
        flash(
          `${s.filesUnreadable} von ${s.filesChecked} Archiv-Dateien nicht lesbar (z. B. Dateirechte) — kein Manipulationsbefund, Prüfung aber unvollständig.`,
          "error",
        );
      } else {
        flash(s.filesChecked === 1 ? "1 Datei geprüft — unverändert." : `${s.filesChecked} Dateien geprüft — alle unverändert.`);
      }
      // Ohne Ladezustand nachladen: „Jetzt prüfen“ bleibt stehen und behält den Fokus.
      await fetchAll();
    } catch (e) {
      flash(`Archiv prüfen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      scanning = false;
    }
  }

  const ACTION_LABELS: Record<string, string> = {
    "invoice.draft_create": "Rechnungs-Entwurf angelegt",
    "invoice.draft_updated": "Rechnungs-Entwurf geändert",
    "invoice.lock": "Rechnung festgeschrieben",
    "invoice.sent": "Rechnung versendet",
    "invoice.payment_recorded": "Zahlung erfasst",
    "invoice.cancel": "Rechnung storniert",
    "invoice.draft_voided": "Rechnungs-Entwurf verworfen",
    "storno.create": "Storno-Beleg erstellt",
    "payment_account.default_cleared": "Standardkonto bereinigt (Migration)",
    "quote.draft_create": "Angebots-Entwurf angelegt",
    "quote.draft_updated": "Angebots-Entwurf geändert",
    "quote.issue": "Angebot ausgestellt",
    "quote.sent": "Angebot versendet",
    "quote.accepted": "Angebot angenommen",
    "quote.rejected": "Angebot abgelehnt",
    "quote.cancel": "Angebot verworfen",
    "quote.converted": "Angebot in Rechnung umgewandelt",
    "expense.create": "Kosten erfasst",
    "expense.cancel": "Kosten storniert",
    "expense.import_einvoice": "E-Rechnung importiert",
    "private_movement.create": "Privatbewegung erfasst",
    "asset.create": "Anlage angelegt",
    "asset.dispose": "Anlage abgegangen",
    "fiscal_year.close": "Geschäftsjahr abgeschlossen",
    "euer.pdf_export": "EÜR als PDF exportiert",
    "euer.datev_export": "DATEV-Buchungsstapel exportiert",
    "euer.elster_export": "ELSTER-Ausfüllhilfe exportiert",
    "euer.detail_export": "Einzelaufstellung exportiert",
    "euer.stb_export": "Steuerberater-Paket exportiert",
    "euer.snapshot_mismatch": "EÜR weicht vom Jahresabschluss ab",
    "dsgvo.export": "DSGVO-Auskunft erstellt",
    "contact.anonymize": "Kontakt anonymisiert",
    "migration.export": "Migrations-Export erstellt",
    "archive.store": "Datei archiviert",
    "archive.read": "Archiv-Datei abgerufen",
    "archive.integrity_pass": "Integrität geprüft (ok)",
    "archive.integrity_fail": "Integrität gestört (Prüfsumme weicht ab)",
    "archive.integrity_missing": "Archiv-Datei fehlt",
    "archive.integrity_unreadable": "Archiv-Datei nicht lesbar",
    "backup.created": "Backup erstellt",
    "backup.failed": "Backup fehlgeschlagen",
    "backup.downloaded": "Backup heruntergeladen",
    "backup.pre_migration": "Pflicht-Backup vor Migration",
    "backup.restore.requested": "Wiederherstellung angefordert",
    "backup.restore.applied": "Backup zurückgespielt",
    "backup.passphrase.changed": "Daten-Passwort geändert",
    "auth.login.ok": "Angemeldet",
    "auth.reauth.ok": "Anmeldung bestätigt",
    "auth.sessions_revoked": "Andere Sitzungen abgemeldet",
    "auth.totp.renewed": "Zweiter Faktor erneuert",
    "auth.recovery.renewed": "Einmal-Codes erneuert",
    "depreciation.accrue": "Abschreibung gebucht",
    "depreciation.lock": "Abschreibung gebucht",
    "depreciation.reset": "Abschreibung zurückgesetzt",
    "numbering.raised": "Nummernkreis angehoben",
    "scheduler.hold_set": "Automatik nach Wiederherstellung pausiert",
    "scheduler.hold_released": "Automatik wieder freigegeben",
  };
  const ALL_ACTION_LABELS: Record<string, string> = { ...ACTION_LABELS, ...AUDIT_ACTION_LABELS_ADDITIONAL };
  function actionLabel(a: string): string {
    return ALL_ACTION_LABELS[a] ?? a;
  }

  const ACTOR_LABELS: Record<string, string> = {
    owner: "Du",
    scheduler: "Zeitplan",
    system: "System",
  };
  function actorLabel(a: string): string {
    return ACTOR_LABELS[a] ?? a;
  }

  function parseJson(s: string | null): unknown {
    if (!s) return null;
    try {
      return JSON.parse(s);
    } catch {
      return null;
    }
  }
  function idList(s: string | null): string[] {
    const v = parseJson(s);
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
  }
  function baseName(p: string): string {
    const parts = p.split(/[\\/]/);
    return parts[parts.length - 1] || p;
  }

  /** Detail eines Archiv-Eintrags (Dateiname bzw. Fehlertext), sonst null. */
  function archiveDetail(e: AuditEntry): string | null {
    if (!e.action.startsWith("archive.")) return null;
    const d = parseJson(e.detailsJson) as { path?: unknown; error?: unknown } | null;
    if (!d) return null;
    if (typeof d.path === "string") return baseName(d.path);
    if (typeof d.error === "string") return d.error;
    return null;
  }

  /** Archiv-ID → betroffene Datei (aus den Integritäts-Einträgen des Protokolls). */
  const fileByArchiveId = $derived.by(() => {
    const m = new Map<string, string>();
    for (const e of entries) {
      if (!e.entityId || !e.action.startsWith("archive.integrity_")) continue;
      if (m.has(e.entityId)) continue;
      const det = archiveDetail(e);
      if (det) m.set(e.entityId, det);
    }
    return m;
  });

  type Affected = { id: string; kind: string; file: string | null };
  function affected(h: IntegrityCheckRow): Affected[] {
    const missing = new Set(idList(h.missingArchiveIdsJson));
    const out: Affected[] = [];
    for (const id of idList(h.failedArchiveIdsJson)) {
      out.push({ id, kind: missing.has(id) ? "fehlt" : "Prüfsumme weicht ab", file: fileByArchiveId.get(id) ?? null });
    }
    for (const id of missing) {
      if (!out.some((a) => a.id === id)) out.push({ id, kind: "fehlt", file: fileByArchiveId.get(id) ?? null });
    }
    for (const id of idList(h.unreadableArchiveIdsJson)) {
      out.push({ id, kind: "nicht lesbar", file: fileByArchiveId.get(id) ?? null });
    }
    return out;
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Protokoll & Datensicherheit" />

<p class="lead">
  Klein.Buch protokolliert jede festschreibende Aktion lückenlos und unveränderbar
  (GoBD). Hier siehst du das Protokoll und kannst die Unversehrtheit deines
  Beleg-Archivs prüfen.
</p>

{#if error}
  <Banner title="Protokoll und Prüfungen konnten nicht geladen werden.">{error}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if loading}
  <LoadingState />
{:else}
  <section class="card">
    <div class="card-hdr">
      <h2>Archiv-Integrität</h2>
      <Button variant="secondary" onclick={runScan} busy={scanning}>
        {scanning ? "Prüfe …" : "Jetzt prüfen"}
      </Button>
    </div>
    <p class="muted small">
      Jede archivierte Datei (Rechnungs-PDF/XML, Belege) trägt eine Prüfsumme.
      Der Check vergleicht sie neu — so fällt jede stille Veränderung auf. Läuft
      automatisch einmal im Monat.
    </p>
    {#if history.length === 0}
      <p class="muted small">Noch keine Prüfung gelaufen. Starte sie mit „Jetzt prüfen“.</p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Letzte Prüfungen des Archivs</caption>
        <thead>
          <tr>
            <th>Zeitpunkt</th>
            <th class="num">Geprüft</th>
            <th class="num">Ok</th>
            <th class="num">Fehler</th>
            <th class="num">Davon fehlend</th>
            <th class="num">Nicht lesbar</th>
            <th>Betroffene Dateien</th>
          </tr>
        </thead>
        <tbody>
          {#each history as h (h.id)}
            {@const aff = affected(h)}
            <tr class:bad={h.filesFailed > 0}>
              <td class="ts" data-label="Zeitpunkt">{dateTime(h.startedAt)}</td>
              <td class="num" data-label="Geprüft">{h.filesChecked}</td>
              <td class="num" data-label="Ok">{h.filesPassed}</td>
              <td class="num" data-label="Fehler">{h.filesFailed}</td>
              <td class="num" data-label="Davon fehlend">{h.filesMissing ?? "–"}</td>
              <td class="num" data-label="Nicht lesbar">{h.filesUnreadable ?? "–"}</td>
              {#if aff.length > 0}
                <td class="cards-full" data-label="Betroffene Dateien">
                  <details>
                    <summary>{aff.length} {aff.length === 1 ? "Datei" : "Dateien"} anzeigen</summary>
                    <ul class="aff">
                      {#each aff as a (a.id + a.kind)}
                        <li>
                          <span class="aff-kind">{a.kind}:</span>
                          {#if a.file}<span class="mono">{a.file}</span>{/if}
                          <span class="mono muted">Archiv-ID {a.id}</span>
                        </li>
                      {/each}
                    </ul>
                  </details>
                </td>
              {:else}
                <td class="muted none">—</td>
              {/if}
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
      <p class="muted small">Noch keine Einträge. Sie entstehen, sobald du z. B. eine Rechnung festschreibst.</p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Audit-Protokoll, neueste Einträge zuerst</caption>
        <thead>
          <tr>
            <th>Zeitpunkt</th>
            <th>Aktion</th>
            <th>Ausgelöst von</th>
            <th>Bezug</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as e (e.id)}
            {@const det = archiveDetail(e)}
            <tr>
              <td class="ts" data-label="Zeitpunkt">{dateTime(e.timestampUtc)}</td>
              <td class="cards-full" data-label="Aktion">
                {actionLabel(e.action)}
                {#if det}<span class="detail mono">{det}</span>{/if}
                <span class="raw">{e.action}</span>
              </td>
              <td data-label="Ausgelöst von">{actorLabel(e.actor)}</td>
              <td class="ref" data-label="Bezug">
                {#if e.entityType || e.entityId}
                  <span>{#if e.entityType}{e.entityType}{/if}
                  {#if e.entityId}<span class="muted mono">· {e.entityId.slice(0, 8)}</span>{/if}</span>
                {:else}
                  <span class="muted">—</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/if}

<style>
  /* .card / .lead — globale Definitionen aus tokens.css. */
  .muted { color: var(--text-muted); }
  .small { font-size: var(--fs-sm); }
  .mono { font-family: var(--font-mono); overflow-wrap: anywhere; }
  .card-hdr {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem 1rem;
  }
  table { width: 100%; border-collapse: collapse; margin-top: 0.5rem; }
  th, td { font-size: var(--fs-sm); text-align: left; overflow-wrap: anywhere; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  tr.bad td { color: var(--status-danger-fg); font-weight: 600; }
  .ts { font-variant-numeric: tabular-nums; }
  @media (min-width: 821px) {
    .ts { white-space: nowrap; }
  }
  .detail { display: block; font-size: var(--fs-xs); color: var(--text-muted); }
  .raw {
    display: block;
    font-size: var(--fs-xs);
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .ref { color: var(--text-muted); }
  details summary { cursor: pointer; min-height: var(--target); display: flex; align-items: center; font-weight: 600; }
  .aff { margin: 0.25rem 0 0; padding-left: 1.1rem; font-weight: 400; color: var(--text); }
  .aff li { margin-bottom: 0.25rem; }
  .aff .mono { display: block; font-size: var(--fs-xs); }
  .aff-kind { font-weight: 600; }
  @media screen and (max-width: 820px) {
    td.none { display: none; }
  }
</style>
