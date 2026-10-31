<script lang="ts">
  // Block P2b — Paket-Katalog: Liste, nach Kategorie gruppiert.
  // U4 (U3-038, U3-036): je Kategorie eine Liste im Table-Baustein statt einzelner
  // 280-px-Karten; Zeilenaktion „Bearbeiten“ + Menü (Archivieren/Reaktivieren).
  // U4 (Warteliste (14), F6): Kategorien umbenennen (`packageCategoryUpdate`) und
  // sortieren (`packageCategoriesReorder`) — ohne Ziehen, mit Auf/Ab-Knöpfen (2.5.7).
  // U4 (V2-07/V2-09/V2-11/V2-12): Ladefehler mit „Erneut laden“; Archivieren ohne Rückfrage, mit
  // „Rückgängig“ im Toast; „Abbrechen“ vor der Hauptaktion; Knöpfe während eines Laufs ariaDisabled.
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Table from "#lib/Table.svelte";
  import ActionMenu from "#lib/ActionMenu.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { date } from "#lib/format.ts";
  import {
    packagesList,
    packageCategoriesList,
    packageCategoryCreate,
    packageCategoryUpdate,
    packageCategoriesReorder,
    packagesArchive,
    packagesReactivate,
  } from "#lib/api.ts";
  import type { Package, PackageCategory } from "#lib/types.ts";

  let packages = $state<Package[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let newCategory = $state("");
  let busy = $state(false);
  // U1: Fehler am Feld statt Toast.
  let categoryError = $state("");
  let categoryEl: HTMLInputElement | undefined = $state();

  // Umbenennen (eine Kategorie zur Zeit, Formular in der Zeile).
  let renameId = $state<string | null>(null);
  let renameValue = $state("");
  let renameError = $state("");
  let renameEl: HTMLInputElement | undefined = $state();
  // Ansage nach dem Verschieben (4.1.3).
  let orderStatus = $state("");

  async function reloadPackages() {
    try {
      packages = await packagesList();
    } catch (e) {
      flash(`Pakete neu laden fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    }
  }

  async function load() {
    loading = true;
    loadError = null;
    try {
      [packages, categories] = await Promise.all([packagesList(), packageCategoriesList()]);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  let groups = $derived.by(() => {
    const g: { id: string; name: string; items: Package[] }[] = [];
    for (const c of categories) {
      const items = packages.filter((p) => p.categoryId === c.id);
      if (items.length) g.push({ id: c.id, name: c.name, items });
    }
    const orphans = packages.filter(
      (p) => !p.categoryId || !categories.some((c) => c.id === p.categoryId),
    );
    if (orphans.length) g.push({ id: "none", name: "Ohne Kategorie", items: orphans });
    return g;
  });

  const countIn = (catId: string) => packages.filter((p) => p.categoryId === catId).length;

  async function addCategory(e?: Event) {
    e?.preventDefault();
    if (busy) return;
    if (!newCategory.trim()) {
      categoryError = "Bitte gib einen Namen für die Kategorie ein.";
      categoryEl?.focus();
      return;
    }
    categoryError = "";
    busy = true;
    try {
      await packageCategoryCreate(newCategory.trim());
      newCategory = "";
      categories = await packageCategoriesList();
      flash("Kategorie angelegt.");
    } catch (err) {
      categoryError = err instanceof Error ? err.message : String(err);
      categoryEl?.focus();
    } finally {
      busy = false;
    }
  }

  async function startRename(c: PackageCategory) {
    renameId = c.id;
    renameValue = c.name;
    renameError = "";
    await tick();
    renameEl?.focus();
    renameEl?.select();
  }

  async function cancelRename(c: PackageCategory) {
    renameId = null;
    renameError = "";
    await tick();
    document.getElementById(`cat-rename-${c.id}`)?.focus();
  }

  async function saveRename(c: PackageCategory, e?: Event) {
    e?.preventDefault();
    const name = renameValue.trim();
    if (busy) return;
    if (!name) {
      renameError = "Bitte gib einen Namen für die Kategorie ein.";
      renameEl?.focus();
      return;
    }
    if (name === c.name) return cancelRename(c);
    busy = true;
    try {
      await packageCategoryUpdate(c.id, name);
      categories = await packageCategoriesList();
      flash(`Kategorie in „${name}“ umbenannt.`);
      await cancelRename(c);
    } catch (err) {
      renameError = err instanceof Error ? err.message : String(err);
      renameEl?.focus();
    } finally {
      busy = false;
    }
  }

  /** Verschiebt eine Kategorie um eine Stelle (Alternative zum Ziehen, 2.5.7). */
  async function move(c: PackageCategory, dir: -1 | 1) {
    const from = categories.findIndex((x) => x.id === c.id);
    const to = from + dir;
    if (busy || from < 0 || to < 0 || to >= categories.length) return;
    const before = categories;
    const next = [...categories];
    next.splice(from, 1);
    next.splice(to, 0, c);
    categories = next;
    busy = true;
    try {
      await packageCategoriesReorder(next.map((x) => x.id));
      orderStatus = `„${c.name}“ steht jetzt an Stelle ${to + 1} von ${next.length}.`;
    } catch (err) {
      categories = before;
      flash(`Reihenfolge speichern fehlgeschlagen: ${err instanceof Error ? err.message : String(err)}`, "error");
    } finally {
      busy = false;
    }
    // Der Knoten wandert im DOM; Fokus bleibt auf demselben Knopf der Kategorie.
    await tick();
    document.getElementById(`cat-${dir < 0 ? "up" : "down"}-${c.id}`)?.focus();
  }

  /**
   * Archivieren ist umkehrbar: keine Rückfrage, dafür „Rückgängig“ im Toast
   * (design.md §5, U4 V2-12). Der Rückgängig-Lauf selbst bietet kein erneutes Rückgängig an.
   */
  async function setArchived(p: Package, archive: boolean, undo = false) {
    if (busy) return;
    busy = true;
    try {
      if (archive) await packagesArchive(p.id);
      else await packagesReactivate(p.id);
      const msg = archive ? `Paket „${p.name}“ archiviert.` : `Paket „${p.name}“ reaktiviert.`;
      if (undo) flash(msg);
      else flash(msg, "ok", { action: { label: "Rückgängig", run: () => void setArchived(p, !archive, true) } });
      await reloadPackages();
    } catch (e) {
      flash(`Paket ${archive ? "archivieren" : "reaktivieren"} fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar title="Pakete" secondary={[{ label: "Broschüre", href: "/packages/brochure" }]}>
  {#snippet actions()}
    <Button variant="primary" href="/packages/new">+ Neues Paket</Button>
  {/snippet}
</PageBar>

<p class="lead">
  Wiederverwendbare Dienstleistungs-Pakete für Angebote und Rechnungen. Jede
  Änderung an einem Paket erzeugt eine neue Version — alte Versionen bleiben
  unverändert nachvollziehbar.
</p>

{#if loading}
  <LoadingState />
{:else if loadError}
  <Banner title="Die Pakete konnten nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else}
  {#if packages.length === 0}
    <div class="empty">
      <p>Noch keine Pakete. Kategorien sind optional; du kannst sie unten anlegen.</p>
      <!-- U4 (V2-09, ADR 0041 Nr. 64): gleiche Beschriftung wie die PageBar, dort steht schon die primary. -->
      <Button variant="secondary" href="/packages/new">+ Neues Paket</Button>
    </div>
  {/if}

  {#each groups as group (group.id)}
    <section class="group" aria-labelledby="grp-{group.id}">
      <h2 class="cat-h" id="grp-{group.id}">{group.name}</h2>
      <Table cards cardGrid>
        <caption class="kb-visually-hidden">Pakete in {group.name}</caption>
        <thead>
          <tr>
            <th>Paket</th>
            <th class="kb-num c-ver">Version</th>
            <th class="kb-col-opt2 c-date">Geändert</th>
            <th class="c-status">Status</th>
            <th class="c-act"><span class="kb-visually-hidden">Aktion</span></th>
          </tr>
        </thead>
        <tbody>
          {#each group.items as p (p.id)}
            <tr class:archived={p.status === "archived"}>
              <td class="kb-card-head" data-label="Paket"><a class="kb-link kb-target pkg-name" href={`/packages/${p.id}`}>{p.name}</a></td>
              <td class="kb-num" data-label="Version">{p.currentRevision ?? "—"}</td>
              <td class="kb-col-opt2" data-label="Geändert">{date(p.updatedAt)}</td>
              <td class="kb-card-status" data-label="Status">
                {#if p.status === "archived"}
                  <Badge tone="neutral">Archiviert</Badge>
                {:else}
                  <Badge tone="success">Aktiv</Badge>
                {/if}
              </td>
              <td>
                <div class="kb-actions">
                  <Button variant="secondary" href={`/packages/${p.id}`} ariaLabel={`${p.name} bearbeiten`}>
                    Bearbeiten
                  </Button>
                  <ActionMenu
                    variant="ghost"
                    ariaLabel={`Mehr zu ${p.name}`}
                    items={p.status === "archived"
                      ? [{ label: "Reaktivieren", onSelect: () => setArchived(p, false), disabled: busy }]
                      : [{ label: "Archivieren", onSelect: () => setArchived(p, true), danger: true, disabled: busy }]}
                  />
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </Table>
    </section>
  {/each}

  <section class="card cats" aria-labelledby="cats-h">
    <h2 id="cats-h">Kategorien</h2>
    <p class="muted small">
      Kategorien gliedern diese Liste, die Auswahl beim Einfügen in Belege und die
      Broschüre. Die Reihenfolge änderst du mit „Nach oben“ und „Nach unten“.
    </p>
    {#if categories.length > 0}
      <ol class="cat-list">
        {#each categories as c, i (c.id)}
          <li>
            {#if renameId === c.id}
              <form class="cat-rename" novalidate onsubmit={(e) => saveRename(c, e)}>
                <label for="cat-rename-input-{c.id}">Neuer Name für „{c.name}“</label>
                <div class="cat-row">
                  <input
                    id="cat-rename-input-{c.id}"
                    class="kb-input kb-w-long"
                    type="text"
                    autocomplete="off"
                    enterkeyhint="done"
                    aria-invalid={renameError ? "true" : undefined}
                    aria-describedby={renameError ? `cat-rename-err-${c.id}` : undefined}
                    bind:this={renameEl}
                    bind:value={renameValue}
                    oninput={() => (renameError = "")}
                    onkeydown={(e) => {
                      if (e.key === "Escape") {
                        e.preventDefault();
                        void cancelRename(c);
                      }
                    }}
                  />
                  <div class="kb-actions kb-start">
                    <!-- U4 (V2-11): „Abbrechen“ zuerst, Hauptaktion zuletzt. -->
                    <Button variant="secondary" onclick={() => cancelRename(c)}>Abbrechen</Button>
                    <Button type="submit" variant="primary" ariaDisabled={busy}>Umbenennen</Button>
                  </div>
                </div>
                {#if renameError}<p id="cat-rename-err-{c.id}" class="err">{renameError}</p>{/if}
              </form>
            {:else}
              <div class="cat-info">
                <span class="cat-name">{c.name}</span>
                <span class="muted small">{countIn(c.id) === 1 ? "1 Paket" : `${countIn(c.id)} Pakete`}</span>
              </div>
              <div class="kb-actions">
                <Button
                  id={`cat-up-${c.id}`}
                  variant="secondary"
                  ariaDisabled={i === 0 || busy}
                  ariaLabel={`Nach oben: ${c.name}`}
                  onclick={() => move(c, -1)}
                ><span aria-hidden="true">↑</span> Nach oben</Button>
                <Button
                  id={`cat-down-${c.id}`}
                  variant="secondary"
                  ariaDisabled={i === categories.length - 1 || busy}
                  ariaLabel={`Nach unten: ${c.name}`}
                  onclick={() => move(c, 1)}
                ><span aria-hidden="true">↓</span> Nach unten</Button>
                <Button
                  id={`cat-rename-${c.id}`}
                  variant="secondary"
                  ariaLabel={`Umbenennen: ${c.name}`}
                  onclick={() => startRename(c)}
                >Umbenennen</Button>
              </div>
            {/if}
          </li>
        {/each}
      </ol>
    {:else}
      <p class="muted">Noch keine Kategorien.</p>
    {/if}
    <p class="kb-visually-hidden" role="status">{orderStatus}</p>

    <form class="cat-add" onsubmit={addCategory} novalidate>
      <label for="cat-new">Neue Kategorie</label>
      <div class="cat-row">
        <input
          id="cat-new"
          class="kb-input kb-w-long"
          type="text"
          autocomplete="off"
          enterkeyhint="done"
          placeholder="z. B. Hochzeit, Porträt"
          aria-invalid={categoryError ? "true" : undefined}
          aria-describedby={categoryError ? "cat-new-err" : undefined}
          bind:this={categoryEl}
          bind:value={newCategory}
          oninput={() => (categoryError = "")}
        />
        <Button type="submit" variant="secondary" ariaDisabled={busy}>Kategorie anlegen</Button>
      </div>
      {#if categoryError}<p id="cat-new-err" class="err">{categoryError}</p>{/if}
    </form>
  </section>
{/if}

<style>
  /* .lead global aus tokens.css; Listen im Table-Baustein (design.md §4.5). */
  .muted { color: var(--text-muted); }
  .small { font-size: var(--type-small); }
  .group { margin-bottom: var(--sp-5); }
  /* U4 (V3-11): Fluchten über alle Kategorien — jede Teiltabelle hat dieselben festen
     Spaltenbreiten (erste Zeile bestimmt sie, ausgeblendete Spalten zählen nicht); der
     Paketname bekommt den Rest. Nur im Tabellenmodus, die Karten (≤ 820 px) bleiben. */
  @media screen and (min-width: 821px), print {
    .group :global(table.kb-table) { table-layout: fixed; }
    .group th.c-ver { width: 6rem; }
    .group th.c-date { width: 8rem; }
    .group th.c-status { width: 8rem; }
    .group th.c-act { width: 12rem; }
  }
  .cat-h {
    font-size: var(--type-heading);
    margin: 0 0 var(--sp-3);
  }
  /* Paketname als Link: `.kb-link .kb-target` (V2-21, ADR 0041 Nr. 166). */
  .pkg-name { font-weight: 600; overflow-wrap: anywhere; }
  /* Archiviert: gedämpfter Text statt Transparenz (ADR 0041 Nr. 26), Badge trägt den Status. */
  tr.archived .pkg-name { color: var(--text-muted); }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--sp-3);
    margin-bottom: var(--sp-5);
  }
  .empty p { margin: 0; color: var(--text-muted); }
  .cats { max-width: 48rem; }
  .cat-list { list-style: none; margin: 0 0 var(--sp-4); padding: 0; }
  .cat-list li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-4);
    padding: var(--sp-2) 0;
    border-bottom: var(--rule);
  }
  .cat-list li:first-child { border-top: var(--rule); }
  .cat-info { display: flex; flex-direction: column; min-width: 0; flex: 1 1 10rem; }
  .cat-name { font-weight: 600; overflow-wrap: anywhere; }
  .cat-rename { flex: 1 1 100%; }
  .cat-add label, .cat-rename label {
    display: block;
    margin-bottom: var(--sp-1);
    font-size: var(--type-small);
    font-weight: 600;
  }
  .cat-row { display: flex; gap: var(--sp-2); flex-wrap: wrap; align-items: center; }
  .cat-row input { flex: 1 1 14rem; min-width: 0; scroll-margin-block: var(--sp-6); }
  .err { margin: var(--sp-1) 0 0; font-size: var(--type-small); font-weight: 600; color: var(--status-danger-fg); }
</style>
