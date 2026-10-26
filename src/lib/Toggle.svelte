<script lang="ts">
  // Klein.Buch Toggle (G2-UX.1) — iOS-Slider als role="switch".
  // Pattern: <label> wrapt ein visuell verstecktes (aber fokussierbares)
  //   <input type="checkbox" role="switch">. Der sichtbare Slider ist eine
  //   reine Schmuck-`<span class="track">` mit Pseudo-Thumb. Klick aufs
  //   Label togglet nativ; Tab fokussiert; Space togglet (Browser-Default).
  // Token-basiert: --accent (an), --line-control (aus, ≥ 3:1),
  //   --focus-ring, --r-pill; Trefferfläche mindestens --target.
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
  /* Trefferfläche: das ganze Label, mindestens --target hoch/breit. */
  .kb-toggle {
    position: relative;
    display: inline-flex;
    align-items: flex-start;
    gap: 0.6rem;
    min-height: var(--target);
    min-width: var(--target);
    padding-block: max(0px, calc((var(--target) - 24px) / 2));
    cursor: pointer;
    line-height: 1.35;
    user-select: none;
  }
  .kb-toggle:not(.has-text) { align-items: center; justify-content: center; padding-block: 0; }
  .kb-toggle.disabled { cursor: not-allowed; opacity: 0.6; }

  /* Visuell versteckt, aber fokussier-/tastatur-bedienbar (sr-only-Muster).
     left/top an der Schiene, damit Fokus-Scrollen die richtige Stelle zeigt. */
  .kb-toggle input {
    position: absolute;
    left: 0; top: 0;
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
    /* aus: Bedienelement-Grenze ≥ 3:1 gegen Weiß; an: Akzent */
    background: var(--line-control);
    transition: background var(--t-fast) ease;
    margin-top: 2px; /* optisch zur Label-Baseline */
  }
  .kb-toggle:not(.has-text) .track { margin-top: 0; }
  .thumb {
    position: absolute;
    top: 2px; left: 2px;
    width: 16px; height: 16px;
    border-radius: 50%;
    background: var(--surface-card);
    box-shadow: 0 1px 2px rgba(16, 40, 50, 0.25);
    transition: transform var(--t-fast) ease;
  }

  /* Zustände via peer-Selektor am echten Input. */
  .kb-toggle input:checked + .track { background: var(--accent); }
  .kb-toggle input:checked + .track .thumb { transform: translateX(16px); }
  .kb-toggle input:focus-visible + .track {
    outline: var(--focus-width) solid var(--focus-ring);
    outline-offset: var(--focus-offset);
  }
  .kb-toggle input:disabled + .track { opacity: 0.6; }

  .text {
    display: flex; flex-direction: column; gap: 0.1rem;
  }
  .text .lbl {
    font-weight: 600;
    color: var(--text);
    font-size: var(--fs-base);
  }
  .text .desc {
    color: var(--text-muted);
    font-size: var(--fs-sm);
  }
</style>
