<script lang="ts">
  // Einheitlicher In-App-Bestätigungs-Dialog. Genau einmal im Root-Layout gemountet.
  // Gesteuert über confirmStore / confirmDialog() (siehe confirm.svelte.ts).
  import { confirmStore, settleConfirm } from "$lib/confirm.svelte";
  import { pushModal, popModal } from "$lib/modalStack.svelte";

  let confirmBtn = $state<HTMLButtonElement | null>(null);
  // R5-013: Trigger-Element merken, damit der Fokus nach Close zurück darauf
  // springt (Tastatur-User landet sonst auf <body>). Pattern aus AboutDialog.
  let triggerEl: HTMLElement | null = null;

  // R5-014 + R5-013: beim Öffnen Trigger merken + Modal-Stack pushen +
  // Confirm-Button fokussieren; beim Schließen Stack poppen + Fokus zurück.
  $effect(() => {
    if (confirmStore.current) {
      triggerEl =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
      pushModal();
      // Im nächsten Tick fokussieren — `confirmBtn` ist erst nach dem Render
      // gesetzt.
      queueMicrotask(() => confirmBtn?.focus());
      return () => {
        popModal();
        triggerEl?.focus();
        triggerEl = null;
      };
    }
  });

  function onKeydown(e: KeyboardEvent) {
    if (!confirmStore.current) return;
    if (e.key === "Escape") {
      e.preventDefault();
      settleConfirm(false);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if confirmStore.current}
  {@const c = confirmStore.current}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) settleConfirm(false); }}
  >
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
      <h2 id="confirm-title">{c.title}</h2>
      {#if c.body}
        <p class="body">{c.body}</p>
      {/if}
      {#if c.bullets && c.bullets.length > 0}
        <ul class="bullets">
          {#each c.bullets as line}
            <li>{line}</li>
          {/each}
        </ul>
      {/if}
      <div class="actions">
        <button class="btn-cancel" onclick={() => settleConfirm(false)}>
          {c.cancelLabel ?? "Abbrechen"}
        </button>
        <button
          class="btn-confirm"
          class:danger={c.danger}
          bind:this={confirmBtn}
          onclick={() => settleConfirm(true)}
        >
          {c.confirmLabel ?? "OK"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* G2-UX.3.2 — macOS-Sheet-Look: gedimmter Backdrop mit weichem Blur,
     großzügiger Radius (--r-xl), zweilagiger XL-Schatten, Sheet-Drift mit
     Apple-Easing (--ease-apple, --t-slow). */
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 2000;
    background: rgba(16, 40, 50, 0.32);
    -webkit-backdrop-filter: blur(6px) saturate(140%);
    backdrop-filter: blur(6px) saturate(140%);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
    animation: fade-in var(--t-base) var(--ease-apple);
  }
  .dialog {
    background: var(--c-surface);
    border-radius: var(--r-xl);
    box-shadow: var(--sh-xl);
    padding: 1.5rem;
    max-width: 30rem;
    width: 100%;
    animation: sheet-in var(--t-slow) var(--ease-apple);
  }
  h2 {
    margin: 0 0 0.6rem;
    font-size: 1.15rem;
    color: var(--c-text);
  }
  .body {
    margin: 0 0 0.5rem;
    color: var(--c-text-muted);
    white-space: pre-line;
    line-height: 1.45;
  }
  .bullets {
    margin: 0.25rem 0 0.5rem;
    padding-left: 1.25rem;
    color: var(--c-text-muted);
    line-height: 1.5;
  }
  .bullets li {
    margin: 0.15rem 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 1.25rem;
  }
  button {
    border: 1px solid transparent;
    padding: 0.55rem 1.1rem;
    border-radius: var(--r-md);
    cursor: pointer;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .btn-cancel {
    background: var(--c-surface);
    color: var(--c-text);
    border-color: var(--c-border-strong);
  }
  .btn-cancel:hover {
    background: var(--c-bg);
  }
  .btn-confirm {
    background: var(--c-primary-600);
    color: #fff;
  }
  .btn-confirm:hover {
    background: var(--c-primary-700);
  }
  .btn-confirm.danger {
    background: var(--c-danger-500);
  }
  .btn-confirm.danger:hover {
    background: var(--c-danger-700);
  }
  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  /* Sheet-Einflug: deutlicher Vertikal-Drift (16 px) + minimales Scale,
     fühlt sich wie ein macOS-Action-Sheet an statt eines abrupten Pop-ins. */
  @keyframes sheet-in {
    from { transform: translateY(16px) scale(0.97); opacity: 0; }
    to { transform: translateY(0) scale(1); opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .backdrop, .dialog { animation: none; }
  }
</style>
