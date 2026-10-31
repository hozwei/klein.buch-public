<script lang="ts">
  // Leistungszeitpunkt einer Rechnung (ADR 0042 Nr. 10/11): Wahl zwischen
  // Leistungsdatum und Leistungszeitraum als Radiogruppe (fieldset/legend
  // „Leistung“, Optionen „Datum“ und „Zeitraum (von–bis)“ — so trägt nur das
  // Datumsfeld den Namen „Leistungsdatum“; Pfeiltasten wie jede native
  // Radiogruppe), Meldungen am Feld (ADR 0041). Kein `max` am Datum: eine
  // Rechnung vor der Leistung (Vorausrechnung) ist zulässig und bekommt nur
  // einen Hinweis.
  //
  // Verwendung (Rechnungs-Editor, Angebotsumwandlung):
  //   <ServiceDateFields bind:mode bind:deliveryDate bind:periodStart bind:periodEnd
  //     {invoiceDate} showErrors={submitted} ondeliveryinput={() => (touched = true)} />
  import { advanceInvoiceHint, serviceDateErrors, type ServiceDateMode } from "#lib/serviceDate.ts";

  let {
    mode = $bindable<ServiceDateMode>("date"),
    deliveryDate = $bindable(""),
    periodStart = $bindable(""),
    periodEnd = $bindable(""),
    invoiceDate = "",
    showErrors = false,
    idPrefix = "svc",
    ondeliveryinput,
  }: {
    mode?: ServiceDateMode;
    deliveryDate?: string;
    periodStart?: string;
    periodEnd?: string;
    invoiceDate?: string;
    showErrors?: boolean;
    idPrefix?: string;
    ondeliveryinput?: () => void;
  } = $props();

  const errs = $derived(serviceDateErrors(mode, deliveryDate, periodStart, periodEnd));
  const hint = $derived(advanceInvoiceHint(mode, deliveryDate, periodEnd, invoiceDate));
  const errDelivery = $derived(showErrors ? errs.delivery : "");
  const errStart = $derived(showErrors ? errs.start : "");
  // Ende vor Beginn sofort melden (eindeutiger Fehler), fehlendes Ende erst nach dem Speichern.
  const errEnd = $derived(
    showErrors || (periodStart && periodEnd) ? errs.end : "",
  );

  // Beim Wechsel zum Zeitraum das bisherige Leistungsdatum als Beginn anbieten.
  function chooseMode(next: ServiceDateMode) {
    if (next === "period" && !periodStart && deliveryDate) periodStart = deliveryDate;
    mode = next;
  }

  function describedBy(...ids: (string | false)[]): string | undefined {
    const list = ids.filter(Boolean).join(" ");
    return list || undefined;
  }
</script>

<fieldset class="svc">
  <legend>Leistung <span class="req" aria-hidden="true">*</span></legend>
  <div class="svc-modes">
    <label class="svc-mode" class:sel={mode === "date"}>
      <input type="radio" name="{idPrefix}-mode" value="date" checked={mode === "date"}
        onchange={() => chooseMode("date")} />
      <span>Datum</span>
    </label>
    <label class="svc-mode" class:sel={mode === "period"}>
      <input type="radio" name="{idPrefix}-mode" value="period" checked={mode === "period"}
        onchange={() => chooseMode("period")} />
      <span>Zeitraum (von–bis)</span>
    </label>
  </div>

  <!-- UI-01: Das Label trägt nur den Feldnamen; die Meldung steht daneben (design.md §4.7). -->
  {#if mode === "date"}
    <div class="svc-fields">
      <div class="svc-field">
        <label for="{idPrefix}-date">Leistungsdatum <span class="req" aria-hidden="true">*</span></label>
        <input
          id="{idPrefix}-date"
          type="date"
          class="kb-w-date"
          bind:value={deliveryDate}
          oninput={() => ondeliveryinput?.()}
          required
          aria-required="true"
          aria-invalid={errDelivery ? "true" : undefined}
          aria-describedby={describedBy(!!errDelivery && `${idPrefix}-date-err`, `${idPrefix}-hint`)}
        />
        {#if errDelivery}<span id="{idPrefix}-date-err" class="field-err">{errDelivery}</span>{/if}
      </div>
    </div>
  {:else}
    <div class="svc-fields">
      <div class="svc-field">
        <label for="{idPrefix}-start">Beginn <span class="req" aria-hidden="true">*</span></label>
        <input
          id="{idPrefix}-start"
          type="date"
          class="kb-w-date"
          bind:value={periodStart}
          required
          aria-required="true"
          aria-invalid={errStart ? "true" : undefined}
          aria-describedby={describedBy(!!errStart && `${idPrefix}-start-err`, `${idPrefix}-hint`)}
        />
        {#if errStart}<span id="{idPrefix}-start-err" class="field-err">{errStart}</span>{/if}
      </div>
      <div class="svc-field">
        <label for="{idPrefix}-end">Ende <span class="req" aria-hidden="true">*</span></label>
        <input
          id="{idPrefix}-end"
          type="date"
          class="kb-w-date"
          bind:value={periodEnd}
          min={periodStart || undefined}
          required
          aria-required="true"
          aria-invalid={errEnd ? "true" : undefined}
          aria-describedby={describedBy(!!errEnd && `${idPrefix}-end-err`, `${idPrefix}-hint`)}
        />
        {#if errEnd}<span id="{idPrefix}-end-err" class="field-err">{errEnd}</span>{/if}
      </div>
    </div>
  {/if}
  <!-- Hinweis bleibt bei einem Fehler stehen (design.md §4.7); bei einer Vorausrechnung der Hinweis darauf. -->
  <p id="{idPrefix}-hint" class="field-hint" class:advance={!!hint}>
    {hint ||
      (mode === "date"
        ? "Tag der Lieferung oder Leistung."
        : "Für Leistungen über einen Zeitraum, z. B. einen Monat; auf der Rechnung steht „Leistungszeitraum: TT.MM.JJJJ – TT.MM.JJJJ“.")}
  </p>
</fieldset>

<style>
  .svc { border: 0; margin: 0; padding: 0; min-width: 0; }
  .svc legend { padding: 0; margin-bottom: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .req { color: var(--status-danger-fg); }
  .svc-modes { display: flex; flex-wrap: wrap; gap: var(--sp-2); margin-bottom: var(--sp-2); }
  /* Trefferfläche: das ganze Label, mindestens --target hoch (U1, 2.5.8). */
  .svc-mode {
    display: inline-flex; flex-direction: row; align-items: center; gap: var(--sp-2);
    min-height: var(--target); padding: 0 var(--sp-3);
    border: 1px solid var(--line-control); border-radius: var(--r-sm);
    font-size: var(--fs-sm); font-weight: 400; color: var(--text); cursor: pointer;
  }
  .svc-mode.sel { border-color: var(--accent); background: var(--accent-soft); }
  .svc-mode:focus-within { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); }
  .svc-mode input { width: auto; margin: 0; padding: 0; flex: none; }
  .svc-fields { display: flex; flex-wrap: wrap; gap: 0.75rem; }
  .svc-field { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .svc-fields input {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text); background: var(--surface-card);
  }
  .field-err { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { display: block; margin: 4px 0 0; font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  /* Vorausrechnung: Hinweis in Info-Farbe (kein Fehler, ADR 0042 Nr. 11). */
  .field-hint.advance { color: var(--status-info-fg); font-weight: 600; }
</style>
