<script lang="ts" module>
  /** Was ContactSelect von einem Kontakt braucht (passt zu `Contact` aus types.ts). */
  export interface ContactOption {
    id: string;
    name: string;
    city?: string | null;
    email?: string | null;
  }

  const collator = new Intl.Collator("de", { sensitivity: "base", numeric: true });

  /**
   * Suchform eines Textes: klein, ohne Akzente, ß → ss, Umlaute auch als ae/oe/ue
   * („Mueller“ findet „Müller“, „Ubermuth“ findet „Übermuth“). Suchtext und Einträge
   * werden gleich gefaltet.
   */
  export function foldDe(s: string): string {
    return s
      .toLocaleLowerCase("de")
      .replace(/ß/g, "ss")
      .replace(/ä/g, "a")
      .replace(/ö/g, "o")
      .replace(/ü/g, "u")
      .replace(/ae/g, "a")
      .replace(/oe/g, "o")
      .replace(/ue/g, "u")
      .normalize("NFD")
      .replace(/\p{M}/gu, "");
  }

  /** Nach Name in deutscher Sortierung (DIN 5007-1, ADR 0041 Nr. 57). */
  export function sortContacts<T extends { name: string }>(list: readonly T[]): T[] {
    return [...list].sort((a, b) => collator.compare(a.name, b.name));
  }

  /**
   * Filtert nach allen Wörtern des Suchtexts (jedes muss in Name oder Zusatz
   * vorkommen, Reihenfolge egal). Leerer Suchtext = alle.
   */
  export function filterContacts<T extends ContactOption>(
    list: readonly T[],
    query: string,
    detail: (c: T) => string = (c) => c.city ?? "",
  ): T[] {
    const words = foldDe(query).split(/\s+/).filter(Boolean);
    if (words.length === 0) return [...list];
    return list.filter((c) => {
      const hay = foldDe(`${c.name} ${detail(c)}`);
      return words.every((w) => hay.includes(w));
    });
  }
</script>

<script lang="ts">
  // Kontaktauswahl für lange Listen (U4, Warteliste (15); design.md §4.7).
  //
  // Entscheidung (ADR 0041): Suchfeld + natives `<select>` statt APG-Combobox.
  // - Das native Auswahlfeld öffnet auf iOS/Android die Auswahl des Systems,
  //   Screenreader und Tastatur kennen es, und es nimmt an der Formularprüfung
  //   teil (`name`, `required`, `setCustomValidity`).
  // - Eine eigene Combobox mit Listbox bräuchte viel eigene Tastatur- und
  //   Ansagelogik und verhält sich in VoiceOver/TalkBack uneinheitlich.
  // - Das Suchfeld erscheint erst ab `searchFrom` Kontakten (Standard 8) und
  //   schränkt nur die Optionen ein; die Wahl trifft man immer im Auswahlfeld.
  //   Enter im Suchfeld übernimmt einen einzigen Treffer, sonst springt der
  //   Fokus ins Auswahlfeld; Escape leert die Suche. Die Trefferzahl wird höflich
  //   angesagt (4.1.3).
  // - Der gewählte Kontakt bleibt immer als Option erhalten, auch wenn die Suche
  //   ihn ausblendet (sonst verlöre das Feld still seinen Wert).
  //
  // Props wie MoneyInput: `label` oder `ariaLabel`, `id`, `name`, `hint`, `error`,
  // `required`, `showErrors`, `onchange`; `validate()` und `focus()` über `bind:this`.
  import { randomId } from "#lib/browser.ts";

  let {
    value = $bindable(""),
    contacts,
    label,
    ariaLabel,
    id,
    name,
    hint,
    error,
    required = false,
    disabled = false,
    placeholder = "— wählen —",
    requiredMessage = "Bitte einen Kontakt wählen.",
    detail = (c: ContactOption) => c.city ?? "",
    searchFrom = 8,
    sort = true,
    showErrors = false,
    onchange,
    class: className = "",
  }: {
    /** Gewählte Kontakt-ID; "" = keine Wahl. */
    value?: string;
    contacts: readonly ContactOption[];
    /** Sichtbares Label; ohne Label `ariaLabel` oder ein externes `<label for={id}>`. */
    label?: string;
    ariaLabel?: string;
    /** ID des Auswahlfelds (für ein externes Label oder den Fokus nach einem Fehler). */
    id?: string;
    name?: string;
    hint?: string;
    /** Fehler von außen (z. B. Server-Meldung); hat Vorrang. */
    error?: string;
    required?: boolean;
    disabled?: boolean;
    /** Text der leeren Option. */
    placeholder?: string;
    requiredMessage?: string;
    /** Zusatz in Klammern hinter dem Namen (Standard: Ort; z. B. E-Mail beim Versand). */
    detail?: (c: ContactOption) => string;
    /** Suchfeld ab so vielen Kontakten (0 = immer). */
    searchFrom?: number;
    /** Nach Name sortieren (deutsche Kollation); `false` behält die Reihenfolge. */
    sort?: boolean;
    /** Alle Fehler sofort zeigen (z. B. nach einem Absende-Versuch). */
    showErrors?: boolean;
    /** Nach einer Wahl durch den Nutzer. */
    onchange?: (id: string) => void;
    class?: string;
  } = $props();

  const uid = randomId();
  const selectId = $derived(id ?? `cs-${uid}`);
  const searchId = `cs-search-${uid}`;
  const countId = `cs-count-${uid}`;
  const hintId = `cs-hint-${uid}`;
  const errorId = `cs-err-${uid}`;

  let selectEl: HTMLSelectElement | undefined = $state();
  let query = $state("");
  /** Sichtbare Pflicht-Meldung (erst nach Verlassen/Absenden). */
  let shown = $state("");

  const ordered = $derived(sort ? sortContacts(contacts) : [...contacts]);
  const searchable = $derived(contacts.length >= searchFrom);
  const matches = $derived(filterContacts(ordered, searchable ? query : "", detail));
  /** Gewählter Kontakt, falls die Suche ihn ausblendet. */
  const pinned = $derived(
    value && !matches.some((c) => c.id === value) ? (ordered.find((c) => c.id === value) ?? null) : null,
  );
  const countText = $derived.by(() => {
    if (!searchable || !query.trim()) return "";
    const n = matches.length;
    if (n === 0) return "Kein Kontakt passt zur Suche.";
    return n === 1 ? "1 Kontakt passt zur Suche." : `${n} Kontakte passen zur Suche.`;
  });

  function optionText(c: ContactOption): string {
    const d = detail(c);
    return d ? `${c.name} (${d})` : c.name;
  }

  function currentError(): string {
    return required && !value ? requiredMessage : "";
  }
  const visibleError = $derived(error || (showErrors ? currentError() : shown));
  // U4 (V4-14): Hinweis bleibt bei einem Fehler stehen (Korrekturhilfe, 3.3.3).
  const describedBy = $derived(
    [visibleError ? errorId : "", hint ? hintId : ""].filter(Boolean).join(" ") || undefined,
  );

  // Formularprüfung des Browsers (ohne `novalidate`).
  $effect(() => {
    selectEl?.setCustomValidity(currentError());
  });

  function choose(next: string) {
    if (value !== next) value = next;
    if (shown && !currentError()) shown = "";
    onchange?.(next);
  }

  function onSelectChange(e: Event) {
    choose((e.currentTarget as HTMLSelectElement).value);
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      // Nie das Formular absenden: Enter heißt „übernehmen“.
      e.preventDefault();
      if (matches.length === 1 && matches[0].id !== value) choose(matches[0].id);
      selectEl?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      selectEl?.focus();
    } else if (e.key === "Escape" && query) {
      // Erst die Suche leeren; ein umgebender Dialog schließt erst beim zweiten Escape.
      e.preventDefault();
      e.stopPropagation();
      query = "";
    }
  }

  /**
   * Zeigt die Pflicht-Meldung (Absenden mit `novalidate`) und liefert, ob eine
   * Wahl vorliegt (bzw. das Feld optional ist).
   */
  export function validate(): boolean {
    shown = currentError();
    return !shown;
  }

  /** Setzt den Fokus ins Auswahlfeld (z. B. auf das erste fehlerhafte Feld). */
  export function focus(): void {
    selectEl?.focus();
  }
