<script lang="ts">
  // Betragseingabe (Lauf 2026.11, U1; Plan §4.2, ADR 0041).
  //
  // - `type="text"` + `inputmode="decimal"`: kein `type="number"` (kein
  //   Scrollrad, keine Browser-Auslegung von Komma/Punkt), Geld bleibt
  //   Integer-Cent über `parseMoneyToCents` (eine geprüfte Stelle).
  // - Gebunden wird `cents` (`number | null`). Bei jeder Eingabe wird neu
  //   gelesen: gültig → Cent, leer oder ungültig → `null`.
  // - Fehler erscheinen erst beim Verlassen des Feldes, beim Absenden
  //   (`validate()` über `bind:this`, oder Prop `showErrors`) oder wenn der
  //   Browser das Feld bei der Formularprüfung als ungültig meldet
  //   (`setCustomValidity`). Eine sichtbare Meldung verschwindet, sobald die
  //   Eingabe gültig ist; neue Meldungen kommen nicht bei jedem Tastendruck.
  // - Beim Verlassen steht der erkannte Wert im DIN-Format im Feld
  //   (`12.5` → `12,50`, `1.234` → `1.234,00`): die Auslegung ist sichtbar.
  // - Ändert der Aufrufer `cents` (z. B. Brutto aus Netto), zeigt das Feld
  //   den neuen Wert, solange es nicht fokussiert ist.
  // - `allowNegative`: `inputmode="text"`, weil die iOS-Dezimaltastatur kein
  //   Minus hat (ADR 0041; Stand U1 lässt kein Eingabefeld der Domain negative
  //   Beträge zu, Storno-Beträge erzeugt der Server).
  import { untrack } from "svelte";
  import { randomId } from "#lib/browser.ts";
  import { formatCents, MAX_AMOUNT_CENTS, parseMoneyToCents } from "#lib/money.ts";

  let {
    cents = $bindable<number | null>(null),
    label,
    ariaLabel,
    id,
    name,
    hint,
    error,
    placeholder,
    required = false,
    disabled = false,
    readonly = false,
    allowNegative = false,
    maxAbsCents = MAX_AMOUNT_CENTS,
    showErrors = false,
    enterkeyhint,
    onchange,
    class: className = "",
  }: {
    /** Betrag in Cent; `null` = leer oder ungültig. */
    cents?: number | null;
    /** Sichtbares Label; ohne Label `ariaLabel` oder ein externes `<label for={id}>`. */
    label?: string;
    ariaLabel?: string;
    id?: string;
    name?: string;
    hint?: string;
    /** Fehler von außen (z. B. Server-Meldung); hat Vorrang vor der Lesefehler-Meldung. */
    error?: string;
    placeholder?: string;
    required?: boolean;
    disabled?: boolean;
    readonly?: boolean;
    allowNegative?: boolean;
    maxAbsCents?: number;
    /** Alle Fehler sofort zeigen (z. B. nach einem Absende-Versuch). */
    showErrors?: boolean;
    enterkeyhint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send";
    /** Nach dem Verlassen, wenn sich der Wert geändert hat. */
    onchange?: (cents: number | null) => void;
    class?: string;
  } = $props();

  const uid = randomId();
  const inputId = $derived(id ?? `mi-${uid}`);
  const hintId = `mi-hint-${uid}`;
  const errorId = `mi-err-${uid}`;

  let inputEl: HTMLInputElement | undefined = $state();
  let text = $state(untrack(() => (cents == null ? "" : formatCents(cents))));
  let focused = $state(false);
  /** Zuletzt von diesem Feld gesetzter Wert (erkennt Änderungen von außen). */
  let own: number | null = untrack(() => cents ?? null);
  let atFocus: number | null = null;
  /** Sichtbare Meldung (erst nach Verlassen/Absenden). */
  let shown = $state("");

  /** Aktuelle Meldung zum Text, unabhängig von der Sichtbarkeit. */
  function currentError(t: string): string {
    const r = parseMoneyToCents(t, { allowNegative, maxAbsCents });
    if (!r.ok) return r.error;
    if (r.value === null && required) return "Bitte einen Betrag eingeben.";
    return "";
  }

  const parseError = $derived(currentError(text));
  const visibleError = $derived(error || (showErrors ? parseError : shown));
  const describedBy = $derived(visibleError ? errorId : hint ? hintId : undefined);

  // Formularprüfung des Browsers (ohne `novalidate`) kennt so auch Lesefehler.
  $effect(() => {
    inputEl?.setCustomValidity(parseError);
  });

  // Änderung von außen übernehmen, solange das Feld nicht fokussiert ist.
  $effect(() => {
    const c = cents ?? null;
    untrack(() => {
      if (c === own || focused) return;
      own = c;
      text = c == null ? "" : formatCents(c);
      shown = "";
    });
  });

  function handleInput(e: Event) {
    text = (e.currentTarget as HTMLInputElement).value;
    const r = parseMoneyToCents(text, { allowNegative, maxAbsCents });
    const v = r.ok ? r.value : null;
    own = v;
    cents = v;
    if (shown && !currentError(text)) shown = "";
  }

  function handleFocus() {
    focused = true;
    atFocus = cents ?? null;
  }

  function handleBlur() {
    focused = false;
    const ext = cents ?? null;
    if (ext !== own) {
      // Während des Fokus von außen geändert: Anzeige folgt dem gebundenen Wert.
      own = ext;
      text = ext == null ? "" : formatCents(ext);
    } else {
      const r = parseMoneyToCents(text, { allowNegative, maxAbsCents });
      if (r.ok) text = r.value === null ? "" : formatCents(r.value);
    }
    shown = currentError(text);
    const v = cents ?? null;
    if (v !== atFocus) onchange?.(v);
  }

  /**
   * Zeigt die Meldung des Feldes (Absenden mit `novalidate`) und liefert,
   * ob der Wert gültig ist (leer ist gültig, außer bei `required`).
   */
  export function validate(): boolean {
    shown = currentError(text);
    return !shown;
  }

  /** Setzt den Fokus ins Feld (z. B. auf das erste fehlerhafte Feld). */
  export function focus(): void {
    inputEl?.focus();
  }
