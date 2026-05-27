<script lang="ts">
  // Klein.Buch Toggle (G2-UX.1) — iOS-Slider als role="switch".
  // Pattern: <label> wrapt ein visuell verstecktes (aber fokussierbares)
  //   <input type="checkbox" role="switch">. Der sichtbare Slider ist eine
  //   reine Schmuck-`<span class="track">` mit Pseudo-Thumb. Klick aufs
  //   Label togglet nativ; Tab fokussiert; Space togglet (Browser-Default).
  // Token-basiert: --c-primary-600 (an), --c-border-strong (aus),
  //   --c-primary-500 (Focus-Ring), --r-pill.
  //
  // Verwendung:
  //   <Toggle bind:checked={x} label="…" description="…" />
  //   <Toggle bind:checked={x} ariaLabel="…" onchange={(v) => save(v)} />
  //
  // Die globale Bucket-B-Checkbox-Regel in tokens.css schließt
  // role="switch" explizit aus, damit dieser hidden-Input nicht den
  // 18×18-Häkchen-Style erbt.
  let {
    checked = $bindable(false),
    disabled = false,
    label,
    description,
    ariaLabel,
    onchange,
  }: {
    checked?: boolean;
    disabled?: boolean;
    label?: string;
    description?: string;
    ariaLabel?: string;
    onchange?: (next: boolean) => void;
  } = $props();

  function onChange(e: Event) {
    const next = (e.currentTarget as HTMLInputElement).checked;
    onchange?.(next);
  }
</script>

<label class="kb-toggle" class:disabled class:has-text={!!(label || description)}>
  <input
    type="checkbox"
    role="switch"
    bind:checked
    {disabled}
    aria-label={label ?? ariaLabel}
    onchange={onChange}
  />
  <span class="track" aria-hidden="true">
    <span class="thumb"></span>
  </span>
  {#if label || description}
    <span class="text">
      {#if label}<strong class="lbl">{label}</strong>{/if}
      {#if description}<small class="desc">{description}</small>{/if}
    </span>
  {/if}
</label>

<style>
  .kb-toggle {
    display: inline-flex;
    align-items: flex-start;
    gap: 0.6rem;
    cursor: pointer;
    line-height: 1.35;
    user-select: none;
  }
  .kb-toggle.disabled { cursor: not-allowed; opacity: 0.6; }

  /* Visuell versteckt, aber fokussier-/tastatur-bedienbar (sr-only-Muster). */
  .kb-toggle input {
    position: absolute;
    width: 1px; height: 1px;
    padding: 0; margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .track {
    position: relative;
    flex-shrink: 0;
    width: 36px; height: 20px;
    border-radius: var(--r-pill);
    background: var(--c-border-strong);
    transition: background .15s ease;
    margin-top: 2px; /* optisch zur Label-Baseline */
  }
  .thumb {
    position: absolute;
    top: 2px; left: 2px;
    width: 16px; height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgba(16, 40, 50, 0.25);
    transition: transform .15s ease;
  }

  /* Zustände via peer-Selektor am echten Input. */
  .kb-toggle input:checked + .track { background: var(--c-primary-600); }
  .kb-toggle input:checked + .track .thumb { transform: translateX(16px); }
  .kb-toggle input:focus-visible + .track {
    outline: 2px solid var(--c-primary-500);
    outline-offset: 2px;
  }
  .kb-toggle input:disabled + .track { opacity: 0.6; }

  .text {
    display: flex; flex-direction: column; gap: 0.1rem;
  }
  .text .lbl {
    font-weight: 600;
    color: var(--c-text);
    font-size: var(--fs-base);
  }
  .text .desc {
    color: var(--c-text-muted);
    font-size: var(--fs-sm);
  }
</style>
