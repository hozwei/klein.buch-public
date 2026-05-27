<script lang="ts">
  // Block PV1-DROP — Watched Drop-Folder fuer eingehende E-Rechnungen.
  // Toggle + Folder-Picker (tauri-plugin-dialog) + manueller Sync-Button.
  // Pipeline-Identitaet zum UI-Import: siehe scheduler/drop_folder.rs.
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import PageBar from "$lib/PageBar.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import {
    dropFolderSettingsGet,
    dropFolderSettingsSet,
    dropFolderSyncNow,
  } from "$lib/api";
  import { flash } from "$lib/toast.svelte";

  let loading = $state(true);
  let saving = $state(false);
  let syncing = $state(false);
  let enabled = $state(false);
  let path = $state("");

  onMount(async () => {
    try {
      const s = await dropFolderSettingsGet();
      enabled = s.enabled;
      path = s.path;
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loading = false;
    }
  });

  async function pickFolder() {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Ordner für eingehende E-Rechnungen wählen",
      });
      if (typeof picked === "string" && picked.trim()) {
        path = picked;
      }
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function save() {
    saving = true;
    try {
      const updated = await dropFolderSettingsSet({ enabled, path });
      enabled = updated.enabled;
      path = updated.path;
      flash("Gespeichert.");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      saving = false;
    }
  }

  async function syncNow() {
    syncing = true;
    try {
      const r = await dropFolderSyncNow();
      if (r.skippedDisabled) {
        flash("Rechnungs-Eingang ist nicht aktiv oder hat keinen gültigen Pfad.");
      } else if (r.imported === 0 && r.failed === 0) {
        flash("Keine neuen Dateien gefunden.");
      } else if (r.failed === 0) {
        flash(`${r.imported} Eingangsrechnung(en) übernommen.`);
      } else {
        flash(
          `${r.imported} übernommen, ${r.failed} fehlerhaft (siehe Hinweise + failed/-Ordner).`,
          r.imported > 0 ? "ok" : "error",
        );
      }
    } catch (e) {
      flash(String(e), "error");
    } finally {
      syncing = false;
    }
  }
</script>

<PageBar
  back="/settings"
  backLabel="Einstellungen"
  title="Rechnungs-Eingang (überwachter Ordner)"
>
  {#snippet actions()}
    <button
      type="button"
      class="btn-secondary btn-sm"
      onclick={syncNow}
      disabled={syncing || !enabled || !path.trim()}
    >
      {syncing ? "Prüfe ..." : "Jetzt prüfen"}
    </button>
    <button
      type="submit"
      form="drop-folder-form"
      class="btn-primary btn-sm"
      disabled={saving}
    >
      {saving ? "Speichere ..." : "Speichern"}
    </button>
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade ...</p>
{:else}
  <form
    id="drop-folder-form"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
    novalidate
  >
    <section class="card">
      <h2>So funktioniert der Rechnungs-Eingang</h2>
      <p>
        Klein.Buch prüft den unten gewählten Ordner alle fünf Minuten und
        beim App-Start auf neue Eingangsrechnungen. Erkannt werden XML-Dateien
        (XRechnung) und PDF-Dateien mit eingebetteter E-Rechnung (ZUGFeRD).
      </p>
      <p>
        Erfolgreich übernommene Dateien wandern in den Unter-Ordner
        <code>processed/JAHR-MONAT/</code>. Dateien, die nicht übernommen
        werden konnten, landen in <code>failed/</code>. Daneben liegt eine
        <code>.error.txt</code> mit dem Grund. Versteckte System-Dateien wie
        <code>.DS_Store</code> oder <code>Thumbs.db</code> werden ignoriert.
      </p>
      <p class="muted">
        Die Übernahme nutzt genau dieselbe Prüfung wie das manuelle Einlesen
        unter „Kosten → E-Rechnung importieren": Pflichtangaben, KoSIT-Befund,
        GoBD-konforme Original-Ablage im Archiv. Es gibt also keinen
        „Schnell-Pfad".
      </p>
    </section>

    <section class="card">
      <h2>Ordner-Einstellungen</h2>
      <div class="field">
        <Toggle
          bind:checked={enabled}
          label="Rechnungs-Eingang aktivieren"
          description="Wenn aus, läuft keine automatische Übernahme. Der Ordner bleibt ungenutzt."
        />
      </div>

      <div class="field">
        <label for="drop-folder-path">Ordnerpfad</label>
        <div class="path-row">
          <input
            id="drop-folder-path"
            type="text"
            bind:value={path}
            placeholder="z. B. C:\Users\Manuel\OneDrive\Rechnungs-Eingang"
            readonly
          />
          <button type="button" class="btn-secondary btn-sm" onclick={pickFolder}>
            Ordner auswählen ...
          </button>
        </div>
        {#if enabled && !path.trim()}
          <p class="hint warn">
            Zum Aktivieren muss zuerst ein Ordner ausgewählt werden.
          </p>
        {/if}
      </div>
    </section>
  </form>
{/if}

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin-top: 0.75rem;
  }
  .field label {
    font-size: 0.85rem;
    color: var(--c-text-muted);
  }
  .path-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  .path-row input {
    flex: 1;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    font: inherit;
    background: var(--c-surface-muted, #f3f4f6);
    color: var(--c-text);
  }
  .hint {
    font-size: 0.85rem;
    margin: 0;
  }
  .hint.warn {
    color: #b45309;
  }
  .muted {
    color: var(--c-text-muted);
  }
  code {
    background: var(--c-surface-muted, #f3f4f6);
    padding: 0.05rem 0.3rem;
    border-radius: 4px;
    font-size: 0.85em;
  }
</style>