</script>

<div class="money-field {className}" class:with-label={!!label} class:invalid={!!visibleError}>
  {#if label}
    <label for={inputId}>
      {label}{#if required}<span class="req" aria-hidden="true">*</span>{/if}
    </label>
  {/if}
  <div class="box">
    <input
      bind:this={inputEl}
      class="kb-input"
      type="text"
      inputmode={allowNegative ? "text" : "decimal"}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      {enterkeyhint}
      id={inputId}
      {name}
      {placeholder}
      {disabled}
      {readonly}
      {required}
      aria-label={label ? undefined : ariaLabel}
      aria-required={required || undefined}
      aria-invalid={visibleError ? "true" : undefined}
      aria-describedby={describedBy}
      value={text}
      oninput={handleInput}
      onfocus={handleFocus}
      onblur={handleBlur}
      oninvalid={() => (shown = currentError(text))}
    />
    <span class="suffix" aria-hidden="true">€</span>
  </div>
  {#if visibleError}
    <div id={errorId} class="msg err">{visibleError}</div>
  {:else if hint}
    <div id={hintId} class="msg hint">{hint}</div>
  {/if}
</div>

<style>
  .money-field { min-width: 0; }
  .money-field.with-label { margin-bottom: 14px; }
  label { display: block; font-size: var(--fs-sm); font-weight: 600; margin-bottom: 5px; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .box { position: relative; }
  input {
    width: 100%;
    text-align: right;
    font-variant-numeric: tabular-nums;
    padding-right: 2em;
  }
  input[readonly] { background: var(--surface-page); }
  .suffix {
    position: absolute;
    right: 0.75em;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-subtle);
    pointer-events: none;
  }
  .msg { font-size: var(--fs-xs); margin-top: 4px; }
  .hint { color: var(--text-subtle); }
  .err { color: var(--status-danger-fg); }
</style>
