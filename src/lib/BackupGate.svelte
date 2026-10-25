<script lang="ts">
  // Anmelde-Maske (Block 4, G1-ENC; seit Plan W6 mit zweitem Faktor).
  //
  // Die Passphrase ist das Daten-Passwort: sie entschlüsselt die Datenbank
  // (SQLCipher, ADR 0035). Dazu kommt TOTP (E5). Abläufe:
  // - Einrichtung: Setup-Token aus dem Container-Log + Passphrase → QR-Code
  //   scannen → Code bestätigen → Einmal-Codes notieren.
  // - TOTP-Neueinrichtung (nach `totp-reset`): Setup-Token + bisherige
  //   Passphrase → QR → Code → Einmal-Codes.
  // - Anmeldung: Passphrase + TOTP-Code (oder Einmal-Code).
  // Meldet der Server später 401/423 (Sitzung abgelaufen, Neustart, DB
  // gesperrt), erscheint die Maske erneut. Solange sie offen ist, rendert das
  // Layout die App-Seiten nicht (appState.ready).
  import { onMount } from "svelte";
  import { appState } from "$lib/stores.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import SourceNotice from "$lib/SourceNotice.svelte";
  import { pushModal, popModal } from "$lib/modalStack.svelte";
  import { flash } from "$lib/toast.svelte";
  import { onAuthRequired } from "$lib/transport";
  import {
    authStatus,
    authSetupOffer,
    authSetupFinish,
    authLogin,
    type TotpOffer,
  } from "$lib/api";

  type Status = "loading" | "setup" | "totp_setup" | "login" | "ready";
  type SetupStep = "token" | "qr" | "codes";
  let status = $state<Status>("loading");
  let step = $state<SetupStep>("token");
  let insecureHttp = $state(false);
  let token = $state("");
  let pass = $state("");
  let confirm = $state("");
  let code = $state("");
  let useRecovery = $state(false);
  let offer = $state<TotpOffer | null>(null);
  let recoveryCodes = $state<string[]>([]);
  let codesNoted = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let firstInput = $state<HTMLInputElement | null>(null);

  $effect(() => {
    appState.ready = status === "ready";
  });

  // R5-014/R5-015: Modal-Stack (App-Shell `inert`) + Auto-Focus.
  $effect(() => {
    if (status === "setup" || status === "totp_setup" || status === "login") {
      pushModal();
      queueMicrotask(() => firstInput?.focus());
      return () => popModal();
    }
  });

  onMount(() => {
    refresh();
    // 401/423 aus einem beliebigen Aufruf → erneut anmelden.
    return onAuthRequired(() => {
      if (status === "ready" || status === "loading") refresh();
    });
  });

  // Review W6: Passphrase nie über eine unsichere Verbindung schicken (der
  // Server lehnt sie dort ab, aber dann war sie schon unterwegs).
  function insecureContext(): boolean {
    if (typeof window === "undefined" || window.isSecureContext || insecureHttp) return false;
    error =
      "Unsichere Verbindung: Anmelden nur über HTTPS (Reverse-Proxy) oder direkt am Server (localhost).";
    return true;
  }

  function resetFields() {
    pass = "";
    confirm = "";
    code = "";
  }

  async function refresh() {
    try {
      const s = await authStatus();
      insecureHttp = s.insecureHttp;
      step = "token";
      offer = null;
      // R5-S2-UI (Lauf F4a): kein Passwort über einen Zustandswechsel hinaus.
      resetFields();
      if (s.state === "setup_required") status = "setup";
      else if (s.state === "totp_setup_required") status = "totp_setup";
      else status = s.state === "ok" ? "ready" : "login";
    } catch (e) {
      error = String(e);
      status = "login";
    }
  }

  // Einrichtung Schritt 1: Token (+ Passphrase prüfen) → QR anfordern.
  async function requestOffer() {
    error = null;
    if (insecureContext()) return;
    if (status === "setup") {
      // R5-005: Passphrase-Floor 16 Zeichen; Backend prüft dasselbe.
      if (pass.length < 16) {
        error =
          "Das Passwort muss mindestens 16 Zeichen haben. " +
          "Tipp: Nimm 3–4 zufällige Wörter, die nichts mit dir oder deiner Firma zu tun haben.";
        return;
      }
      if (pass !== confirm) {
        error = "Die beiden Passwörter stimmen nicht überein.";
        return;
      }
    }
    if (token.trim() === "") {
      error = "Bitte den Setup-Token aus dem Container-Log eingeben.";
      return;
    }
    busy = true;
    try {
      // Lauf F4a: bei der Ersteinrichtung prüft der Server das neue Passwort
      // schon hier (Sperrliste, Muster), damit es vor dem QR-Code korrigierbar ist.
      offer = await authSetupOffer(token.trim(), status === "setup" ? pass : undefined);
      step = "qr";
      code = "";
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  // Einrichtung Schritt 2: Code bestätigen → DB anlegen bzw. TOTP neu.
  async function finishSetup() {
    error = null;
    busy = true;
    try {
      const r = await authSetupFinish(token.trim(), pass, code.trim());
      recoveryCodes = r.recoveryCodes;
      codesNoted = false;
      step = "codes";
      resetFields();
      token = "";
    } catch (e) {
      error = String(e);
      code = "";
    } finally {
      busy = false;
    }
  }

  function done() {
    resetFields();
    token = "";
    recoveryCodes = [];
    offer = null;
    step = "token";
    status = "ready";
  }

  async function doLogin() {
    error = null;
    if (insecureContext()) {
      resetFields();
      return;
    }
    busy = true;
    try {
      const factor = useRecovery ? { recoveryCode: code.trim() } : { totpCode: code.trim() };
      const r = await authLogin(pass, factor);
      if (r.ok) {
        status = "ready";
        if (r.failedAttempts && r.failedAttempts > 0) {
          flash(`${r.failedAttempts} Fehlversuch(e) seit der letzten Anmeldung.`, "error");
        }
        if (r.usedRecoveryCode) {
          flash(
            `Einmal-Code verbraucht — noch ${r.recoveryLeft ?? 0} übrig. Neue Codes unter Einstellungen → Sicherheit.`,
            "ok",
          );
        }
      } else {
        error = "Anmeldung fehlgeschlagen.";
      }
    } catch (e) {
      error = String(e);
      await refresh();
    } finally {
      // R5-006: Klartext nicht über die await-Grenze hinaus halten.
      resetFields();
      busy = false;
    }
  }
</script>

{#if status === "setup" || status === "totp_setup" || status === "login"}
  <div class="overlay" role="dialog" aria-modal="true" aria-labelledby="backup-gate-title">
    <div class="modal-card">
      {#if insecureHttp}
        <p class="insecure" role="alert">
          Unverschlüsselte Verbindung erlaubt (KLEIN_BUCH_INSECURE_HTTP) — nur für Tests.
        </p>
      {/if}

      {#if status === "login"}
        <h2 id="backup-gate-title">Anmelden</h2>
        <p>
          Gib dein <strong>Daten-Passwort</strong> und den aktuellen Code deiner
          Authenticator-App ein. Ohne beides bleiben deine Daten verschlüsselt.
        </p>
        <form
          novalidate
          onsubmit={(e) => {
            e.preventDefault();
            doLogin();
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
          <label>
            {useRecovery ? "Einmal-Code" : "Code aus der Authenticator-App"}
            <input
              type="text"
              bind:value={code}
              inputmode={useRecovery ? "text" : "numeric"}
              autocomplete="one-time-code"
              disabled={busy}
            />
          </label>
          <button type="button" class="link" onclick={() => (useRecovery = !useRecovery)}>
            {useRecovery ? "Code aus der App verwenden" : "Gerät verloren? Einmal-Code verwenden"}
          </button>
          {#if error}<p class="err">{error}</p>{/if}
          <button type="submit" class="primary" disabled={busy}>
            {busy ? "Melde an …" : "Anmelden"}
          </button>
        </form>
      {:else if step === "codes"}
        <h2 id="backup-gate-title">Einmal-Codes notieren</h2>
        <p>
          Mit diesen Codes kommst du hinein, falls dein Gerät mit der Authenticator-App
          verloren geht — jeder Code gilt <strong>einmal</strong> und ersetzt nur den
          App-Code, nicht das Passwort. Sie werden nur jetzt angezeigt.
        </p>
        <ul class="codes">
          {#each recoveryCodes as c (c)}<li><code>{c}</code></li>{/each}
        </ul>
        <label class="check">
          <input type="checkbox" bind:checked={codesNoted} />
          Ich habe die Codes sicher notiert.
        </label>
        <button type="button" class="primary" disabled={!codesNoted} onclick={done}>Weiter</button>
      {:else if step === "qr" && offer}
        <h2 id="backup-gate-title">Authenticator-App verbinden</h2>
        <p>
          Scanne den QR-Code mit deiner Authenticator-App (z. B. Aegis, 2FAS, Google
          Authenticator) und gib den angezeigten Code ein.
        </p>
        <div class="qr">{@html offer.qrSvg}</div>
        <p class="secret">Schlüssel zum Abtippen: <code>{offer.secret}</code></p>
        <form
          novalidate
          onsubmit={(e) => {
            e.preventDefault();
            finishSetup();
          }}
        >
          <label>
            Code aus der App
            <input
              bind:this={firstInput}
              type="text"
              bind:value={code}
              inputmode="numeric"
              autocomplete="one-time-code"
              disabled={busy}
            />
          </label>
          {#if error}<p class="err">{error}</p>{/if}
          <button type="submit" class="primary" disabled={busy}>
            {busy
              ? "Richte ein …"
              : status === "setup"
                ? "Einrichten & erstes Backup erstellen"
                : "Zweiten Faktor einrichten"}
          </button>
        </form>
      {:else}
        <h2 id="backup-gate-title">
          {status === "setup" ? "Klein.Buch einrichten" : "Zweiten Faktor neu einrichten"}
          <HelpAnchor slug="passphrase-einrichten" />
        </h2>
        {#if status === "setup"}
          <p>
            Leg ein <strong>Daten-Passwort</strong> fest. Damit werden deine gesamte
            Buchhaltung und deine Backups verschlüsselt; du brauchst es bei jeder
            Anmeldung.
          </p>
          <p class="caveat">
            Wichtig: Dieses Passwort wird <strong>nirgends gespeichert</strong> und kann
            <strong>nicht zurückgesetzt</strong> werden. Vergisst du es, kommst du an
            deine Daten und Backups nicht mehr heran.
          </p>
        {:else}
          <p>
            Der zweite Faktor wurde zurückgesetzt. Gib den Setup-Token aus dem
            Container-Log und dein bisheriges <strong>Daten-Passwort</strong> ein.
          </p>
        {/if}
        <form
          novalidate
          onsubmit={(e) => {
            e.preventDefault();
            requestOffer();
          }}
        >
          <label>
            Setup-Token (aus dem Container-Log)
            <input
              bind:this={firstInput}
              type="text"
              bind:value={token}
              autocomplete="off"
              spellcheck="false"
              disabled={busy}
            />
          </label>
          <label>
            {status === "setup"
              ? "Passwort (mindestens 16 Zeichen — am besten 3–4 Wörter)"
              : "Daten-Passwort"}
            <input
              type="password"
              bind:value={pass}
              autocomplete={status === "setup" ? "new-password" : "current-password"}
              disabled={busy}
            />
          </label>
          {#if status === "setup"}
            <label>
              Passwort wiederholen
              <input
                type="password"
                bind:value={confirm}
                autocomplete="new-password"
                disabled={busy}
              />
            </label>
          {/if}
          {#if error}<p class="err">{error}</p>{/if}
          <button type="submit" class="primary" disabled={busy}>
            {busy ? "Prüfe …" : "Weiter zum QR-Code"}
          </button>
        </form>
      {/if}

      <!-- AGPL §13 (Plan W12): Version und Quellstand auch vor der Anmeldung -->
      <SourceNotice />
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
  /* Modal-Karte (Overlay): bewusst eigener Stil, nicht globale .card. */
  .modal-card {
    background: var(--c-surface);
    border-radius: var(--r-xl);
    padding: 1.75rem;
    width: 32rem;
    max-width: calc(100vw - 2rem);
    max-height: calc(100vh - 2rem);
    max-height: calc(100dvh - 2rem);
    overflow-y: auto;
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
  label {
    display: block;
    margin: 0.9rem 0 0.2rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: #374151;
  }
  label.check {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    font-weight: 500;
  }
  input[type="password"],
  input[type="text"] {
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
  .insecure {
    background: #fef3c7;
    border: 1px solid #f59e0b;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-weight: 600;
  }
  .qr {
    display: flex;
    justify-content: center;
    margin: 0.5rem 0;
  }
  .qr :global(svg) {
    width: 220px;
    height: 220px;
  }
  .secret code {
    word-break: break-all;
  }
  .codes {
    columns: 2;
    padding-left: 1.2rem;
    font-size: 1rem;
  }
  button.link {
    background: none;
    border: none;
    padding: 0.3rem 0;
    color: var(--c-primary-700);
    cursor: pointer;
    font-size: 0.85rem;
    text-decoration: underline;
  }
  button.primary {
    margin-top: 1rem;
    width: 100%;
    padding: 0.6rem;
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
