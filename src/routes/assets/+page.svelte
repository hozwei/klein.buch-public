<script lang="ts">
  import { yearBerlin } from "#lib/date.ts";
  import { onMount } from "svelte";
  import { assetsList, depreciationAccrueYear, depreciationListForYear } from "#lib/api.ts";
  import type { AssetListItem, DepreciationEntry } from "#lib/types.ts";
  import { euro, date, percent } from "#lib/format.ts";
  import { depreciationMethodShort } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import { assetStatusView } from "./assetStatus.ts";
  import DocNo from "#lib/DocNo.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";

  let all = $state<AssetListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Filter
  let search = $state("");
  let yearFilter = $state<number | "">("");
  let statusFilter = $state<"active" | "disposed" | "">("active");

  // AfA-Buchen
  const currentYear = yearBerlin();
  let showAccrue = $state(false);
  let accrueYear = $state<number>(currentYear);

  // U4 (F6, Warteliste 14): schon gebuchte AfA des gewählten Jahres im Buchen-Bereich
  // (`depreciation_list_for_year`, liest nur) — vor dem Buchen sichtbar, was es gibt.
  let booked = $state<DepreciationEntry[] | null>(null);
  let bookedError = $state(false);
  $effect(() => {
    if (!showAccrue) return;
    const y = accrueYear;
    let stale = false;
    booked = null;
    bookedError = false;
    depreciationListForYear(y)
      .then((list) => {
        if (!stale) booked = list;
      })
      .catch(() => {
        if (!stale) bookedError = true;
      });
    return () => {
      stale = true;
    };
  });
  let bookedSum = $derived((booked ?? []).reduce((s, e) => s + e.depreciationAmountCents, 0));
  let assetById = $derived(new Map(all.map((a): [string, AssetListItem] => [a.id, a])));

  async function load() {
    loading = true;
    error = null;
    try {
      all = await assetsList();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  let years = $derived(
    [...new Set(all.map((a) => a.acquisitionFiscalYear))].sort((a, b) => b - a),
  );

  let accrueYearOptions = $derived.by(() => {
    const min = all.length
      ? Math.min(...all.map((a) => a.acquisitionFiscalYear))
      : currentYear;
    const out: number[] = [];
    for (let y = currentYear; y >= min; y--) out.push(y);
    return out;
  });

  let items = $derived(
    all.filter((a) => {
      if (statusFilter === "active" && a.disposed === 1) return false;
      if (statusFilter === "disposed" && a.disposed === 0) return false;
      if (yearFilter !== "" && a.acquisitionFiscalYear !== yearFilter) return false;
      const q = search.trim().toLowerCase();
      if (q) {
        const hay = [a.assetNumber, a.label].join(" ").toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    }),
  );

  // Gesamter Restbuchwert der angezeigten, nicht veräußerten Anlagen.
  let bookValueSum = $derived(
    items.filter((a) => a.disposed === 0).reduce((acc, a) => acc + a.bookValueCents, 0),
  );

  function resetFilters() {
    search = "";
    yearFilter = "";
    statusFilter = "active";
  }

  async function runAccrue() {
    if (busy) return;
    const ok = await confirmDialog({
      title: `Abschreibung für ${accrueYear} buchen?`,
      body:
        "Für jede Anschaffung wird die Jahres-Abschreibung berechnet und gebucht. " +
        "Noch nicht gebuchte Vorjahre (ab dem Anschaffungsjahr) werden mitgebucht. " +
        "Solange das Geschäftsjahr nicht abgeschlossen ist, kannst du die Buchung " +
        "auf der jeweiligen Anschaffung wieder zurücksetzen.",
      confirmLabel: "Jetzt buchen",
      cancelLabel: "Abbrechen",
    });
    if (!ok) return;
    busy = true;
    try {
      const r = await depreciationAccrueYear(accrueYear);
      if (r.skippedLocked) {
        flash("Backup ist gesperrt — bitte erst entsperren, dann erneut buchen.", "error");
      } else if (r.bookedEntries === 0) {
        flash(`Für ${accrueYear} war nichts (mehr) zu buchen.`);
      } else {
        flash(
          `${r.bookedEntries} Abschreibung(en) für ${r.processedAssets} Anschaffung(en) gebucht · Summe ${euro(r.totalDepreciationCents)}.`,
        );
      }
      showAccrue = false;
      await load();
    } catch (e) {
      flash("Buchen fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

</script>

<!-- U4 (design.md §4.2): „Abschreibung buchen“ als Nebenaktion (bis 600 px im Menü). -->
<PageBar
  title="Anschaffungen"
  secondary={[{ label: "Abschreibung buchen", onSelect: () => (showAccrue = !showAccrue) }]}
>
  {#snippet actions()}
    <Button variant="primary" href="/assets/new">+ Neue Anschaffung</Button>
    <HelpAnchor slug="anlagen-und-afa" />
  {/snippet}
</PageBar>

<p class="muted">
  Größere Anschaffungen (Laptop, Werkzeug, Möbel, Fahrzeug) setzt du nicht auf
  einmal ab, sondern <strong>verteilt über die Nutzungsdauer</strong> (Abschreibung
  / AfA). Klein.Buch rechnet die jährlichen Beträge aus — einmal im Jahr buchst du
  sie mit „Abschreibung buchen“.
</p>

{#if showAccrue}
  <section class="accrue card" aria-labelledby="accrue-h">
    <h2 id="accrue-h">Abschreibung buchen</h2>
    <p class="muted">
      Bucht die Jahres-Abschreibung aller Anschaffungen bis zum gewählten Jahr. Für die
      Erstanwendung wählst du z. B. {currentYear} — Vorjahre werden automatisch
      mitgebucht.
    </p>
    <div class="row">
      <div class="field">
        <label for="accrue-year">Geschäftsjahr</label>
        <select id="accrue-year" class="kb-input" bind:value={accrueYear}>
          {#each accrueYearOptions as y}<option value={y}>{y}</option>{/each}
        </select>
      </div>
      <Button variant="secondary" size="sm" onclick={() => (showAccrue = false)}>Schließen</Button>
      <Button variant="primary" size="sm" onclick={runAccrue} ariaDisabled={busy}>
        {busy ? "Bucht …" : `AfA ${accrueYear} buchen`}
      </Button>
    </div>
    <div class="booked">
      {#if bookedError}
        <p class="muted">Die schon gebuchte Abschreibung für {accrueYear} ließ sich nicht laden.</p>
      {:else if booked === null}
        <p class="muted" role="status">Wird geladen …</p>
      {:else if booked.length === 0}
        <p class="muted">Für {accrueYear} ist noch keine Abschreibung gebucht.</p>
      {:else}
        <details>
          <summary>
            Für {accrueYear} schon gebucht: {booked.length}
            {booked.length === 1 ? "Anschaffung" : "Anschaffungen"}, zusammen <span class="kb-num">{euro(bookedSum)}</span>
          </summary>
          <ul class="booked-list">
            {#each booked as b (b.id)}
              {@const a = assetById.get(b.assetId)}
              <li>
                <span class="booked-name">
                  {#if a}<DocNo value={a.assetNumber} href={`/assets/${a.id}`} inline />{" "}{a.label}{:else}Anschaffung{/if}
                </span>
                <span class="booked-meta">
                  {b.monthsInYear} {b.monthsInYear === 1 ? "Monat" : "Monate"}{#if b.lockedAt}{" "}· festgeschrieben{/if}
                </span>
                <span class="kb-num booked-amt">{euro(b.depreciationAmountCents)}</span>
              </li>
            {/each}
          </ul>
        </details>
      {/if}
    </div>
  </section>
{/if}

<div class="filters" role="search" aria-label="Anschaffungen filtern">
  <div class="f-search">
    <label class="kb-visually-hidden" for="asset-search">Anschaffungen durchsuchen</label>
    <input
      id="asset-search"
      type="search"
      class="kb-input"
      placeholder="Nr. oder Bezeichnung …"
      enterkeyhint="search"
      autocomplete="off"
      bind:value={search}
    />
  </div>
  <label class="f-sel">
    <span class="kb-visually-hidden">Anschaffungsjahr</span>
    <select class="kb-input" bind:value={yearFilter}>
      <option value="">Alle Anschaffungsjahre</option>
      {#each years as y}<option value={y}>{y}</option>{/each}
    </select>
  </label>
  <label class="f-sel">
    <span class="kb-visually-hidden">Status</span>
    <select class="kb-input" bind:value={statusFilter}>
      <option value="active">Aktive</option>
      <option value="disposed">Veräußerte</option>
      <option value="">Alle</option>
    </select>
  </label>
  <Button variant="secondary" size="sm" onclick={resetFilters}>Filter zurücksetzen</Button>
</div>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if all.length === 0}
  <!-- U4 (V2-09): Erststart mit derselben Beschriftung wie die PageBar, als secondary. -->
  <div class="empty">
    <p class="muted">Noch keine Anschaffungen erfasst.</p>
    <Button variant="secondary" href="/assets/new">+ Neue Anschaffung</Button>
  </div>
{:else}
  <!-- U4 (V2-13): Objekt und Bereich heißen „Anschaffung“; „Anlage“ nur als Steuerbegriff. -->
  <p class="result-info" role="status">
    {items.length} {items.length === 1 ? "Anschaffung" : "Anschaffungen"} · Restbuchwert (aktiv):
    <strong class="kb-num">{euro(bookValueSum)}</strong>
  </p>
  {#if items.length === 0}
    <div class="empty">
      <p class="muted">Keine Treffer für diese Filter.</p>
      <Button variant="secondary" onclick={resetFilters}>Filter zurücksetzen</Button>
    </div>
  {:else}
    <Table caption="Anschaffungen" cards cardGrid>
      <thead>
        <tr>
          <th>Nummer</th>
          <th>Bezeichnung</th>
          <th class="kb-col-opt2">Angeschafft</th>
          <th class="num">Anschaffung</th>
          <th class="kb-col-opt1">Abschreibung</th>
          <th class="num">Restbuchwert</th>
          <th>Status</th>
          <th><span class="kb-visually-hidden">Aktion</span></th>
        </tr>
      </thead>
      <tbody>
        {#each items as a (a.id)}
          {@const b = assetStatusView(a)}
          <tr class={a.disposed === 1 ? "disposed" : ""}>
            <td data-label="Nummer"><DocNo value={a.assetNumber} href={`/assets/${a.id}`} muted={a.disposed === 1} /></td>
            <td class="wrap" data-label="Bezeichnung">{a.label}</td>
            <td class="kb-col-opt2" data-label="Angeschafft">{date(a.acquisitionDate)}</td>
            <td class="num kb-num" data-label="Anschaffung">{euro(a.acquisitionCostCents)}</td>
            <td class="kb-col-opt1" data-label="Abschreibung">
              <span class="cat">
                {depreciationMethodShort(a.depreciationMethod)}
                {#if a.businessSharePercent < 100}<Badge kind="tag">{percent(a.businessSharePercent)} betrieblich</Badge>{/if}
              </span>
            </td>
            <td class="num kb-num" data-label="Restbuchwert">{euro(a.bookValueCents)}</td>
            <td data-label="Status"><Badge tone={b.tone}>{b.label}</Badge></td>
            <td class="right">
              <Button variant="secondary" size="sm" href={`/assets/${a.id}`} ariaLabel={`Anschaffung ${a.assetNumber} öffnen`}>Öffnen</Button>
            </td>
          </tr>
        {/each}
      </tbody>
    </Table>
  {/if}
{/if}

<style>
  /* .card / .kb-input: globale Definitionen aus tokens.css. */
  .accrue h2 { margin-top: 0; font-size: var(--fs-md); }
  .accrue .row { display: flex; gap: var(--sp-3); align-items: end; flex-wrap: wrap; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); }
  .field label { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .filters { display: flex; gap: var(--sp-2); align-items: center; flex-wrap: wrap; margin: var(--sp-3) 0; }
  .filters .kb-input { width: auto; max-width: 100%; }
  .f-search { flex: 1 1 18rem; min-width: 0; }
  .f-search .kb-input { width: 100%; }
  .f-sel { display: inline-flex; min-width: 0; max-width: 100%; }
  .result-info { color: var(--text-muted); font-size: var(--fs-sm); margin: var(--sp-1) 0 var(--sp-2); }
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-3); margin: var(--sp-3) 0; }
  .empty p { margin: 0; }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.right { text-align: right; }
  /* U2 (U1-063): nicht mitten im Wort trennen („Arbeitsplatzrec/hner“) — `anywhere` lässt
     die Spalte im Tabellenmodus unter die Wortbreite schrumpfen; Silbentrennung (lang="de"). */
  td.wrap { overflow-wrap: break-word; hyphens: auto; }
  .cat { display: inline-flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap; }
  .booked { margin-top: var(--sp-4); border-top: var(--rule); padding-top: var(--sp-3); }
  .booked p { margin: 0; }
  .booked summary { cursor: pointer; min-height: var(--target); display: flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap; }
  .booked-list { list-style: none; margin: var(--sp-2) 0 0; padding: 0; max-width: 48rem; }
  .booked-list li {
    display: flex; flex-wrap: wrap; align-items: baseline; gap: var(--sp-1) var(--sp-3);
    padding: var(--sp-2) 0; border-bottom: var(--rule);
  }
  .booked-name { flex: 1 1 16rem; min-width: 0; overflow-wrap: break-word; }
  .booked-meta { color: var(--text-muted); font-size: var(--type-small); }
  .booked-amt { min-width: 7rem; white-space: nowrap; }
  .muted { color: var(--text-muted); max-width: 46rem; line-height: 1.5; }
</style>
