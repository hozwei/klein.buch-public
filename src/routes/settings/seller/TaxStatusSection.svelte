<script lang="ts">
  // Abschnitt „Steuerstatus“ in „Meine Firmendaten“ (ADR 0042 Nr. 1–6). Ersetzt den
  // Schalter „Kleinunternehmer nach §19 UStG“ und den §19-Wechseldialog (ADR 0041
  // Nr. 21/220/251): Historie (append-only, Widerruf statt Löschen), Status heute,
  // Gesamtumsatz nach § 19 Abs. 2 UStG mit Grenzhinweisen und der Gefahr-Dialog
  // „Statuswechsel eintragen“ mit Grund, Wirkungsdatum und Vorschau der Folgen.
  //
  // Eintrag und Widerruf verlangen eine frische Re-Auth (Registry, Transport). Der
  // Re-Auth-Dialog liegt im Root-Layout unter seitenweise eingehängten Dialogen;
  // darum schließt der Dialog vor dem Aufruf und öffnet bei einer Ablehnung des
  // Servers mit dem Grund wieder (die Eingaben bleiben).
  //
  // Baustein im Routenordner (design.md §5): `onMount` ruft nur lesende Commands.
  import { onMount, tick } from "svelte";
  import Banner from "#lib/Banner.svelte";
  import Badge from "#lib/Badge.svelte";
  import Button from "#lib/Button.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Table from "#lib/Table.svelte";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { portal } from "#lib/portal.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { date, euro } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { REAUTH_CANCELED } from "#lib/transport.ts";
  import {
    taxStatusOverview,
    taxStatusChangePreview,
    taxStatusChangeAdd,
    taxStatusChangeRevoke,
  } from "#lib/api.ts";
  import type {
    TaxStatus,
    TaxStatusChange,
    TaxStatusChangeInput,
    TaxStatusChangePreview,
    TaxStatusOverview,
    TaxStatusReason,
    TaxThresholdHint,
  } from "#lib/types.ts";
  import {
    REASONS,
    TAX_STATUS_LABEL,
    activeEntries,
    bindingEnd,
    bindingUntilFor,
    changeConfirmLabel,
    choosableReasons,
    consequenceLines,
    effectiveFromError,
    effectiveFromLabel,
    nextChangeAfter,
    statusAt,
    suggestChange,
    thresholdHintText,
  } from "#lib/taxStatus.ts";

  /** `onStatus`: Status heute nach jedem Laden (für die Zeile in „Steuer“). */
  let { onStatus }: { onStatus?: (status: TaxStatus) => void } = $props();

  let overview = $state<TaxStatusOverview | null>(null);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  async function load() {
    loading = overview === null;
    loadError = null;
    try {
      overview = await taxStatusOverview();
      onStatus?.(overview.currentStatus);
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  const entriesNewestFirst = $derived(
    overview ? [...overview.entries].sort((a, b) => (a.effectiveFrom < b.effectiveFrom ? 1 : a.effectiveFrom > b.effectiveFrom ? -1 : 0)) : [],
  );
  const reasons = $derived(overview ? choosableReasons(overview.entries) : []);
  const upcoming = $derived(overview ? nextChangeAfter(overview.entries, overview.today) : null);
  const hasEntries = $derived(!!overview && activeEntries(overview.entries).length > 0);
  /** Grenzhinweise mit Vorschlag, der als nächster Eintrag möglich ist. */
  function hintActionable(h: TaxThresholdHint): boolean {
    return h.exceeded && reasons.includes(h.suggestedReason);
  }

  // ---- Dialog „Statuswechsel eintragen“ ----------------------------------------
  let showChange = $state(false);
  let reason = $state<TaxStatusReason | null>(null);
  let effectiveFrom = $state("");
  let triggerDocument = $state("");
  let note = $state("");
  let tried = $state(false);
  let serverError = $state<string | null>(null);
  let preview = $state<TaxStatusChangePreview | null>(null);
  let previewFor = $state<string | null>(null);
  let previewBusy = $state(false);
  let previewError = $state<string | null>(null);
  let saving = $state(false);
  let previewSeq = 0;

  $effect(() => {
    if (showChange || showRevoke) {
      pushModal();
      return () => popModal();
    }
  });

  const info = $derived(reason ? REASONS[reason] : null);
  const reasonError = $derived(tried && !reason ? "Bitte einen Grund wählen." : "");
  const dateError = $derived(reason && overview ? effectiveFromError(reason, effectiveFrom, overview.today) : "");
  const shownDateError = $derived(tried || effectiveFrom ? dateError : "");
  const plannedBinding = $derived(reason ? bindingUntilFor(reason, effectiveFrom) : null);

  function input(): TaxStatusChangeInput | null {
    if (!reason) return null;
    const status = REASONS[reason].status ?? "regelbesteuerung";
    return {
      effectiveFrom: reason === "initial" ? null : effectiveFrom,
      status,
      reason,
      triggerDocument: REASONS[reason].trigger ? triggerDocument.trim() || null : null,
      note: note.trim() || null,
    };
  }
  // Vorschau hängt nur an Grund, Status und Datum (nicht an Beleg und Notiz).
  const inputKey = $derived(
    JSON.stringify(
      reason && !dateError
        ? { effectiveFrom: reason === "initial" ? null : effectiveFrom, status: REASONS[reason].status ?? "regelbesteuerung", reason }
        : null,
    ),
  );

  // Vorschau der Folgen (schreibt nichts), kurz verzögert nach der letzten Eingabe.
  $effect(() => {
    const key = inputKey;
    if (!showChange || key === "null") {
      preview = null;
      previewFor = null;
      return;
    }
    const seq = ++previewSeq;
    previewBusy = true;
    const t = setTimeout(async () => {
      try {
        const p = await taxStatusChangePreview(JSON.parse(key) as TaxStatusChangeInput);
        if (seq !== previewSeq) return;
        preview = p;
        previewFor = key;
        previewError = null;
      } catch (e) {
        if (seq !== previewSeq) return;
        preview = null;
        previewError = String(e);
      } finally {
        if (seq === previewSeq) previewBusy = false;
      }
    }, 250);
    return () => clearTimeout(t);
  });
  const previewCurrent = $derived(preview && previewFor === inputKey ? preview : null);
  const previewLines = $derived(previewCurrent?.ok ? consequenceLines(previewCurrent.consequences) : []);

  function openChange(prefill?: TaxThresholdHint) {
    if (!overview) return;
    reason = prefill ? prefill.suggestedReason : reasons.length === 1 ? reasons[0] : null;
    effectiveFrom = "";
    triggerDocument = "";
    note = "";
    if (reason) applySuggestion(reason);
    if (prefill?.suggestedEffectiveFrom) effectiveFrom = prefill.suggestedEffectiveFrom;
    if (prefill?.triggerDocument) triggerDocument = prefill.triggerDocument;
    tried = false;
    serverError = null;
    preview = null;
    previewError = null;
    showChange = true;
  }

  function applySuggestion(r: TaxStatusReason) {
    if (!overview) return;
    const s = suggestChange(r, {
      today: overview.today,
      entries: overview.entries,
      earliestReturn: overview.earliestReturn,
      hints: overview.thresholdHints,
    });
    effectiveFrom = s.effectiveFrom;
    triggerDocument = s.triggerDocument;
  }

  function chooseReason(r: TaxStatusReason) {
    reason = r;
    serverError = null;
    applySuggestion(r);
  }

  function closeChange() {
    showChange = false;
    previewSeq++;
  }

  async function confirmChange() {
    if (saving) return;
    tried = true;
    await tick();
    if (!reason) {
      document.querySelector<HTMLInputElement>('input[name="ts-reason"]')?.focus();
      return;
    }
    if (dateError) {
      document.getElementById("ts-date")?.focus();
      return;
    }
    if (!previewCurrent?.ok) {
      // Vorschau läuft noch oder lehnt ab: Gründe stehen im Dialog.
      document.getElementById("ts-preview")?.focus();
      return;
    }
    const payload = input();
    if (!payload) return;
    saving = true;
    showChange = false;
    try {
      const res = await taxStatusChangeAdd(payload);
      const lines = consequenceLines(res.consequences);
      const what = `${REASONS[res.entry.reason].label}: ${effectiveFromLabel(res.entry.effectiveFrom)}`;
      if (lines.length > 0) {
        flash(`Steuerstatus eingetragen (${what}). Bitte prüfen — Einzelheiten unter Hinweise: ${lines.join(" ")}`, "warning");
      } else {
        flash(`Steuerstatus eingetragen (${what}).`);
      }
      await load();
    } catch (e) {
      if (e === REAUTH_CANCELED) {
        flash(String(e), "info");
      } else {
        serverError = String(e);
        showChange = true;
      }
    } finally {
      saving = false;
    }
  }

  // ---- Widerruf -----------------------------------------------------------------
  let showRevoke = $state(false);
  let revokeEntry = $state<TaxStatusChange | null>(null);
  let revokeReason = $state("");
  let revokeTried = $state(false);
  let revokeServerError = $state<string | null>(null);
  let revoking = $state(false);
  const revokeReasonError = $derived(revokeTried && !revokeReason.trim() ? "Bitte einen Grund für den Widerruf angeben." : "");
  /** Status, der am Wirkungsdatum des widerrufenen Eintrags wieder gilt. */
  const statusAfterRevoke = $derived(
    overview && revokeEntry
      ? statusAt(
          overview.entries.filter((e) => e.id !== revokeEntry!.id),
          revokeEntry.effectiveFrom,
        )
      : null,
  );

  function openRevoke(e: TaxStatusChange) {
    revokeEntry = e;
    revokeReason = "";
    revokeTried = false;
    revokeServerError = null;
    showRevoke = true;
  }

  function closeRevoke() {
    showRevoke = false;
  }

  async function confirmRevoke() {
    if (revoking || !revokeEntry) return;
    revokeTried = true;
    if (!revokeReason.trim()) {
      document.getElementById("ts-revoke-reason")?.focus();
      return;
    }
    revoking = true;
    showRevoke = false;
    try {
      const res = await taxStatusChangeRevoke(revokeEntry.id, revokeReason.trim());
      const lines = consequenceLines(res.consequences);
      if (lines.length > 0) {
        flash(`Eintrag widerrufen. Bitte prüfen — Einzelheiten unter Hinweise: ${lines.join(" ")}`, "warning");
      } else {
        flash("Eintrag widerrufen.");
      }
      await load();
    } catch (e) {
      if (e === REAUTH_CANCELED) {
        flash(String(e), "info");
      } else {
        revokeServerError = String(e);
        showRevoke = true;
      }
    } finally {
      revoking = false;
    }
  }
</script>

<section class="card tax-status" aria-labelledby="ts-h" id="steuerstatus">
  <div class="ts-head">
    <h2 id="ts-h">Steuerstatus</h2>
    <HelpAnchor slug="kleinunternehmer-regelung" />
  </div>

  {#if loading}
    <LoadingState />
  {:else if loadError}
    <Banner title="Der Steuerstatus konnte nicht geladen werden.">{loadError}
      {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
    </Banner>
  {:else if overview}
    <!-- Status heute -->
    <p class="now">
      Heute ({date(overview.today)}): <strong>{TAX_STATUS_LABEL[overview.currentStatus]}</strong>
    </p>
    {#if overview.currentStatus === "kleinunternehmer"}
      <p class="kb-muted">
        Auf jeder Rechnung erscheint der Hinweis „{overview.hinweisText}“. Du weist keine
        Umsatzsteuer aus; Rechnungen und Angebote blenden die USt-Felder aus.
        {#if !hasEntries}Das ist der Standard, solange kein Wechsel eingetragen ist.{/if}
      </p>
    {:else}
      <p class="kb-muted">
        Du weist auf deinen Rechnungen Umsatzsteuer aus und meldest sie ans Finanzamt.
        {#if overview.bindingUntil}
          Durch den Verzicht bist du bis <strong>{date(bindingEnd(overview.bindingUntil))}</strong>
          an die Regelbesteuerung gebunden.
        {/if}
        {#if overview.earliestReturn}
          Zurück zur Kleinunternehmer-Regelung frühestens zum <strong>{date(overview.earliestReturn)}</strong>.
        {/if}
      </p>
    {/if}
    {#if upcoming}
      <p class="kb-muted">
        Eingetragen: ab <strong>{date(upcoming.effectiveFrom)}</strong>
        {TAX_STATUS_LABEL[upcoming.status]} ({REASONS[upcoming.reason].label}).
      </p>
    {/if}
    <p class="kb-muted">
      Klein.Buch rechnet jeden Beleg mit dem Steuerstatus seines Datums: Rechnungen nach dem
      Leistungsdatum (sonst Rechnungsdatum), Angebote nach dem Angebotsdatum, Kosten nach dem
      Belegdatum. Beim Festschreiben bleibt der Status einer Rechnung unverändert.
    </p>

    <!-- Grenzhinweise (ADR 0042 Nr. 5): Klein.Buch wechselt nie selbst. -->
    {#each overview.thresholdHints as h (h.kind + h.year)}
      <Banner kind={h.exceeded ? "warning" : "info"}
        title={h.exceeded ? "Umsatzgrenze überschritten." : "Umsatzgrenze erreicht."}>
        {thresholdHintText(h)} Den Wechsel trägst du selbst ein; prüfe ihn mit deiner Steuerberatung.
        {#snippet actions()}
          {#if hintActionable(h)}
            <Button variant="secondary" onclick={() => openChange(h)}>Wegfall eintragen …</Button>
          {/if}
        {/snippet}
      </Banner>
    {/each}

    <!-- Gesamtumsatz nach vereinnahmten Entgelten (§ 19 Abs. 2 UStG) -->
    <h3 class="sub">Gesamtumsatz</h3>
    <dl class="kb-ledger turnover">
      <div>
        <dt>{overview.turnover.previousYear} (Grenze 25.000 €)</dt>
        <dd>{euro(overview.turnover.previousYearCents)}</dd>
      </div>
      <div>
        <dt>{overview.turnover.currentYear} bis heute (Grenze 100.000 €)</dt>
        <dd>{euro(overview.turnover.currentYearCents)}</dd>
      </div>
    </dl>
    <p class="kb-muted small">
      Nach vereinnahmten Entgelten, netto, Erstattungen aus Stornos abgezogen; ohne Verkäufe von
      Anlagen (§ 19 Abs. 2 UStG). Klein.Buch kennt nur die hier erfassten Zahlungen; Umsätze
      außerhalb von Klein.Buch rechnest du selbst hinzu.
    </p>

    <!-- Historie -->
    <h3 class="sub" id="ts-hist-h">Verlauf</h3>
    {#if entriesNewestFirst.length === 0}
      <p class="kb-muted">
        Noch kein Eintrag: Es gilt die Kleinunternehmer-Regelung. Warst du schon immer
        regelbesteuert, trag das als „Stand zu Beginn“ ein.
      </p>
    {:else}
      <Table caption="Verlauf des Steuerstatus" cards>
        <thead>
          <tr>
            <th>Wirkung ab</th>
            <th>Status</th>
            <th>Grund</th>
            <th>Bindung bis</th>
            <th><span class="kb-visually-hidden">Aktion</span></th>
          </tr>
        </thead>
        <tbody>
          {#each entriesNewestFirst as e (e.id)}
            <tr class:revoked={!!e.revokedAt}>
              <td data-label="Wirkung ab" class="kb-nowrap">{effectiveFromLabel(e.effectiveFrom)}</td>
              <td data-label="Status">
                {TAX_STATUS_LABEL[e.status]}
                {#if e.revokedAt}<Badge strike>Widerrufen</Badge>{/if}
              </td>
              <td data-label="Grund">
                {REASONS[e.reason]?.label ?? e.reason}
                {#if e.triggerDocument}<br /><small>Auslösender Beleg: {e.triggerDocument}</small>{/if}
                {#if e.note}<br /><small>Notiz: {e.note}</small>{/if}
                {#if e.revokedAt}
                  <br /><small>Widerrufen am {date(e.revokedAt)}{e.revokeReason ? `: ${e.revokeReason}` : ""}</small>
                {/if}
              </td>
              <td data-label="Bindung bis" class="kb-nowrap">
                {#if e.bindingUntil}
                  {date(bindingEnd(e.bindingUntil))}<br /><small>Rückkehr ab {date(e.bindingUntil)}</small>
                {:else}—{/if}
              </td>
              <td data-label="Aktion">
                {#if e.id === overview.revocableId && !e.revokedAt}
                  <Button variant="secondary" onclick={() => openRevoke(e)}
                    ariaLabel={`Widerrufen: Eintrag ab ${effectiveFromLabel(e.effectiveFrom)}`}>Widerrufen …</Button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </Table>
      <p class="kb-muted small">
        Einträge werden nie gelöscht oder geändert. Einen irrtümlichen Eintrag widerrufst du mit
        Grund; widerrufen lässt sich nur der jüngste.
      </p>
    {/if}

    <div class="kb-actions kb-start">
      <Button variant="secondary" onclick={() => openChange()} ariaDisabled={saving}>Statuswechsel eintragen …</Button>
    </div>
  {/if}
</section>

{#if showChange && overview}
  <!-- Gefahr-Dialog (ADR 0041 Nr. 11, ADR 0042 Nr. 1–4): kein Schließen über den
       Hintergrund, Startfokus „Abbrechen“, Escape bricht ab. Löst ADR 0041 Nr. 21/220/251 ab. -->
  <div class="kb-backdrop" use:portal>
    <div
      class="kb-dialog ts-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="ts-title"
      aria-describedby="ts-lead"
      use:focusTrap={{ onEscape: closeChange }}
    >
      <header class="kb-dialog-head">
        <h2 id="ts-title">Statuswechsel eintragen?</h2>
      </header>
      <div class="kb-dialog-body">
        <p id="ts-lead">
          Der Eintrag ändert, ob deine Belege ab dem Wirkungsdatum Umsatzsteuer ausweisen. Er bleibt
          dauerhaft im Verlauf; einen Irrtum widerrufst du mit Grund. Die Hinweise ersetzen keine
          Steuerberatung.
        </p>

        {#if serverError}
          <Banner title="Nicht eingetragen.">{serverError}</Banner>
        {/if}

        <fieldset class="reasons" aria-describedby={reasonError ? "ts-reason-err" : undefined}>
          <legend>Grund<span class="req" aria-hidden="true">*</span><span class="kb-visually-hidden">(Pflicht)</span></legend>
          {#if reasonError}<p id="ts-reason-err" class="err">{reasonError}</p>{/if}
          {#each reasons as r (r)}
            <label class="reason">
              <input type="radio" name="ts-reason" value={r} checked={reason === r}
                onchange={() => chooseReason(r)} aria-describedby="ts-reason-{r}" />
              <span>
                <strong>{REASONS[r].label}</strong>
                <small id="ts-reason-{r}">{REASONS[r].explain}</small>
              </span>
            </label>
          {/each}
        </fieldset>

        {#if reason && info && reason !== "initial"}
          <div class="f">
            <label for="ts-date">Wirkungsdatum<span class="req" aria-hidden="true">*</span></label>
            <input id="ts-date" class="kb-input kb-w-date" type="date" bind:value={effectiveFrom}
              required aria-required="true"
              max={reason === "wegfall_laufend" || reason === "wegfall_gruendungsjahr" ? overview.today : undefined}
              aria-invalid={shownDateError ? "true" : undefined}
              aria-describedby={shownDateError ? "ts-date-err ts-date-hint" : "ts-date-hint"} />
            {#if shownDateError}<span id="ts-date-err" class="err">{shownDateError}</span>{/if}
            <small id="ts-date-hint" class="hint">
              {#if info.janFirst}
                Nur zum 1. Januar eines Jahres (§ 19 Abs. 1 und 3 UStG).
              {:else}
                Tag des Umsatzes, mit dem die Grenze überschritten wird (Leistungsdatum der Rechnung);
                dieser Umsatz ist schon steuerpflichtig.
              {/if}
              Rückwirkend nur in einem offenen Geschäftsjahr.
            </small>
          </div>
          {#if info.trigger}
            <div class="f">
              <label for="ts-trigger">Beleg, der die Grenze überschreitet</label>
              <input id="ts-trigger" class="kb-input kb-w-medium" type="text" bind:value={triggerDocument}
                maxlength="100" autocomplete="off" spellcheck="false" aria-describedby="ts-trigger-hint" />
              <small id="ts-trigger-hint" class="hint">Rechnungsnummer, z. B. RE-2026-0042. Freiwillig, zur Nachvollziehbarkeit.</small>
            </div>
          {/if}
        {/if}

        {#if reason}
          <div class="f">
            <label for="ts-note">Notiz</label>
            <textarea id="ts-note" class="kb-input" rows="2" maxlength="2000" bind:value={note}
              aria-describedby="ts-note-hint"></textarea>
            <small id="ts-note-hint" class="hint">Freiwillig, z. B. Datum der Erklärung ans Finanzamt. Ohne Namen von Personen.</small>
          </div>
        {/if}

        {#if plannedBinding}
          <Banner kind="warning" title="Fünf Jahre Bindung.">
            Mit dem Verzicht bist du bis <strong>{date(bindingEnd(plannedBinding))}</strong> an die
            Regelbesteuerung gebunden (fünf Kalenderjahre einschließlich des Wirkungsjahres). Zurück zur
            Kleinunternehmer-Regelung kannst du frühestens zum <strong>{date(plannedBinding)}</strong>.
          </Banner>
        {/if}

        <!-- Vorschau der Folgen (Server, schreibt nichts). -->
        <div id="ts-preview" class="preview" tabindex="-1" aria-live="polite">
          {#if reason && !dateError}
            {#if previewError}
              <Banner kind="error" title="Die Vorschau ist fehlgeschlagen.">{previewError}</Banner>
            {:else if previewCurrent && !previewCurrent.ok}
              <Banner kind="error" title="So lässt sich der Wechsel nicht eintragen.">
                {#if previewCurrent.errors.length === 1}
                  {previewCurrent.errors[0]}
                {:else}
                  <ul>{#each previewCurrent.errors as m (m)}<li>{m}</li>{/each}</ul>
                {/if}
              </Banner>
            {:else if previewCurrent}
              {#if previewCurrent.warnings.length > 0}
                <!-- V1-R07: Warnungen sperren nicht (Erklärungsfrist, Vorjahresumsatz). -->
                <Banner kind="warning" title="Bitte prüfen.">
                  {#if previewCurrent.warnings.length === 1}
                    {previewCurrent.warnings[0]}
                  {:else}
                    <ul>{#each previewCurrent.warnings as w (w)}<li>{w}</li>{/each}</ul>
                  {/if}
                </Banner>
              {/if}
              {#if previewLines.length > 0}
                <Banner kind="warning" title="Folgen des Eintrags.">
                  <ul>{#each previewLines as l (l)}<li>{l}</li>{/each}</ul>
                </Banner>
              {:else}
                <p class="kb-muted">Keine festgeschriebenen Belege, Entwürfe oder Abo-Vorlagen betroffen.</p>
              {/if}
            {:else if previewBusy}
              <p class="kb-muted" role="status">Folgen werden geprüft …</p>
            {/if}
          {/if}
        </div>

        {#if reason && REASONS[reason].status === "regelbesteuerung"}
          <h3>Nach dem Wechsel</h3>
          <ul>
            <li>Du weist auf Rechnungen ab dem Wirkungsdatum Umsatzsteuer aus,</li>
            <li>schickst Umsatzsteuer-Voranmeldungen ans Finanzamt</li>
            <li>und ziehst Vorsteuer aus Kosten ab dem Wirkungsdatum ab.</li>
          </ul>
          <p>
            Nutzt du Anlagen weiter, die du als Kleinunternehmer angeschafft hast, kann der Wechsel
            eine Berichtigung der Vorsteuer nach § 15a UStG auslösen. Klein.Buch berechnet sie
            nicht; kläre das mit deiner Steuerberatung. Die Umsatzsteuer-Voranmeldung, Zahlungen an
            und Erstattungen vom Finanzamt sowie die Umsatzsteuer auf Anlagenverkäufe bildet
            Klein.Buch nicht ab.
          </p>
        {:else if reason === "rueckkehr"}
          <p>
            Ab dem Wirkungsdatum weisen neue Rechnungen keine Umsatzsteuer aus. Auch die Rückkehr
            kann bei weiter genutzten Anlagen eine Berichtigung nach § 15a UStG auslösen; kläre das
            mit deiner Steuerberatung.
          </p>
        {/if}
        <p>
          Mehr im Handbuch, Kapitel
          <a class="kb-link" href="/help/kleinunternehmer-regelung">Kleinunternehmer-Regelung</a>.
        </p>
      </div>
      <footer class="kb-dialog-foot">
        <Button variant="secondary" initialFocus onclick={closeChange}>Abbrechen</Button>
        <Button variant="danger" onclick={confirmChange} ariaDisabled={saving}>{changeConfirmLabel(reason)}</Button>
      </footer>
    </div>
  </div>
{/if}

{#if showRevoke && revokeEntry}
  <div class="kb-backdrop" use:portal>
    <div
      class="kb-dialog ts-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="ts-revoke-title"
      aria-describedby="ts-revoke-lead"
      use:focusTrap={{ onEscape: closeRevoke }}
    >
      <header class="kb-dialog-head">
        <h2 id="ts-revoke-title">Eintrag widerrufen?</h2>
      </header>
      <div class="kb-dialog-body">
        <p id="ts-revoke-lead">
          Der Eintrag „{REASONS[revokeEntry.reason]?.label ?? revokeEntry.reason}“ ab
          {effectiveFromLabel(revokeEntry.effectiveFrom)} gilt danach nicht mehr; er bleibt mit dem
          Grund im Verlauf sichtbar.
          {#if statusAfterRevoke}
            Ab {effectiveFromLabel(revokeEntry.effectiveFrom)} gilt dann wieder:
            <strong>{TAX_STATUS_LABEL[statusAfterRevoke]}</strong>.
          {/if}
          Festgeschriebene Rechnungen ändern sich nicht; betroffene nennt Klein.Buch unter Hinweise.
        </p>
        {#if revokeServerError}
          <Banner title="Nicht widerrufen.">{revokeServerError}</Banner>
        {/if}
        <div class="f">
          <label for="ts-revoke-reason">Grund<span class="req" aria-hidden="true">*</span></label>
          <textarea id="ts-revoke-reason" class="kb-input" rows="3" maxlength="2000" bind:value={revokeReason}
            required aria-required="true"
            aria-invalid={revokeReasonError ? "true" : undefined}
            aria-describedby={revokeReasonError ? "ts-revoke-err ts-revoke-hint" : "ts-revoke-hint"}></textarea>
          {#if revokeReasonError}<span id="ts-revoke-err" class="err">{revokeReasonError}</span>{/if}
          <small id="ts-revoke-hint" class="hint">Zum Beispiel: falsches Wirkungsdatum eingetragen. Ohne Namen von Personen.</small>
        </div>
      </div>
      <footer class="kb-dialog-foot">
        <Button variant="secondary" initialFocus onclick={closeRevoke}>Abbrechen</Button>
        <Button variant="danger" onclick={confirmRevoke} ariaDisabled={revoking}>Eintrag widerrufen</Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .tax-status { max-width: 64rem; margin-top: 1rem; }
  .ts-head { display: flex; align-items: center; gap: 0.25rem; }
  .ts-head h2 { margin: 0; }
  .now { margin: 0.75rem 0 0.25rem; }
  .sub { font-size: var(--fs-md); margin: 1.25rem 0 0.5rem; }
  .turnover { max-width: 32rem; }
  .small { font-size: var(--fs-sm); }
  tr.revoked td { color: var(--text-muted); }
  .kb-actions { margin-top: 1rem; }
  .ts-dialog { --dialog-w: 40rem; }
  .ts-dialog h3 { font-size: var(--fs-md); margin: 1rem 0 0.25rem; }
  .ts-dialog ul { padding-left: 1.2rem; }
  .ts-dialog li { margin-bottom: 0.3rem; }
  .ts-dialog a { text-decoration: underline; }
  .reasons { border: 0; padding: 0; margin: 1rem 0; min-width: 0; }
  .reasons legend { font-weight: 600; padding: 0; margin-bottom: 0.4rem; }
  .reason {
    display: flex; gap: 0.6rem; align-items: flex-start;
    padding: 0.5rem 0; min-height: var(--target); cursor: pointer;
    border-bottom: 1px solid var(--line);
  }
  .reason input { margin-top: 0.25rem; min-width: 1.1rem; min-height: 1.1rem; }
  .reason span { display: flex; flex-direction: column; gap: 0.15rem; }
  .reason small { color: var(--text-muted); font-size: var(--fs-sm); }
  .f { display: flex; flex-direction: column; gap: 0.25rem; margin-bottom: 0.75rem; min-width: 0; }
  .f > label { font-size: var(--fs-sm); font-weight: 600; }
  .f input, .f textarea { min-height: var(--target); font: inherit; padding: 0.45rem; box-sizing: border-box; }
  .f textarea { width: 100%; }
  .hint { display: block; font-size: var(--fs-sm); color: var(--text-muted); }
  .err { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--status-danger-fg); margin: 0; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .preview:focus { outline: none; }
</style>
