<script lang="ts">
  // Formularfeld-Wrapper (DS-1): Label + Slot (das eigentliche Input) + Hinweis/Fehler.
  //
  // R5-016: Komponente generiert deterministische IDs (Input-ID, Hint-ID,
  // Error-ID) und reicht sie als Snippet-Parameter an den Slot durch. Der
  // Aufrufer bindet sie ans `<input>` per `id`, `aria-describedby`,
  // `aria-invalid`, `aria-required` — damit liest der Screen-Reader
  // Label + Hinweis/Fehler verlässlich vor. `htmlFor` wird gegenüber der
  // generierten ID vorrangig akzeptiert, falls der Aufrufer eine feste ID
  // braucht.
  import type { Snippet } from "svelte";

  type SlotProps = {
    inputId: string;
    describedBy: string | undefined;
    invalid: boolean;
    required: boolean;
  };

  let {
    label,
    hint,
    error,
    htmlFor,
    required = false,
    children,
  }: {
    label?: string;
    hint?: string;
    error?: string;
    htmlFor?: string;
    required?: boolean;
    children?: Snippet<[SlotProps]>;
  } = $props();

  // Stable per-Mount-ID — `$state`-frei, weil sie sich pro Komponente nicht
  // ändert und keine Reaktivität braucht. `crypto.randomUUID()` ist in Tauri-
  // WebView verfügbar (kein Secure-Context-Zwang).
  const _uid = crypto.randomUUID();
  const inputId = $derived(htmlFor ?? `ff-${_uid}`);
  const hintId = `ff-hint-${_uid}`;
  const errorId = `ff-err-${_uid}`;
  const describedBy = $derived(
    error ? errorId : hint ? hintId : undefined,
  );
</script>

<div class="field" class:error={!!error}>
  {#if label}
    <label for={inputId}>
      {label}{#if required}<span class="req" aria-hidden="true">*</span>{/if}
    </label>
  {/if}
  {@render children?.({
    inputId,
    describedBy,
    invalid: !!error,
    required,
  })}
  {#if error}
    <div id={errorId} class="msg err">{error}</div>
  {:else if hint}
    <div id={hintId} class="msg hint">{hint}</div>
  {/if}
</div>

<style>
  .field { margin-bottom: 14px; }
  label { display: block; font-size: var(--fs-sm); font-weight: 600; margin-bottom: 5px; }
  .req { color: var(--c-danger-500); margin-left: 2px; }
  .msg { font-size: var(--fs-xs); margin-top: 4px; }
  .hint { color: var(--c-text-subtle); }
  .err { color: var(--c-danger-700); }
</style>