</script>

<div class="contact-select {className}" class:with-label={!!label} class:invalid={!!visibleError}>
  {#if label}
    <label for={selectId}>
      {label}{#if required}<span class="req" aria-hidden="true">*</span>{/if}
    </label>
  {/if}
  {#if searchable}
    <div class="cs-search">
      <svg
        class="cs-icon"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        aria-hidden="true"><circle cx="7" cy="7" r="4.5" /><path d="M10.5 10.5 14 14" /></svg
      >
      <input
        id={searchId}
        class="kb-input"
        type="search"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="done"
        placeholder="Suchen: Name oder Ort"
        aria-label={`${label ?? ariaLabel ?? "Kontakt"} suchen`}
        aria-controls={selectId}
        aria-describedby={countId}
        {disabled}
        bind:value={query}
        onkeydown={onSearchKey}
      />
    </div>
  {/if}
  <select
    bind:this={selectEl}
    id={selectId}
    class="kb-input"
    {name}
    {required}
    {disabled}
    bind:value
    aria-label={label ? undefined : ariaLabel}
    aria-required={required || undefined}
    aria-invalid={visibleError ? "true" : undefined}
    aria-describedby={describedBy}
    onchange={onSelectChange}
    onblur={() => (shown = currentError())}
  >
    <option value="" disabled={required}>{placeholder}</option>
    {#if pinned}<option value={pinned.id}>{optionText(pinned)}</option>{/if}
    {#each matches as c (c.id)}<option value={c.id}>{optionText(c)}</option>{/each}
  </select>
  {#if searchable}
    <div id={countId} class="msg count" aria-live="polite">{countText}</div>
  {/if}
  {#if visibleError}
    <div id={errorId} class="msg err">{visibleError}</div>
  {/if}
  {#if hint}
    <div id={hintId} class="msg hint">{hint}</div>
  {/if}
</div>

<style>
  .contact-select { min-width: 0; }
  .contact-select.with-label { margin-bottom: var(--sp-4); }
  label {
    display: block;
    font-size: var(--type-small);
    font-weight: 600;
    line-height: var(--lh-snug);
    margin-bottom: var(--sp-1);
  }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .cs-search {
    position: relative;
    margin-bottom: var(--sp-2);
  }
  .cs-icon {
    position: absolute;
    left: 11px;
    top: 50%;
    width: 16px;
    height: 16px;
    transform: translateY(-50%);
    color: var(--text-muted);
    pointer-events: none;
  }
  .cs-search input {
    -webkit-appearance: none;
    appearance: none;
    padding-left: 34px;
  }
  /* Kein Browser-Kreuz zum Leeren (zu kleine Trefferfläche); Escape leert. */
  .cs-search input::-webkit-search-cancel-button {
    -webkit-appearance: none;
    appearance: none;
  }
  .msg { font-size: var(--type-small); margin-top: var(--sp-1); line-height: 1.4; }
  .count { color: var(--text-muted); }
  .count:empty { margin: 0; }
  .hint { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); font-weight: 600; }
  .contact-select :global(select), .contact-select :global(input) { scroll-margin-block: var(--sp-6); }
</style>
