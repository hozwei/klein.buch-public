<script lang="ts">
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import {
    recurringInvoicesList,
    recurringInvoicesSetActive,
    recurringInvoicesRunNow,
    recurringInvoicesRunDueCheck,
    recurringInvoicesResolveBacklog,
    recurringRestoreReviewList,
    contactsList,
    invoicesGet,
    invoicesList,
  } from "#lib/api.ts";
  import { askBacklogChoice, withinTerm } from "#lib/recurringChoice.ts";
  import type { RecurringInvoiceRow, Contact, Invoice, ResumeMode } from "#lib/types.ts";
  import { date } from "#lib/format.ts";
  import { daysUntilBerlin } from "#lib/date.ts";
  import { dayClock } from "#lib/dayClock.svelte.ts";
  import { focusRowAfterRemoval } from "#lib/focusAfterRemoval.ts";
  import { frequencyLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import ActionMenu, { type ActionMenuItem } from "#lib/ActionMenu.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";

  let rows = $state<RecurringInvoiceRow[]>([]);
  let inactiveChk = $state<HTMLInputElement | null>(null);
  let contactInfo = $state<Record<string, { name: string; email: string | null }>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let busyId = $state<string | null>(null);
  let checking = $state(false);
  // Vorlagen, die nach einer Wiederherstellung auf „Rückstand …“ warten (F1d).
  let reviewIds = $state<Set<string>>(new Set());

  async function load() {
    loading = true;
    error = null;
    try {
      rows = await recurringInvoicesList(includeInactive);
      reviewIds = new Set((await recurringRestoreReviewList()).templates);
      if (Object.keys(contactInfo).length === 0) {
        const cs = await contactsList(true);
        contactInfo = Object.fromEntries(cs.map((c: Contact) => [c.id, { name: c.name, email: c.email ?? null }]));
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Das Öffnen der Seite löst keinen Lauf aus (W14-030, Lauf F): fällige
  // Rechnungen erstellt der Scheduler-Tick; „fällige jetzt erstellen“ ist ein
  // ausdrücklicher Knopf.
  onMount(load);

  // Tage bis zur Fälligkeit (negativ = überfällig) ab dem Berliner Datum,
  // nicht in der Zeitzone des Geräts (Lauf F, D-15). U7 (R4-07): über `dayClock`,
  // damit Fälligkeit und „Jetzt erstellen“ über Mitternacht mitlaufen.
  function daysUntil(iso: string): number {
    return daysUntilBerlin(iso, dayClock.now);
  }

  // Fällig nur innerhalb der Laufzeit (F1d, wie `has_backlog` im Server).
  function isDue(s: RecurringInvoiceRow): boolean {
    return s.active === 1 && withinTerm(s) && daysUntil(s.nextDueDate) <= 0;
  }

  // Rückstand: verpasste Termine oder offene Prüfung nach einer Wiederherstellung.
  function hasBacklog(s: RecurringInvoiceRow): boolean {
    return withinTerm(s) && (daysUntil(s.nextDueDate) < 0 || (reviewIds.has(s.id) && daysUntil(s.nextDueDate) <= 0));
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

  function dueBadge(s: RecurringInvoiceRow): { label: string; tone: Tone } | null {
    if (s.active !== 1) return null;
    if (!withinTerm(s)) return { label: "Beendet", tone: "neutral" };
    const n = daysUntil(s.nextDueDate);
    if (n < 0) return { label: "Überfällig", tone: "danger" };
    if (n === 0) return { label: "Heute fällig", tone: "warning" };
    if (n <= 7) return { label: `In ${n} ${n === 1 ? "Tag" : "Tagen"}`, tone: "info" };
    return null;
  }

  function rhythm(s: RecurringInvoiceRow): string {
    // U3: geschütztes Leerzeichen, damit „zum“ und der Tag nicht getrennt umbrechen.
    return `${frequencyLabel(s.frequency)} · zum ${s.dayOfPeriod}.`;
  }

  // V1-12: Modus-Badges heißen wie die Optionen im Formular. Ein Modus ist kein
  // Status, darum neutral — außer „Ausstellen + senden“ (Warnton), weil dann ohne
  // Zutun eine Rechnung an den Kunden geht.
  function modeBadge(s: RecurringInvoiceRow): { label: string; tone: Tone } {
    switch (s.autoMode) {
      case "issue":
        return { label: "Automatisch ausstellen", tone: "neutral" };
      case "issue_send":
        return { label: "Ausstellen + senden", tone: "warning" };
      default:
        return { label: "Nur Entwurf", tone: "neutral" };
    }
  }

  // Nach einer Wiederherstellung (Prüfung offen) entsteht immer nur ein Entwurf (F1d).
  function effectiveMode(s: RecurringInvoiceRow): string {
    return reviewIds.has(s.id) ? "draft" : s.autoMode;
  }

  function recipient(s: RecurringInvoiceRow): string {
    return contactInfo[s.contactId]?.email || "den Kunden";
  }

  // Zeilenaktionen (U3-036, design.md §4.5): eine sichtbare Aktion — die, die den
  // Zustand der Zeile auflöst — und der Rest im Menü „Mehr“. Bei einem Rückstand ist
  // das „Rückstand …“ (Nachholen erstellt auch den fälligen Termin), sonst bei
  // Fälligkeit „Jetzt erstellen“, sonst „Bearbeiten“; pausiert „Fortsetzen“.
  type RowAction = { label: string; ariaLabel: string; href?: string; onclick?: () => void };
  function rowActions(s: RecurringInvoiceRow): { main: RowAction; more: ActionMenuItem[] } {
    const busy = busyId === s.id;
    const edit: RowAction = { label: "Bearbeiten", ariaLabel: `„${s.label}“ bearbeiten`, href: `/recurring-invoices/new?id=${s.id}` };
    if (s.active !== 1) {
      return {
        main: { label: "Fortsetzen", ariaLabel: `„${s.label}“ fortsetzen`, onclick: () => resume(s) },
        more: [{ label: edit.label, href: edit.href }],
      };
    }
    const create: RowAction = {
      label: "Jetzt erstellen",
      ariaLabel: `Rechnung für „${s.label}“ jetzt erstellen`,
      onclick: () => createNow(s),
    };
    const backlog: RowAction = {
      label: "Rückstand …",
      ariaLabel: `Rückstand von „${s.label}“ klären`,
      onclick: () => resolveBacklog(s),
    };
    const main = hasBacklog(s) ? backlog : isDue(s) ? create : edit;
    const more: ActionMenuItem[] = [];
    if (main !== create && isDue(s)) more.push({ label: create.label, onSelect: create.onclick, disabled: busy });
    if (main !== edit) more.push({ label: edit.label, href: edit.href });
    more.push({ label: "Pausieren", onSelect: () => pause(s), disabled: busy });
    return { main, more };
  }

  let dueRows = $derived(rows.filter(isDue));
  let dueCount = $derived(dueRows.length);
  let activeCount = $derived(rows.filter((s) => s.active === 1).length);

  // V2-12 (design.md §5): Pausieren ist umkehrbar — keine Rückfrage, dafür ein
  // Toast mit „Rückgängig“ (ersetzt die Rückfrage aus KB-0057).
  async function pause(s: RecurringInvoiceRow) {
    if (busyId) return;
    const index = rows.findIndex((x) => x.id === s.id);
    busyId = s.id;
    try {
      await recurringInvoicesSetActive(s.id, false);
      flash(`„${s.label}“ pausiert — es entstehen keine Rechnungen, bis du die Abo-Vorlage fortsetzt.`, "ok", undefined, {
        action: { label: "Rückgängig", run: () => void resume({ ...s, active: 0 }) },
      });
      await load();
      // U7 (R3-04): Verschwindet die Zeile, nie den Fokus auf <body> fallen lassen.
      await tick();
      focusRowAfterRemoval(index, inactiveChk);
    } catch (e) {
      flash("Pausieren fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  // Fortsetzen: Bei verpassten Terminen verlangt der Server die Wahl „nachholen“
  // oder „ab heute fortsetzen“ (W14-026).
  async function resume(s: RecurringInvoiceRow) {
    if (busyId) return;
    let resumeMode: ResumeMode | undefined;
    if (hasBacklog(s)) {
      const choice = await askBacklogChoice("invoice", s, reviewIds.has(s.id));
      if (!choice) return;
      resumeMode = choice;
    }
    busyId = s.id;
    try {
      await recurringInvoicesSetActive(s.id, true, resumeMode);
      flash(`„${s.label}“ fortgesetzt.`);
      await load();
    } catch (e) {
      flash("Fortsetzen fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  // V1-02: Bei „Automatisch ausstellen“ bzw. „Ausstellen + senden“ schreibt „Jetzt
  // erstellen“ die Rechnung endgültig fest (und versendet sie) — Rückfrage wie beim
  // Ausstellen einer einzelnen Rechnung (design.md §5).
  async function confirmCreate(s: RecurringInvoiceRow): Promise<boolean> {
    const mode = effectiveMode(s);
    if (mode !== "issue" && mode !== "issue_send") return true;
    const send = mode === "issue_send";
    return confirmDialog({
      title: send
        ? `Rechnung für „${s.label}“ jetzt ausstellen und an ${recipient(s)} senden?`
        : `Rechnung für „${s.label}“ jetzt ausstellen?`,
      bullets: [
        `Es entsteht die Rechnung zum Termin ${date(s.nextDueDate)} mit fertiger PDF- und E-Rechnung.`,
        "Danach lässt sie sich nicht mehr ändern (gesetzlich vorgeschrieben); korrigieren geht nur über einen Storno-Beleg.",
        ...(send ? [`Die Rechnung geht sofort per E-Mail an ${recipient(s)} (Standard-Postfach).`] : []),
      ],
      confirmLabel: send ? "Ausstellen und senden" : "Ausstellen",
    });
  }

  async function createNow(s: RecurringInvoiceRow) {
    if (busyId) return;
    if (!(await confirmCreate(s))) return;
    busyId = s.id;
    try {
      const invoiceId = await recurringInvoicesRunNow(s.id);
      await flashCreated(invoiceId, s);
      await goto(`/invoices/${invoiceId}`);
    } catch (e) {
      flash("Erstellen fehlgeschlagen: " + String(e), "error");
      busyId = null;
    }
  }

  // Rückmeldung nach dem tatsächlichen Ergebnis (V1-02): Entwurf, ausgestellt oder
  // ausgestellt und gesendet — ein gescheiterter Versand bleibt sichtbar.
  async function flashCreated(invoiceId: string, s: RecurringInvoiceRow) {
    let inv: Invoice | null = null;
    try {
      inv = (await invoicesGet(invoiceId))?.invoice ?? null;
    } catch {
      inv = null;
    }
    if (!inv) {
      flash("Rechnung erstellt.");
    } else if (!inv.lockedAt) {
      flash(`Rechnung ${inv.invoiceNumber} als Entwurf angelegt — prüfen und ausstellen.`);
    } else if (inv.sentAt) {
      flash(`Rechnung ${inv.invoiceNumber} ausgestellt und an ${inv.buyerEmail ?? recipient(s)} gesendet.`);
    } else if (effectiveMode(s) === "issue_send") {
      flash(`Rechnung ${inv.invoiceNumber} ausgestellt, aber nicht gesendet — Grund unter Hinweise; „Senden“ auf der Rechnung versucht es erneut.`, "warning");
    } else {
      flash(`Rechnung ${inv.invoiceNumber} ausgestellt.`);
    }
  }

  // Rückstand einer aktiven Vorlage (Hinweis „Entscheidung nötig“, W14-026).
  async function resolveBacklog(s: RecurringInvoiceRow) {
    if (busyId) return;
    const mode = await askBacklogChoice("invoice", s, reviewIds.has(s.id));
    if (!mode) return;
    busyId = s.id;
    try {
      await recurringInvoicesResolveBacklog(s.id, mode);
      // V1-11: sagen, wann der Lauf kommt (Scheduler-Takt 5 Minuten).
      flash(
        mode === "catchUp"
          ? "Nachholen bestätigt — die offenen Termine entstehen beim nächsten automatischen Lauf, in der Regel innerhalb von 5 Minuten."
          : "Verpasste Termine ausgelassen.",
      );
      await load();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  function plural(n: number, one: string, many: string): string {
    return `${n} ${n === 1 ? one : many}`;
  }

  // V1-02: Sammellauf mit Rückfrage, sobald eine fällige Abo-Vorlage ausstellt oder versendet.
  async function confirmCreateAll(): Promise<boolean> {
    const modes = dueRows.map(effectiveMode);
    const drafts = modes.filter((m) => m !== "issue" && m !== "issue_send").length;
    const issues = modes.filter((m) => m === "issue").length;
    const sends = modes.filter((m) => m === "issue_send").length;
    if (issues + sends === 0) return true;
    return confirmDialog({
      title: `${plural(dueCount, "fällige Abo-Rechnung", "fällige Abo-Rechnungen")} jetzt erstellen?`,
      bullets: [
        ...(drafts ? [`${plural(drafts, "Abo-Vorlage legt", "Abo-Vorlagen legen")} einen Entwurf an.`] : []),
        ...(issues ? [`${plural(issues, "Abo-Vorlage stellt", "Abo-Vorlagen stellen")} die Rechnung sofort aus (festgeschrieben, nicht mehr änderbar).`] : []),
        ...(sends ? [`${plural(sends, "Abo-Vorlage stellt", "Abo-Vorlagen stellen")} die Rechnung aus und schickt sie per E-Mail an den Kunden.`] : []),
        "Bei mehreren verpassten Terminen folgt nur der jüngste der Automatik; ältere entstehen als Entwurf.",
      ],
      confirmLabel: "Erstellen",
    });
  }

  async function createAllDue() {
    if (checking) return;
    if (!(await confirmCreateAll())) return;
    checking = true;
    try {
      // Vorher/Nachher-Abgleich der Rechnungsliste (nur lesend) für eine Rückmeldung
      // nach Ergebnis; scheitert er, bleibt die Summe aus dem Bericht.
      let before: Set<string> | null = null;
      try {
        before = new Set((await invoicesList({ includeCanceled: true })).map((i) => i.id));
      } catch {
        before = null;
      }
      const r = await recurringInvoicesRunDueCheck();
      if (r.skippedLocked) {
        flash("Übersprungen: Backup ist gesperrt — zuerst entsperren.", "error");
      } else if (r.skippedHold) {
        flash("Automatische Abo-Läufe sind angehalten — siehe Hinweis oben.", "error");
      } else if (r.createdInvoices === 0 && (r.awaitingDecision ?? 0) > 0) {
        flash("Rückstand über der Nachhol-Grenze — bitte bei der Abo-Vorlage „Rückstand …“ wählen.", "error");
      } else if (r.createdInvoices === 0 && !r.failedTemplates) {
        flash("Keine fälligen Abo-Rechnungen.");
      } else {
        let text = `${plural(r.createdInvoices, "Rechnung", "Rechnungen")} erstellt.`;
        if (before) {
          try {
            const created = (await invoicesList({ includeCanceled: true })).filter((i) => !before!.has(i.id));
            const drafts = created.filter((i) => i.status === "draft").length;
            const sent = created.filter((i) => i.status === "sent").length;
            const issued = created.length - drafts - sent;
            const parts = [
              ...(drafts ? [plural(drafts, "Entwurf", "Entwürfe")] : []),
              ...(issued ? [`${issued} ausgestellt`] : []),
              ...(sent ? [`${sent} ausgestellt und gesendet`] : []),
            ];
            if (created.length > 0) text = `${plural(created.length, "Rechnung", "Rechnungen")} erstellt: ${parts.join(", ")}.`;
          } catch {
            // Aufschlüsselung ist Zusatzinformation.
          }
        }
        const failed = r.failedTemplates ?? 0;
        if (failed > 0) {
          flash(`${text} ${plural(failed, "Abo-Vorlage", "Abo-Vorlagen")} mit Fehler — siehe Hinweise.`, "warning");
        } else {
          flash(text);
        }
      }
      await load();
    } catch (e) {
      flash("Erstellen fehlgeschlagen: " + String(e), "error");
    } finally {
      checking = false;
    }
  }

  // Ergebniszeile (4.1.3): dauerhaft im DOM; beim Laden „Wird geladen …“ (V2-02).
  let resultText = $derived(
    loading
      ? "Wird geladen …"
      : error
        ? ""
        : rows.length === 0
          ? includeInactive
            ? "Noch keine Abo-Vorlagen."
            : "Keine aktiven Abo-Vorlagen."
          : `${plural(rows.length, "Abo-Vorlage", "Abo-Vorlagen")}: ${activeCount} aktiv${dueCount > 0 ? `, ${dueCount} fällig` : ""}.`,
  );
</script>

<!-- V2-15: Bereich und Titel „Abo-Rechnungen“ (Glossar), Vorlagen heißen „Abo-Vorlage“. -->
<PageBar back="/invoices" backLabel="Rechnungen" title="Abo-Rechnungen">
  {#snippet actions()}
    <Button variant="primary" href="/recurring-invoices/new">+ Neue Abo-Vorlage</Button>
  {/snippet}
</PageBar>

<p class="sub">
  Abo-Vorlagen für regelmäßige Rechnungen an Kunden — z.&nbsp;B. monatliche Server-Wartung.
</p>

<!-- V1-12: alle drei Modi erklärt, Wörter wie im Formular. -->
<ul class="legend" aria-label="Modi der Abo-Vorlagen">
  <li><Badge kind="tag">Nur Entwurf</Badge> Am Stichtag entsteht ein Rechnungs-Entwurf; du stellst ihn selbst aus.</li>
  <li><Badge kind="tag">Automatisch ausstellen</Badge> Die Rechnung wird am Stichtag festgeschrieben; du versendest selbst.</li>
  <li><Badge tone="warning">Ausstellen + senden</Badge> Die Rechnung wird festgeschrieben und per E-Mail an den Kunden gesendet.</li>
</ul>

{#if !error}
  <div class="toolbar">
    <label class="chk">
      <input type="checkbox" bind:this={inactiveChk} bind:checked={includeInactive} onchange={load} /> Pausierte anzeigen
    </label>
    {#if dueCount > 0}
      <!-- U2 (Touch ohne Hover): Erklärung sichtbar statt nur als `title`-Tooltip,
           wie bei den Kosten-Abos. -->
      <Button variant="secondary" onclick={createAllDue} ariaDisabled={checking}
        ariaDescribedby="ri-createall-hint">
        {checking ? "Erstelle …" : `${dueCount} fällige jetzt erstellen`}
      </Button>
    {/if}
  </div>
  {#if dueCount > 0}
    <p id="ri-createall-hint" class="toolbar-hint">Sonst erstellt der Hintergrund-Lauf fällige Abo-Rechnungen von selbst.</p>
  {/if}
{/if}

<p class="result-info" role="status">{resultText}</p>

{#if loading}
  <!-- Ladezustand: die Ergebniszeile darüber sagt „Wird geladen …“ an. -->
{:else if error}
  <Banner>{error}</Banner>
{:else if rows.length === 0}
  <!-- V2-09: leerer Zustand mit derselben Aktion wie die PageBar (secondary). -->
  <div class="empty">
    {#if !includeInactive}<p class="muted">Pausierte Abo-Vorlagen blendest du mit „Pausierte anzeigen“ ein.</p>{/if}
    <Button variant="secondary" href="/recurring-invoices/new">+ Neue Abo-Vorlage</Button>
  </div>
{:else}
  <!-- U3-030: Liste im Table-Baustein; je Zeile eine sichtbare Aktion + „Mehr“ (U3-036). -->
  <Table caption="Abo-Vorlagen" cards cardGrid>
    <thead>
      <tr>
        <th>Abo-Vorlage</th>
        <th>Rhythmus</th>
        <th>Nächste Fälligkeit</th>
        <th>Modus</th>
        <th class="actions-col"><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as s (s.id)}
        {@const due = dueBadge(s)}
        {@const mode = modeBadge(s)}
        {@const act = rowActions(s)}
        <tr class:paused={s.active !== 1} class:is-due={isDue(s)}>
          <td class="kb-card-head" data-label="Abo-Vorlage">
            <div class="abo-name">{s.label}</div>
            <div class="abo-meta">
              {contactInfo[s.contactId]?.name ?? "—"}
              {#if s.active !== 1}<Badge tone="neutral">Pausiert</Badge>{/if}
            </div>
          </td>
          <td class="muted-cell" data-label="Rhythmus">{rhythm(s)}</td>
          <td data-label="Nächste Fälligkeit">
            {#if s.active === 1}
              <span class="due-cell">
                {date(s.nextDueDate)}
                {#if due}<Badge tone={due.tone}>{due.label}</Badge>{/if}
              </span>
            {:else}
              <span class="muted-cell">—</span>
            {/if}
          </td>
          <td data-label="Modus"><Badge kind={mode.tone === "neutral" ? "tag" : "status"} tone={mode.tone}>{mode.label}</Badge></td>
          <td class="actions">
            <div class="kb-actions">
              <Button variant="secondary" href={act.main.href} onclick={act.main.onclick} ariaDisabled={!act.main.href && busyId === s.id} ariaLabel={act.main.ariaLabel}>{act.main.label}</Button>
              <ActionMenu variant="ghost" ariaLabel={`Mehr zu „${s.label}“`} items={act.more} />
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  .sub { margin: 0 0 0.5rem; color: var(--text-muted); font-size: var(--fs-sm); }
  .legend {
    list-style: none; font-size: var(--fs-sm); color: var(--text-muted); background: var(--surface-sunken);
    border: 1px solid var(--line); border-radius: var(--r-md); padding: 0.5rem 0.75rem; margin: 0.9rem 0;
    display: flex; flex-direction: column; gap: 0.35rem;
  }
  .legend li { display: flex; align-items: baseline; gap: 0.5rem; flex-wrap: wrap; }
  .toolbar { display: flex; align-items: center; gap: 1rem; margin: 0.5rem 0 0.25rem; flex-wrap: wrap; }
  .toolbar-hint { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 0.5rem; }
  .result-info { color: var(--text-muted); font-size: var(--fs-sm); margin: var(--sp-1) 0 var(--sp-2); min-height: 1.4em; }
  .chk { display: inline-flex; align-items: center; gap: 0.35rem; min-height: var(--target); font-size: var(--fs-sm); color: var(--text-muted); }
  .actions-col { text-align: right; }
  /* U2: nicht mitten im Wort trennen („Betriebshaftpfli/cht“), Silbentrennung (lang="de"). */
  .abo-name { font-weight: 600; color: var(--text); overflow-wrap: break-word; hyphens: auto; }
  /* UI-13: eigenes Gewicht, sonst erbt die Nebenzeile in der Kartenansicht 700 von td.kb-card-head (tokens.css). */
  .abo-meta { font-size: var(--fs-sm); font-weight: 400; color: var(--text-muted); margin-top: 0.15rem; display: flex; align-items: center; gap: 0.3rem; flex-wrap: wrap; }
  .due-cell { display: inline-flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .muted-cell { color: var(--text-muted); }
  /* Pausiert: gedämpft über die Textfarbe statt Deckkraft (Kontrast 1.4.3). */
  tr.paused td { color: var(--text-muted); background: var(--surface-sunken); }
  tr.is-due td { background: var(--status-warning-bg); }
  td.actions { text-align: right; white-space: nowrap; }
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-3); margin: 0 0 var(--sp-3); }
  .empty p { margin: 0; }
  .muted { color: var(--text-muted); }
  /* W10: In der Karten-Ansicht dürfen die Aktions-Buttons umbrechen. */
  @media screen and (max-width: 820px) {
    td.actions { white-space: normal; }
  }
</style>
