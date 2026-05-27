<script lang="ts">
  import { onMount } from "svelte";
  import { assetsList, depreciationAccrueYear } from "$lib/api";
  import type { AssetListItem } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { depreciationMethodShort } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";

  let all = $state<AssetListItem[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Filter
  let search = $state("");
  let yearFilter = $state<number | "">("");
  let statusFilter = $state<"active" | "disposed" | "">("active");

  // AfA-Buchen
  const currentYear = new Date().getFullYear();
  let showAccrue = $state(false);
  let accrueYear = $state<number>(currentYear);

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
    const ok = await confirmDialog({
      title: `Abschreibung für ${accrueYear} buchen?`,
      body:
        "Für jede Anlage wird die Jahres-Abschreibung berechnet und gebucht. " +
        "Noch nicht gebuchte Vorjahre (ab dem Anschaffungsjahr) werden mitgebucht. " +
        "Solange das Geschäftsjahr nicht abgeschlossen ist, kannst du die Buchung " +
        "auf der jeweiligen Anlage wieder zurücksetzen.",
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
          `${r.bookedEntries} Abschreibung(en) für ${r.processedAssets} Anlage(n) gebucht · Summe ${euro(r.totalDepreciationCents)}.`,
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

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";
  function statusBadge(a: AssetListItem): { tone: Tone; text: string } {
    if (a.disposed === 1) return { tone: "neutral", text: "veräußert" };
    if (a.bookValueCents <= 0) return { tone: "success", text: "abgeschrieben" };
    if (a.lockedAt) return { tone: "primary", text: "festgeschrieben" };
    if (a.lastDepreciationYear != null) return { tone: "info", text: "in Abschreibung" };
    return { tone: "warning", text: "neu" };
  }
</script>

<PageBar title="Anschaffungen">
  {#snippet actions()}
    <Button variant="secondary" onclick={() => (showAccrue = !showAccrue)}>
      Abschreibung buchen
    </Button>
    <Button variant="primary" href="/assets/new">+ Neue Anschaffung</Button>
    <HelpAnchor slug="anlagen-und-afa" />
  {/snippet}
</PageBar>

<p class="muted">
  Größere Anschaffungen (Laptop, Werkzeug, Möbel, Fahrzeug) setzt du nicht auf
  einmal ab, sondern <strong>verteilt über die Nutzungsdauer</strong> (Abschreibung
  / AfA). Klein.Buch rechnet die jährlichen Beträge aus — einmal im Jahr buchst du
  sie mit „Abschreibung buchen".
</p>

{#if showAccrue}
  <section class="accrue card">
    <h2>Abschreibung buchen</h2>
    <p class="muted">
      Bucht die Jahres-Abschreibung aller Anlagen bis zum gewählten Jahr. Für die
      Erstanwendung wählst du z. B. {currentYear} — Vorjahre werden automatisch
      mitgebucht.
    </p>
    <div class="row">
      <label>
        Geschäftsjahr
        <select class="kb-input" bind:value={accrueYear}>
          {#each accrueYearOptions as y}<option value={y}>{y}</option>{/each}
        </select>
      </label>
      <Button variant="primary" size="sm" onclick={runAccrue} disabled={busy}>
        {busy ? "Buche …" : `AfA ${accrueYear} buchen`}
      </Button>
      <Button variant="secondary" size="sm" onclick={() => (showAccrue = false)}>Schließen</Button>
    </div>
  </section>
{/if}

<div class="filters">
  <input
    type="search"
    class="kb-input search"
    placeholder="Suche: Nr. oder Bezeichnung …"
    bind:value={search}
  />
  <select class="kb-input" bind:value={yearFilter}>
    <option value="">Alle Anschaffungsjahre</option>
    {#each years as y}<option value={y}>{y}</option>{/each}
  </select>
  <select class="kb-input" bind:value={statusFilter}>
    <option value="active">Aktive</option>
    <option value="disposed">Veräußerte</option>
    <option value="">Alle</option>
  </select>
  <Button variant="secondary" size="sm" onclick={resetFilters}>Zurücksetzen</Button>
</div>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if all.length === 0}
  <p class="muted">Noch keine Anschaffungen erfasst.</p>
{:else}
  <p class="result-info">
    {items.length} {items.length === 1 ? "Anlage" : "Anlagen"} · Restbuchwert (aktiv):
    <strong>{euro(bookValueSum)}</strong>
  </p>
  {#if items.length === 0}
    <p class="muted">Keine Anlagen passen zu den Filtern.</p>
  {:else}
    <table class="kb-table">
      <thead>
        <tr>
          <th>Nr.</th>
          <th>Bezeichnung</th>
          <th>Angeschafft</th>
          <th class="num">Anschaffung</th>
          <th>Abschreibung</th>
          <th class="num">Restbuchwert</th>
          <th>Status</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each items as a (a.id)}
          {@const b = statusBadge(a)}
          <tr class={a.disposed === 1 ? "disposed" : ""}>
            <td><a href={`/assets/${a.id}`}>{a.assetNumber}</a></td>
            <td>{a.label}</td>
            <td>{date(a.acquisitionDate)}</td>
            <td class="num">{euro(a.acquisitionCostCents)}</td>
            <td>
              <span class="cat">
                {depreciationMethodShort(a.depreciationMethod)}
                {#if a.businessSharePercent < 100}<Badge tone="info">{a.businessSharePercent}%</Badge>{/if}
              </span>
            </td>
            <td class="num">{euro(a.bookValueCents)}</td>
            <td><Badge tone={b.tone}>{b.text}</Badge></td>
            <td class="right"><Button variant="secondary" size="sm" href={`/assets/${a.id}`}>Öffnen</Button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .accrue h2 { margin-top: 0; font-size: 1rem; }
  .accrue .row { display: flex; gap: 0.75rem; align-items: end; flex-wrap: wrap; }
  .accrue label { display: flex; flex-direction: column; font-size: 0.85rem; color: var(--c-text-muted); gap: 0.25rem; }
  .filters { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin: 0.75rem 0; }
  .filters .kb-input { width: auto; font-size: 0.9rem; }
  .search { flex: 1 1 18rem; }
  .result-info { color: var(--c-text-muted); font-size: 0.85rem; margin: 0.25rem 0 0.5rem; }
  table { width: 100%; }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  td.right { text-align: right; }
  .cat { display: inline-flex; align-items: center; gap: 0.35rem; flex-wrap: wrap; }
  tr.disposed td a { color: var(--c-text-subtle); }
  .muted { color: var(--c-text-muted); max-width: 46rem; line-height: 1.5; }
</style>
