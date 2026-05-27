<script lang="ts">
  // Block 4 + G1-ENC Schritt 2 — Onboarding/Unlock-Gate.
  //
  // Seit der Bootstrap-Inversion ist die Passphrase das **Daten-Passwort**: die
  // SQLite-DB wird damit verschlüsselt (SQLCipher, ADR 0035) und der DB-Pool
  // entsteht erst NACH der Eingabe. Ohne Passwort lässt sich die App nicht
  // öffnen — deshalb gibt es kein „Später"/Überspringen mehr. Solange nicht
  // entsperrt ist, rendert das Layout die App-Seiten nicht (appState.ready).
  import { onMount } from "svelte";
  import { appState } from "$lib/stores.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import { pushModal, popModal } from "$lib/modalStack.svelte";
  import {
    backupNeedsOnboarding,
    backupIsUnlocked,
    backupSetupPassphrase,
    backupUnlock,
  } from "$lib/api";

  type Status = "loading" | "setup" | "unlock" | "ready";
  let status = $state<Status>("loading");
  let pass = $state("");
  let confirm = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  // R5-015: erstes Input-Element für Auto-Focus beim Wechsel auf setup/unlock.
  let firstInput = $state<HTMLInputElement | null>(null);

  // App-Seiten dürfen erst laden, wenn entsperrt ist (Pool liegt im State).
  $effect(() => {
    appState.ready = status === "ready";
  });

  // R5-014 + R5-015: sobald die Karte sichtbar ist (setup/unlock):
  // Modal-Stack pushen (App-Shell `inert`) + Auto-Focus aufs erste
  // Passphrase-Feld. queueMicrotask, damit `firstInput` nach dem Render
  // schon gesetzt ist. Bei Wechsel zu `ready`/`loading` wird gepoppt.
  $effect(() => {
    if (status === "setup" || status === "unlock") {
      pushModal();
      queueMicrotask(() => firstInput?.focus());
      return () => popModal();
    }
  });

  onMount(refresh);

  async function refresh() {
    try {
      if (await backupNeedsOnboarding()) {
        status = "setup";
        return;
      }
      status = (await backupIsUnlocked()) ? "ready" : "unlock";
    } catch (e) {
      error = String(e);
      status = "unlock";
    }
  }

  async function doSetup() {
    error = null;
    // R5-005: Passphrase-Floor 16 Zeichen (war 8). Verlust = Totalverlust by
    // design (ADR 0035), 8-Zeichen-Passwörter sind Wörterbuch-Treffer-fähig.
    // Backend erzwingt denselben Floor in `backup_setup_passphrase`.
    if (pass.length < 16) {
      error =
        "Das Passwort muss mindestens 16 Zeichen haben. " +
        "Tipp: Nimm 3–4 zufällige Wörter (z. B. „korrekt-pferd-batterie-klammer“).";
      return;
    }
    if (pass !== confirm) {
      error = "Die beiden Passwörter stimmen nicht überein.";
      return;
    }
    busy = true;
    try {
      await backupSetupPassphrase(pass);
      status = "ready";
    } catch (e) {
      error = String(e);
    } finally {
      // R5-007: Klartext bei JEDEM Pfad raus aus dem $state — auch im
      // Fehlerfall. Audit-Safe-Default: Re-Type beim nächsten Versuch.
      pass = "";
      confirm = "";
      busy = false;
    }
  }

  async function doUnlock() {
    error = null;
    busy = true;
    try {
      const ok = await backupUnlock(pass);
      if (ok) {
        status = "ready";
      } else {
        error = "Falsches Passwort.";
      }
    } catch (e) {
      error = String(e);
    } finally {
      // R5-006: Klartext nicht über die await-Grenze hinaus halten — egal
      // ob Erfolg, falsche Eingabe oder Exception.
      pass = "";
      busy = false;
    }
  }

  // R5-015: `onKey`-Helper entfernt — Enter-Submit läuft jetzt nativ über
  // das `<form onsubmit|preventDefault>`-Wrapping (auch aus dem ersten Feld).
</script>

