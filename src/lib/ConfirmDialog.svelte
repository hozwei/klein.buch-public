<script lang="ts">
  // Einheitlicher In-App-Bestätigungs-Dialog. Genau einmal im Root-Layout gemountet.
  // Gesteuert über confirmStore / confirmDialog() (siehe confirm.svelte.ts).
  //
  // U1: Fokusfalle, Escape und Rücksprung zum Auslöser über `focusTrap`.
  // Startfokus: bei Gefahr-Dialogen „Abbrechen“ (versehentliches Enter löst
  // nichts aus), sonst die Bestätigung. Der Hintergrund schließt nur
  // ungefährliche Dialoge; bei Gefahr braucht es eine bewusste Wahl.
  // ≤ 600 px Vollbild-Blatt (tokens.css `.kb-dialog`), Knöpfe unten.
  import { confirmStore, settleConfirm, settleChoice } from "#lib/confirm.svelte.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import Button from "#lib/Button.svelte";

  // Beim Schließen setzt settleChoice `current` auf null, bevor der {#if}-Block
  // abgebaut ist; die Ausdrücke darin (c.danger …) liefen dann gegen null
  // (pageerror in den E2E-Läufen). Darum den letzten Inhalt festhalten.
  type Shown = NonNullable<typeof confirmStore.current>;
  let last: Shown | null = null;
  const shown = $derived.by((): Shown | null => {
    const cur = confirmStore.current;
    if (cur) last = cur;
    return cur ?? last;
  });

  // R5-014: App-Shell `inert`, solange der Dialog offen ist.
  $effect(() => {
    if (confirmStore.current) {
      pushModal();
      return () => popModal();
    }
  });
</script>

{#if confirmStore.current}
  {@const c = shown!}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="kb-backdrop confirm-backdrop"
    role="presentation"
    onclick={(e) => {
      if (!c.danger && e.target === e.currentTarget) settleConfirm(false);
    }}
  >
    <div
      class="kb-dialog confirm"
      role={c.danger ? "alertdialog" : "dialog"}
      aria-modal="true"
      aria-labelledby="confirm-title"
      aria-describedby={c.body || c.bullets?.length ? "confirm-desc" : undefined}
      use:focusTrap={{ onEscape: () => settleConfirm(false) }}
    >
      <!-- tabindex: Der Text kann auf kleinen Geräten scrollen; so ist er auch per Tastatur
           scrollbar (axe scrollable-region-focusable). Startfokus bleibt auf dem Knopf. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="kb-dialog-body" tabindex="0" role="region" aria-labelledby="confirm-title">
        <h2 id="confirm-title">{c.title}</h2>
        <div id="confirm-desc">
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
        </div>
      </div>
      <div class="kb-dialog-foot actions">
        <Button variant="secondary" initialFocus={!!c.danger} onclick={() => settleConfirm(false)}>
          {c.cancelLabel ?? "Abbrechen"}
        </Button>
        {#if c.altLabel}
          <Button variant="secondary" onclick={() => settleChoice("alt")}>
            {c.altLabel}
          </Button>
        {/if}
        <span class="confirm-btn" class:danger={c.danger}>
          <Button variant="primary" initialFocus={!c.danger} onclick={() => settleConfirm(true)}>
            {c.confirmLabel ?? "OK"}
          </Button>
        </span>
      </div>
    </div>
  </div>
{/if}

<style>
  .confirm-backdrop { z-index: 2000; }
  .confirm { --dialog-w: 30rem; }
  h2 {
    margin: 0 0 0.6rem;
    font-size: var(--fs-lg);
    color: var(--text);
  }
  .body {
    margin: 0 0 0.5rem;
    color: var(--text-muted);
    white-space: pre-line;
    line-height: 1.5;
  }
  .bullets {
    margin: 0.25rem 0 0.5rem;
    padding-left: 1.25rem;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .bullets li {
    margin: 0.15rem 0;
  }
  .actions {
    border-top: 0;
    background: transparent;
    padding-top: 0;
  }
  .confirm-btn { display: contents; }
  /* Gefahr: Weiß auf --danger-solid (7,38:1) statt Petrol. */
  .confirm-btn.danger :global(.btn) {
    background: var(--danger-solid);
    border-color: var(--danger-solid);
    color: var(--text-on-accent);
  }
  @media (hover: hover) {
    .confirm-btn.danger :global(.btn:hover) {
      background: var(--danger-solid-hover);
      border-color: var(--danger-solid-hover);
    }
  }
  /* Vollbild-Blatt: Text oben, Knöpfe unten in Daumenreichweite, volle Breite. */
  @media (max-width: 600px) {
    .actions {
      flex-direction: column-reverse;
      align-items: stretch;
      border-top: 1px solid var(--line);
      padding-top: var(--sp-3);
    }
    .actions :global(.btn) { width: 100%; }
  }
</style>
