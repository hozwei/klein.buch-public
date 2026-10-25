<script lang="ts">
  // Plan W6 — Sicherheit & Anmeldung: zweiter Faktor (TOTP) neu einrichten,
  // Einmal-Codes neu erzeugen, Daten-Passwort ändern. Alle drei verlangen eine
  // frische Bestätigung (Re-Auth-Dialog öffnet sich automatisch).
  // Lauf F4a (W14-005): angemeldete Sitzungen, „andere abmelden“ (Re-Auth);
  // schlichte Darstellung, Feinschliff im UI/UX-Lauf. Neue Passwörter liegen
  // nur bis zum Absenden bzw. Abbrechen im Zustand der Seite.
  // Plan W12 — Versionsprüfung (opt-in, E14): Schalter für die tägliche
  // Abfrage der GitHub-Releases; Hinweise landen in der Inbox.
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { dateTime } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import {
    authSecurity,
    authSessions,
    authRevokeOtherSessions,
    authTotpRenewOffer,
    authTotpRenewFinish,
    authRecoveryRenew,
    authChangePassphrase,
    updateCheckGet,
    updateCheckSet,
    type AuthSession,
    type TotpOffer,
  } from "#lib/api.ts";
  import type { UpdateCheckSettings } from "#lib/types.ts";

  let updateCheck = $state<UpdateCheckSettings | null>(null);
  let recoveryLeft = $state<number | null>(null);
  let codes = $state<string[]>([]);
  let offer = $state<TotpOffer | null>(null);
  let totpCode = $state("");
  let newPass = $state("");
  let newPassConfirm = $state("");
  let busy = $state(false);
  let sessions = $state<AuthSession[] | null>(null);

  onMount(() => {
    load();
    loadSessions();
    loadUpdateCheck();
  });

  async function loadSessions() {
    try {
      sessions = await authSessions();
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function revokeOthers() {
    if (
      !(await confirmDialog({
        title: "Andere Sitzungen abmelden?",
        body: "Alle anderen Browser und Geräte werden sofort abgemeldet; diese Sitzung bleibt.",
        confirmLabel: "Abmelden",
        danger: true,
      }))
    ) {
      return;
    }
    busy = true;
    try {
      const n = await authRevokeOtherSessions();
      flash(n === 1 ? "1 andere Sitzung abgemeldet." : `${n} andere Sitzungen abgemeldet.`, "ok");
      await loadSessions();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  function clearNewPass() {
    newPass = "";
    newPassConfirm = "";
  }

  async function load() {
    try {
      recoveryLeft = (await authSecurity()).recoveryLeft;
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function loadUpdateCheck() {
    try {
      updateCheck = await updateCheckGet();
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function toggleUpdateCheck(box: HTMLInputElement) {
    const enabled = box.checked;
    busy = true;
    try {
      await updateCheckSet(enabled);
      flash(
        enabled
          ? "Versionsprüfung eingeschaltet — die erste Abfrage folgt in wenigen Minuten."
          : "Versionsprüfung ausgeschaltet.",
        "ok",
      );
    } catch (e) {
      // Die Checkbox ist nur einseitig gebunden: nach einem Fehler den
      // angezeigten Zustand ausdrücklich zurücksetzen (Svelte setzt ihn nicht
      // neu, wenn der geladene Wert gleich bleibt).
      box.checked = !enabled;
      flash(String(e), "error");
    } finally {
      busy = false;
      await loadUpdateCheck();
      if (updateCheck) box.checked = updateCheck.enabled;
    }
  }


  async function renewRecovery() {
    if (
      !(await confirmDialog({
        title: "Neue Einmal-Codes erzeugen?",
        body: "Die bisherigen Einmal-Codes werden ungültig.",
        confirmLabel: "Neu erzeugen",
      }))
    ) {
      return;
    }
    busy = true;
    try {
      codes = await authRecoveryRenew();
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function startTotp() {
    busy = true;
    try {
      offer = await authTotpRenewOffer();
      totpCode = "";
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function finishTotp() {
    busy = true;
    try {
      codes = await authTotpRenewFinish(totpCode.trim());
      offer = null;
      flash("Zweiter Faktor neu eingerichtet — die alte App-Verbindung gilt nicht mehr.", "ok");
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      totpCode = "";
      busy = false;
    }
  }

  async function changePassphrase() {
    if (newPass.length < 16) {
      flash("Das neue Passwort muss mindestens 16 Zeichen haben.", "error");
      return;
    }
    if (newPass !== newPassConfirm) {
      clearNewPass();
      flash("Die beiden Passwörter stimmen nicht überein.", "error");
      return;
    }
    if (
      !(await confirmDialog({
        title: "Daten-Passwort ändern?",
        body:
          "Die Datenbank wird mit dem neuen Passwort neu verschlüsselt; andere angemeldete " +
          "Geräte werden abgemeldet. Ältere Backups bleiben mit dem bisherigen Passwort " +
          "einspielbar — bewahre es dafür auf.",
        confirmLabel: "Ändern",
        danger: true,
      }))
    ) {
      clearNewPass();
      return;
    }
    busy = true;
    try {
      await authChangePassphrase(newPass);
      flash(
        "Passwort geändert. Ein Backup mit dem neuen Passwort wird erstellt (siehe Backup-Protokoll).",
        "ok",
      );
    } catch (e) {
      flash(String(e), "error");
    } finally {
      clearNewPass();
      busy = false;
      await loadSessions();
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Sicherheit & Anmeldung" />

<section class="card">
  <h2>Einmal-Codes</h2>
  <p>
    Einmal-Codes ersetzen den App-Code, falls das Gerät mit der Authenticator-App
    verloren geht. Noch übrig: <strong>{recoveryLeft ?? "…"}</strong>.
  </p>
  <button type="button" class="btn-secondary" disabled={busy} onclick={renewRecovery}>
    Neue Einmal-Codes erzeugen
  </button>
</section>

{#if codes.length > 0}
  <section class="card">
    <h2>Neue Einmal-Codes</h2>
    <p>Jetzt notieren — sie werden nur einmal angezeigt.</p>
    <ul class="codes">
      {#each codes as c (c)}<li><code>{c}</code></li>{/each}
    </ul>
    <button type="button" class="btn-secondary" onclick={() => (codes = [])}>Notiert</button>
  </section>
{/if}

<section class="card">
  <h2>Zweiter Faktor (Authenticator-App)</h2>
  {#if offer}
    <p>Neuen QR-Code scannen und den angezeigten Code eingeben.</p>
    <div class="qr">{@html offer.qrSvg}</div>
    <p>Schlüssel zum Abtippen: <code class="secret">{offer.secret}</code></p>
    <form
      novalidate
      onsubmit={(e) => {
        e.preventDefault();
        finishTotp();
      }}
    >
      <label>
        Code aus der App
        <input type="text" bind:value={totpCode} inputmode="numeric" autocomplete="one-time-code" />
      </label>
      <button type="submit" class="btn-primary" disabled={busy}>Neu einrichten</button>
      <button type="button" class="btn-ghost" onclick={() => (offer = null)}>Abbrechen</button>
    </form>
  {:else}
    <p>Neues Gerät oder neue App? Richte den zweiten Faktor neu ein.</p>
    <button type="button" class="btn-secondary" disabled={busy} onclick={startTotp}>
      Zweiten Faktor neu einrichten
    </button>
  {/if}
</section>

<section class="card">
  <h2>Angemeldete Sitzungen</h2>
  <p>
    Jede Anmeldung in einem Browser ist eine Sitzung. Unbekannte Einträge? Andere Sitzungen
    abmelden und das Daten-Passwort ändern.
  </p>
  {#if sessions}
    <ul class="sessions">
      {#each sessions as s (s.id)}
        <li>
          <code>{s.id}</code> · angemeldet {dateTime(s.loginAt)} · zuletzt aktiv
          {dateTime(s.lastSeenAt)}{#if s.current} · <strong>diese Sitzung</strong>{/if}
        </li>
      {/each}
    </ul>
    <button
      type="button"
      class="btn-secondary"
      disabled={busy || sessions.length < 2}
      onclick={revokeOthers}
    >
      Andere Sitzungen abmelden
    </button>
  {:else}
    <p class="muted">Lade …</p>
  {/if}
</section>

<section class="card">
  <h2>Daten-Passwort ändern</h2>
  <form
    novalidate
    onsubmit={(e) => {
      e.preventDefault();
      changePassphrase();
    }}
  >
    <label>
      Neues Passwort (mindestens 16 Zeichen)
      <input type="password" bind:value={newPass} autocomplete="new-password" />
    </label>
    <label>
      Neues Passwort wiederholen
      <input type="password" bind:value={newPassConfirm} autocomplete="new-password" />
    </label>
    <button type="submit" class="btn-primary" disabled={busy}>Passwort ändern</button>
  </form>
</section>

<section class="card">
  <h2>Versionsprüfung</h2>
  <p>
    Auf Wunsch fragt Klein.Buch höchstens einmal täglich bei GitHub nach, ob eine neue
    Version erschienen ist, und legt dann einen Hinweis an. Gesendet wird nur eine
    Anfrage mit festem Programmnamen — keine Daten über diese Instanz. Das Update selbst
    spielst du in deiner Container-Verwaltung ein (neues Image-Tag); vor einer
    Datenbank-Umstellung legt Klein.Buch beim Entsperren selbst ein Backup an.
  </p>
  {#if updateCheck}
    <label class="check">
      <input
        type="checkbox"
        checked={updateCheck.enabled}
        disabled={busy}
        onchange={(e) => toggleUpdateCheck(e.currentTarget)}
      />
      Nach neuen Versionen suchen
    </label>
    {#if updateCheck.enabled}
      <p class="muted">Zuletzt geprüft: {dateTime(updateCheck.lastCheckAt, "noch nie")}</p>
    {/if}
  {:else}
    <p class="muted">Lade …</p>
  {/if}
</section>

<style>
  .card {
    margin-bottom: 1rem;
  }
  label {
    display: block;
    margin: 0.6rem 0;
    font-weight: 600;
    font-size: 0.85rem;
  }
  input {
    display: block;
    width: 100%;
    max-width: 24rem;
    margin-top: 0.2rem;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--c-border);
    border-radius: 6px;
    font-size: 1rem;
  }
  .qr :global(svg) {
    width: 220px;
    height: 220px;
  }
  .secret {
    word-break: break-all;
  }
  .codes {
    columns: 2;
    max-width: 24rem;
  }
  .sessions {
    padding-left: 1.2rem;
    font-size: 0.9rem;
  }
  label.check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  label.check input {
    display: inline-block;
    width: auto;
    margin: 0;
    padding: 0;
  }
  .muted {
    color: var(--c-text-muted);
    font-size: 0.85rem;
  }
</style>