{#if status === "setup" || status === "unlock"}
  <!-- R5-015: role=dialog + aria-modal + aria-labelledby; Inhalt als <form>,
       damit Enter aus jedem Input submitted (statt nur aus dem zweiten Feld
       per onkeydown). novalidate + preventDefault, damit der Browser kein
       eigenes Form-Verhalten dazwischenfunkt. -->
  <div
    class="overlay"
    role="dialog"
    aria-modal="true"
    aria-labelledby="backup-gate-title"
  >
    <div class="modal-card">
      {#if status === "setup"}
        <h2 id="backup-gate-title">
          Daten-Passwort festlegen <HelpAnchor slug="passphrase-einrichten" />
        </h2>
        <p>
          Bevor du loslegst, leg ein <strong>Daten-Passwort</strong> fest. Damit wird
          deine gesamte Buchhaltung auf diesem PC verschlüsselt — und auch deine
          Backups. Du brauchst es ab jetzt bei jedem Start, um Klein.Buch zu öffnen.
        </p>
        <p class="caveat">
          Wichtig: Dieses Passwort wird <strong>nirgends gespeichert</strong> und kann
          <strong>nicht zurückgesetzt</strong> werden. Vergisst du es, kommst du an
          deine Daten und Backups nicht mehr heran. Schreib es dir an einem sicheren
          Ort auf (z. B. in einem Passwort-Manager).
        </p>
        <form
          novalidate
          onsubmit={(e) => {
            e.preventDefault();
            doSetup();
          }}
        >
          <label>
            Passwort (mindestens 16 Zeichen — am besten 3–4 Wörter)
            <input
              bind:this={firstInput}
              type="password"
              bind:value={pass}
              autocomplete="new-password"
              disabled={busy}
            />
          </label>
          <label>
            Passwort wiederholen
            <input
              type="password"
              bind:value={confirm}
              autocomplete="new-password"
              disabled={busy}
            />
          </label>
          {#if error}<p class="err">{error}</p>{/if}
          <button type="submit" class="primary" disabled={busy}>
            {busy ? "Richte ein …" : "Festlegen & erstes Backup erstellen"}
          </button>
        </form>
      {:else}
        <h2 id="backup-gate-title">Willkommen zurück</h2>
        <p>
          Gib dein <strong>Daten-Passwort</strong> ein, um Klein.Buch zu öffnen. Ohne
          das Passwort bleiben deine Daten verschlüsselt. Es gilt nur für diese
          Sitzung — beim nächsten Start fragt die App erneut.
        </p>
        <form
          novalidate
          onsubmit={(e) => {
            e.preventDefault();
            doUnlock();
          }}
        >
          <label>
            Daten-Passwort
            <input
              bind:this={firstInput}
              type="password"
              bind:value={pass}
              autocomplete="current-password"
              disabled={busy}
            />
          </label>
          {#if error}<p class="err">{error}</p>{/if}
          <button type="submit" class="primary" disabled={busy}>
            {busy ? "Entsperre …" : "Öffnen"}
          </button>
        </form>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(17, 24, 39, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
  }
  /* Modal-Karte (Overlay): bewusst eigener Stil, nicht globale .card.
     Größere Tiefe, feste Breite, höherer Schatten — Overlay-Pattern. */
  .modal-card {
    background: var(--c-surface);
    border-radius: var(--r-xl);
    padding: 1.75rem;
    width: 32rem;
    max-width: calc(100vw - 2rem);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }
  .modal-card h2 {
    margin: 0 0 0.75rem;
  }
  .modal-card p {
    color: var(--c-text);
    font-size: 0.92rem;
    line-height: 1.5;
  }
  /* .warn-line entfernt — durch globales .caveat (tokens.css) ersetzt. */
  label {
    display: block;
    margin: 0.9rem 0 0.2rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: #374151;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.55rem 0.7rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 1rem;
  }
  .err {
    color: #991b1b;
    font-weight: 600;
  }
  button.primary {
    margin-top: 1rem;
    width: 100%;
    padding: 0.6rem;
    /* R5-012: Tailwind-Blau (#2563eb) durch Petrol-Marke ersetzt
       (Memory `feedback_design_direction`). Onboarding-Flow ist der
       allererste UI-Eindruck — der trug die falsche Farbe. */
    background: var(--c-primary-600);
    color: #fff;
    border: none;
    border-radius: 6px;
    font-size: 1rem;
    cursor: pointer;
  }
  button.primary:hover:not(:disabled) {
    background: var(--c-primary-700);
  }
  button.primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
