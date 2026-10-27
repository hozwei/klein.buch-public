<script lang="ts">
  // Mengen- und Zahleneingabe (Lauf 2026.11, U1; Plan §4.2, ADR 0041).
  //
  // Gleiche Leseregel und gleiches Verhalten wie `MoneyInput.svelte`, aber
  // ohne Währung: Menge (Standard 4 Nachkommastellen wie der Server,
  // `domain::money::QUANTITY_DECIMALS`), Kilometer, Nutzungsdauer, Prozent.
  // `integer` macht daraus eine Ganzzahl-Eingabe (Tage, Port, Jahr, Zähler)
  // mit `inputmode="numeric"`. Achtung Mengen: Ein einzelner Punkt mit drei
  // Ziffern ist ein Tausenderpunkt (`1.234` = 1234, Plan-Regel); die Anzeige
  // nach dem Verlassen („1.234“) macht das sichtbar.
  import { untrack } from "svelte";
  import { randomId } from "#lib/browser.ts";
  import { formatQuantity, parseQuantity, QUANTITY_DECIMALS } from "#lib/money.ts";

  let {
    value = $bindable<number | null>(null),
    maxFractionDigits = QUANTITY_DECIMALS,
    integer = false,
    grouping = true,
    max,
    unit,
    noun = "Menge",
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
    showErrors = false,
    enterkeyhint,
    onchange,
    class: className = "",
    fieldClass = "",
  }: {
    /** Zahl; `null` = leer oder ungültig. */
    value?: number | null;
    /** Erlaubte Nachkommastellen (ignoriert bei `integer`). */
    maxFractionDigits?: number;
    /** Nur ganze Zahlen (`inputmode="numeric"`). */
    integer?: boolean;
    /** Tausenderpunkt in der Anzeige (aus für Port, Jahr, Zähler). */
    grouping?: boolean;
    /** Obergrenze (inklusive) des Betrags. */
    max?: number;
    /** Sichtbare Einheit hinter dem Wert (`km`, `%`, `Jahre`); nicht Teil des Werts. */
    unit?: string;
    /** Wort in Fehlermeldungen: „Menge“ (Standard), „Wert“, „Zahl“. */
    noun?: string;
    label?: string;
    ariaLabel?: string;
    id?: string;
    name?: string;
    hint?: string;
    error?: string;
    placeholder?: string;
    required?: boolean;
    disabled?: boolean;
    readonly?: boolean;
    allowNegative?: boolean;
    showErrors?: boolean;
    enterkeyhint?: "enter" | "done" | "go" | "next" | "previous" | "search" | "send";
    onchange?: (value: number | null) => void;
    /** Klassen am äußeren Baustein (Label, Feld, Hinweis, Fehler), z. B. Rasterplatz. */
    class?: string;
    /**
     * Klassen nur an der Feld-Box, z. B. `kb-w-amount` (U4): begrenzt die Breite des
     * Felds, Label, Hinweis und Fehler behalten die volle Breite.
     */
    fieldClass?: string;
  } = $props();

  const uid = randomId();
  const inputId = $derived(id ?? `qi-${uid}`);
  const hintId = `qi-hint-${uid}`;
  const errorId = `qi-err-${uid}`;
  const frac = $derived(integer ? 0 : maxFractionDigits);

  function parse(t: string) {
    return parseQuantity(t, { maxFractionDigits: frac, allowNegative, max, noun });
  }
  function fmt(v: number | null): string {
    return v == null ? "" : formatQuantity(v, frac, grouping);
  }

  let inputEl: HTMLInputElement | undefined = $state();
  let text = $state(untrack(() => fmt(value)));
  let focused = $state(false);
  let own: number | null = untrack(() => value ?? null);
  let atFocus: number | null = null;
  let shown = $state("");

  function currentError(t: string): string {
    const r = parse(t);
    if (!r.ok) return r.error;
    if (r.value === null && required) return "Bitte einen Wert eingeben.";
    return "";
  }

  const parseError = $derived(currentError(text));
  const visibleError = $derived(error || (showErrors ? parseError : shown));
  // U4 (V4-14): Hinweis bleibt bei einem Fehler stehen (Korrekturhilfe, 3.3.3).
  const describedBy = $derived(
    [visibleError ? errorId : "", hint ? hintId : ""].filter(Boolean).join(" ") || undefined,
  );

  $effect(() => {
    inputEl?.setCustomValidity(parseError);
  });

  $effect(() => {
    const v = value ?? null;
    untrack(() => {
      if (v === own || focused) return;
      own = v;
      text = fmt(v);
      shown = "";
    });
  });

  function handleInput(e: Event) {
    text = (e.currentTarget as HTMLInputElement).value;
    const r = parse(text);
    const v = r.ok ? r.value : null;
    own = v;
    value = v;
    if (shown && !currentError(text)) shown = "";
  }

  function handleFocus() {
    focused = true;
    atFocus = value ?? null;
  }

  function handleBlur() {
    focused = false;
    const ext = value ?? null;
    if (ext !== own) {
      own = ext;
      text = fmt(ext);
    } else {
      const r = parse(text);
      if (r.ok) text = fmt(r.value);
    }
    shown = currentError(text);
    const v = value ?? null;
    if (v !== atFocus) onchange?.(v);
  }

  /** Zeigt die Meldung des Feldes und liefert, ob der Wert gültig ist. */
  export function validate(): boolean {
    shown = currentError(text);
    return !shown;
  }

  export function focus(): void {
    inputEl?.focus();
  }
</script>

<div class="qty-field {className}" class:with-label={!!label} class:invalid={!!visibleError}>
  {#if label}
    <label for={inputId}>
      {label}{#if required}<span class="req" aria-hidden="true">*</span>{/if}
    </label>
  {/if}
  <div class="box {fieldClass}" class:has-unit={!!unit}>
    <input
      bind:this={inputEl}
      class="kb-input"
      type="text"
      inputmode={allowNegative ? "text" : integer ? "numeric" : "decimal"}
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
    {#if unit}<span class="suffix" aria-hidden="true">{unit}</span>{/if}
  </div>
  {#if visibleError}
    <div id={errorId} class="msg err">{visibleError}</div>
  {/if}
  {#if hint}
    <div id={hintId} class="msg hint">{hint}</div>
  {/if}
</div>

<style>
  .qty-field { min-width: 0; }
  .qty-field.with-label { margin-bottom: var(--sp-4); }
  /* Wie FormField (U4, V4-14): Label, Hinweis und Fehler aus den Typo-Rollen. */
  label { display: block; font-size: var(--type-small); font-weight: 600; line-height: var(--lh-snug); margin-bottom: var(--sp-1); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .box { position: relative; }
  input {
    width: 100%;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .has-unit input { padding-right: 3.5em; }
  input[readonly] { background: var(--surface-page); }
  .suffix {
    position: absolute;
    right: 0.75em;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-subtle);
    pointer-events: none;
  }
  .msg { font-size: var(--type-small); margin-top: var(--sp-1); line-height: 1.4; }
  .hint { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); font-weight: 600; }
</style>
