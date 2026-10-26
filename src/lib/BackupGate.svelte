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
  //
  // U1 (Lauf 2026.11): Rahmen als Dialog-Blatt (tokens.css `.kb-dialog`,
  // ≤ 600 px Vollbild), Fokusfalle ohne Escape (die Maske lässt sich nicht
  // wegklicken), Anzeigen/Verbergen für Passwortfelder (ASVS V2.1.12) und
  // eine Stärke-Schätzung bei neuem Passwort (V2.1.8, nur Hinweis — ob das
  // Passwort angenommen wird, entscheidet der Server). Einfügen und
  // Passwortmanager bleiben erlaubt (3.3.8).
  import { onMount } from "svelte";
  import { appState } from "#lib/stores.svelte.ts";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import SourceNotice from "#lib/SourceNotice.svelte";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { onAuthRequired } from "#lib/transport.ts";
  import {
    authStatus,
    authSetupOffer,
    authSetupFinish,
    authLogin,
    type TotpOffer,
  } from "#lib/api.ts";

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
  let showPass = $state(false);
  let showConfirm = $state(false);

  // Stärke-Schätzung (Heuristik aus Länge, Wortzahl, Wiederholungen). Die
  // verbindlichen Regeln (Mindestlänge, Sperrliste, Muster) prüft der Server.
  type Strength = { level: 0 | 1 | 2 | 3; text: string };
  function estimateStrength(p: string): Strength | null {
    if (!p) return null;
    const len = [...p].length;
    const words = p.trim().split(/[\s\-_.,;:+\/]+/).filter((w) => w.length >= 3).length;
    const unique = new Set(p.toLowerCase()).size;
    if (len < 16) return { level: 0, text: `Zu kurz: ${len} von mindestens 16 Zeichen.` };
    if (unique < 6) return { level: 1, text: "Schwach: viele Wiederholungen. Nimm 3–4 zufällige Wörter." };
    if (len >= 28 || words >= 4) return { level: 3, text: "Stark." };
    if (len >= 20 || words >= 3) return { level: 2, text: "Gut." };
    return { level: 1, text: "Ausreichend. Länger ist besser, z. B. mit einem weiteren Wort." };
  }
  const strength = $derived(status === "setup" ? estimateStrength(pass) : null);

  $effect(() => {
    appState.ready = status === "ready";
  });

  // R5-014: Modal-Stack (App-Shell `inert`). Startfokus über `focusTrap`
  // (erstes Feld des jeweiligen Schritts, `data-initial-focus`).
  $effect(() => {
    if (status === "setup" || status === "totp_setup" || status === "login") {
      pushModal();
      return () => popModal();
    }
  });

  // Schrittwechsel (Token → QR → Einmal-Codes, Anmelden ↔ Einrichten): das
  // fokussierte Element verschwindet; Fokus auf das erste Feld des neuen Schritts.
  let gateEl = $state<HTMLDivElement | null>(null);
  $effect(() => {
    void step;
    void status;
    void offer;
    const el = gateEl;
    if (!el) return;
    queueMicrotask(() => {
      const a = document.activeElement;
      if (a && a !== document.body && el.contains(a)) return;
      el.querySelector<HTMLElement>("[data-initial-focus]")?.focus();
    });
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
    showPass = false;
    showConfirm = false;
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

{#snippet errorText()}
  {#if error}<p id="gate-err" class="err" role="alert">{error}</p>{/if}
{/snippet}

{#if status === "setup" || status === "totp_setup" || status === "login"}
  <div class="kb-backdrop gate-backdrop" role="presentation">
    <div
      class="kb-dialog gate"
      bind:this={gateEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby="backup-gate-title"
      use:focusTrap={{ returnFocus: false }}
    >
      <div class="kb-dialog-body">
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
          <label for="gate-pass">Daten-Passwort</label>
          <div class="pw">
            <input
              id="gate-pass"
              data-initial-focus
              type={showPass ? "text" : "password"}
              bind:value={pass}
              autocomplete="current-password"
              autocapitalize="off"
              spellcheck="false"
              enterkeyhint="next"
              aria-invalid={error ? "true" : undefined}
              aria-describedby={error ? "gate-err" : undefined}
              disabled={busy}
            />
            <button
              type="button"
              class="pw-toggle"
              aria-controls="gate-pass"
              aria-label={showPass ? "Passwort verbergen" : "Passwort anzeigen"}
              onclick={() => (showPass = !showPass)}
            >{showPass ? "Verbergen" : "Anzeigen"}</button>
          </div>
          <label for="gate-code">
            {useRecovery ? "Einmal-Code" : "Code aus der Authenticator-App"}
          </label>
          <input
            id="gate-code"
            type="text"
            bind:value={code}
            inputmode={useRecovery ? "text" : "numeric"}
            autocomplete="one-time-code"
            autocapitalize="off"
            spellcheck="false"
            enterkeyhint="go"
            aria-invalid={error ? "true" : undefined}
            aria-describedby={error ? "gate-err" : undefined}
            disabled={busy}
          />
          <button type="button" class="link switch" onclick={() => (useRecovery = !useRecovery)}>
            {useRecovery ? "Code aus der App verwenden" : "Gerät verloren? Einmal-Code verwenden"}
          </button>
          {@render errorText()}
          <button type="submit" class="primary" disabled={busy} aria-busy={busy}>
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
          <input type="checkbox" bind:checked={codesNoted} data-initial-focus />
          Ich habe die Codes sicher notiert.
        </label>
        <button type="button" class="primary" disabled={!codesNoted} onclick={done}>Weiter</button>
      {:else if step === "qr" && offer}
        <h2 id="backup-gate-title">Authenticator-App verbinden</h2>
        <p>
          Scanne den QR-Code mit deiner Authenticator-App (z. B. Aegis, 2FAS, Google
          Authenticator) und gib den angezeigten Code ein.
        </p>
        <div class="qr" role="img" aria-label="QR-Code für die Authenticator-App">{@html offer.qrSvg}</div>
        <p class="secret">Schlüssel zum Abtippen: <code>{offer.secret}</code></p>
        <form
          novalidate
          onsubmit={(e) => {
            e.preventDefault();
            finishSetup();
          }}
        >
          <label for="gate-totp">Code aus der App</label>
          <input
            id="gate-totp"
            data-initial-focus
            type="text"
            bind:value={code}
            inputmode="numeric"
            autocomplete="one-time-code"
            autocapitalize="off"
            spellcheck="false"
            enterkeyhint="done"
            aria-invalid={error ? "true" : undefined}
            aria-describedby={error ? "gate-err" : undefined}
            disabled={busy}
          />
          {@render errorText()}
          <button type="submit" class="primary" disabled={busy} aria-busy={busy}>
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
          <label for="gate-token">Setup-Token (aus dem Container-Log)</label>
          <input
            id="gate-token"
            data-initial-focus
            type="text"
            bind:value={token}
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            enterkeyhint="next"
            disabled={busy}
          />
          <label for="gate-pass">
            {status === "setup"
              ? "Passwort (mindestens 16 Zeichen — am besten 3–4 Wörter)"
              : "Daten-Passwort"}
          </label>
          <div class="pw">
            <input
              id="gate-pass"
              type={showPass ? "text" : "password"}
              bind:value={pass}
              autocomplete={status === "setup" ? "new-password" : "current-password"}
              autocapitalize="off"
              spellcheck="false"
              enterkeyhint="next"
              aria-invalid={error ? "true" : undefined}
              aria-describedby={[error ? "gate-err" : "", strength ? "gate-strength" : ""]
                .filter(Boolean)
                .join(" ") || undefined}
              disabled={busy}
            />
            <button
              type="button"
              class="pw-toggle"
              aria-controls="gate-pass"
              aria-label={showPass ? "Passwort verbergen" : "Passwort anzeigen"}
              onclick={() => (showPass = !showPass)}
            >{showPass ? "Verbergen" : "Anzeigen"}</button>
          </div>
          {#if strength}
            <div class="strength" data-level={strength.level}>
              <span class="bar" aria-hidden="true"><span></span><span></span><span></span></span>
              <span id="gate-strength" class="strength-text">
                Stärke (Schätzung): {strength.text}
              </span>
            </div>
          {/if}
          {#if status === "setup"}
            <label for="gate-confirm">Passwort wiederholen</label>
            <div class="pw">
              <input
                id="gate-confirm"
                type={showConfirm ? "text" : "password"}
                bind:value={confirm}
                autocomplete="new-password"
                autocapitalize="off"
                spellcheck="false"
                enterkeyhint="go"
                aria-invalid={error ? "true" : undefined}
                aria-describedby={error ? "gate-err" : undefined}
                disabled={busy}
              />
              <button
                type="button"
                class="pw-toggle"
                aria-controls="gate-confirm"
                aria-label={showConfirm ? "Wiederholung verbergen" : "Wiederholung anzeigen"}
                onclick={() => (showConfirm = !showConfirm)}
              >{showConfirm ? "Verbergen" : "Anzeigen"}</button>
            </div>
          {/if}
          {@render errorText()}
          <button type="submit" class="primary" disabled={busy} aria-busy={busy}>
            {busy ? "Prüfe …" : "Weiter zum QR-Code"}
          </button>
        </form>
      {/if}

      <!-- AGPL §13 (Plan W12): Version und Quellstand auch vor der Anmeldung -->
      <SourceNotice />
      </div>
    </div>
  </div>
{/if}

<style>
  .gate-backdrop { z-index: 2000; }
  .gate { --dialog-w: 32rem; }
  .gate h2 {
    margin: 0 0 0.75rem;
    font-size: var(--fs-xl);
  }
  .gate p {
    color: var(--text);
    font-size: var(--fs-base);
    line-height: 1.5;
  }
  label {
    display: block;
    margin: 0.9rem 0 0.3rem;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text);
  }
  label.check {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    min-height: var(--target);
    font-weight: 500;
  }
  input[type="password"],
  input[type="text"] {
    width: 100%;
    box-sizing: border-box;
    padding: 0.55rem 0.7rem;
    border: 1px solid var(--line-control);
    border-radius: var(--r-md);
    font: inherit;
    font-size: var(--fs-md);
    background: var(--surface-card);
    color: var(--text);
  }
  .pw {
    display: flex;
    gap: var(--sp-2);
    align-items: stretch;
  }
  .pw input { flex: 1 1 auto; min-width: 0; }
  .pw-toggle {
    flex: none;
    min-width: var(--target);
    min-height: var(--target);
    padding: 0 var(--sp-3);
    border: 1px solid var(--line-control);
    border-radius: var(--r-md);
    background: var(--surface-card);
    color: var(--accent-on-soft);
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 600;
    cursor: pointer;
  }
  .strength {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
    font-size: var(--fs-sm);
    color: var(--text-muted);
  }
  .strength .bar {
    display: inline-flex;
    gap: 3px;
    flex: none;
  }
  .strength .bar span {
    width: 22px;
    height: 6px;
    border-radius: var(--r-pill);
    background: var(--line);
  }
  .strength[data-level="0"] .bar span:nth-child(-n + 1) { background: var(--status-danger-line); }
  .strength[data-level="1"] .bar span:nth-child(-n + 1) { background: var(--status-warning-line); }
  .strength[data-level="2"] .bar span:nth-child(-n + 2) { background: var(--status-success-line); }
  .strength[data-level="3"] .bar span { background: var(--status-success-fg); }
  .err {
    color: var(--status-danger-fg);
    font-weight: 600;
  }
  .insecure {
    background: var(--status-warning-bg);
    border: 1px solid var(--status-warning-line);
    color: var(--status-warning-fg);
    border-radius: var(--r-md);
    padding: 0.5rem 0.75rem;
    font-weight: 600;
  }
  .qr {
    display: flex;
    justify-content: center;
    margin: 0.5rem 0;
  }
  .qr :global(svg) {
    width: min(220px, 100%);
    height: auto;
    aspect-ratio: 1;
  }
  .secret code {
    word-break: break-all;
  }
  .codes {
    columns: 2;
    padding-left: 1.2rem;
    font-size: var(--fs-md);
  }
  .codes li { white-space: nowrap; }
  /* U1: einspaltig bis 420 px, Codes nie an den Bindestrichen umbrechen. */
  @media (max-width: 420px) {
    .codes { columns: 1; }
  }
  .switch { margin-top: var(--sp-3); }
  button.primary {
    margin-top: 1rem;
    width: 100%;
    min-height: max(var(--target), 40px);
    padding: 0.6rem;
    background: var(--accent);
    color: var(--text-on-accent);
    border: 1px solid var(--accent);
    border-radius: var(--r-md);
    font: inherit;
    font-size: var(--fs-md);
    font-weight: 600;
    cursor: pointer;
  }
  @media (hover: hover) {
    button.primary:hover:not(:disabled) {
      background: var(--accent-hover);
    }
  }
  button.primary:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
