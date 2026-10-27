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
  //
  // U4: `class` für das Raster (`kb-span-all`) bzw. Feldbreiten am Feld selbst
  // (`kb-w-date` …, tokens.css); Abstände und Schrift aus den Rollen-Tokens.
  // U4 (V4-14, 3.3.3): Der Hinweis bleibt bei einem Fehler stehen — er ist dann die
  // Korrekturhilfe. Fehler steht darüber; `aria-describedby` nennt beide
  // (Fehler zuerst), wie ContactForm.
  import type { Snippet } from "svelte";
  import { randomId } from "#lib/browser.ts";

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
    class: className = "",
    children,
  }: {
    label?: string;
    hint?: string;
    error?: string;
    htmlFor?: string;
    required?: boolean;
    /** Zusatzklassen am Rahmen, z. B. `kb-span-all` im Formular-Raster. */
    class?: string;
    children?: Snippet<[SlotProps]>;
  } = $props();

  // Stable per-Mount-ID — `$state`-frei, weil sie sich pro Komponente nicht
  // ändert und keine Reaktivität braucht. `randomId` kommt auch ohne Secure
  // Context aus (HTTP im LAN, Plan W9).
  const _uid = randomId();
  const inputId = $derived(htmlFor ?? `ff-${_uid}`);
  const hintId = `ff-hint-${_uid}`;
  const errorId = `ff-err-${_uid}`;
  const describedBy = $derived(
    [error ? errorId : "", hint ? hintId : ""].filter(Boolean).join(" ") || undefined,
  );
</script>

<div class="field {className}" class:error={!!error}>
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
  {/if}
  {#if hint}
    <div id={hintId} class="msg hint">{hint}</div>
  {/if}
</div>

<style>
  .field { margin-bottom: var(--sp-4); min-width: 0; }
  label { display: block; font-size: var(--type-small); font-weight: 600; line-height: var(--lh-snug); margin-bottom: var(--sp-1); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .msg { font-size: var(--type-small); margin-top: var(--sp-1); line-height: 1.4; }
  .hint { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); font-weight: 600; }
  /* Bildschirmtastatur/klebende Leisten: Feld beim Fokus nicht verdecken. */
  .field :global(input), .field :global(select), .field :global(textarea) { scroll-margin-block: var(--sp-6); }
</style>
