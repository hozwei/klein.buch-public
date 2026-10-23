<script lang="ts">
  // Plan W6 — Sicherheit & Anmeldung: zweiter Faktor (TOTP) neu einrichten,
  // Einmal-Codes neu erzeugen, Daten-Passwort ändern. Alle drei verlangen eine
  // frische Bestätigung (Re-Auth-Dialog öffnet sich automatisch).
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import {
    authSecurity,
    authTotpRenewOffer,
    authTotpRenewFinish,
    authRecoveryRenew,
    authChangePassphrase,
    type TotpOffer,
  } from "$lib/api";

  let recoveryLeft = $state<number | null>(null);
  let codes = $state<string[]>([]);
  let offer = $state<TotpOffer | null>(null);
  let totpCode = $state("");
  let newPass = $state("");
  let newPassConfirm = $state("");
  let busy = $state(false);

  onMount(load);

  async function load() {
    try {
      recoveryLeft = (await authSecurity()).recoveryLeft;
    } catch (e) {
      flash(String(e), "error");
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
      newPass = "";
      newPassConfirm = "";
      busy = false;
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
</style>
