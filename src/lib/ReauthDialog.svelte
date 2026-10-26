<script lang="ts">
  // Re-Auth-Dialog (Plan W6): Passphrase + TOTP-Code (oder Einmal-Code)
  // bestätigen eine kritische Aktion; gilt 5 Minuten.
  //
  // U1: Fokusfalle mit Startfokus im Passwortfeld, Escape = Abbrechen
  // (nicht während der Prüfung), Rücksprung zum Auslöser; Anzeigen/Verbergen
  // des Passworts (ASVS V2.1.12); Einfügen und Passwortmanager bleiben
  // erlaubt (3.3.8). ≤ 600 px Vollbild-Blatt.
  import { onMount } from "svelte";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { setReauthHandler } from "#lib/transport.ts";
  import { authReauth } from "#lib/api.ts";
  import { reauthStore, requestReauth, resolveReauth } from "#lib/reauth.svelte.ts";

  let pass = $state("");
  let code = $state("");
  let useRecovery = $state(false);
  let showPass = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);

  onMount(() => {
    setReauthHandler(requestReauth);
    return () => setReauthHandler(null);
  });

  $effect(() => {
    if (reauthStore.visible) {
      pushModal();
      return () => popModal();
    }
  });

  function close(ok: boolean) {
    pass = "";
    code = "";
    error = null;
    showPass = false;
    resolveReauth(ok);
  }

  async function confirm() {
    error = null;
    busy = true;
    try {
      const factor = useRecovery ? { recoveryCode: code.trim() } : { totpCode: code.trim() };
      if (await authReauth(pass, factor)) {
        close(true);
      } else {
        error = "Bestätigung fehlgeschlagen. Prüfe Passwort und Code und versuche es erneut.";
      }
    } catch (e) {
      error = String(e);
    } finally {
      pass = "";
      code = "";
      busy = false;
    }
  }
</script>

{#if reauthStore.visible}
  <div class="kb-backdrop reauth-backdrop" role="presentation">
    <div
      class="kb-dialog reauth"
      role="dialog"
      aria-modal="true"
      aria-labelledby="reauth-title"
      aria-describedby="reauth-desc"
      use:focusTrap={{
        initialFocus: "#reauth-pass",
        onEscape: () => {
          if (!busy) close(false);
        },
      }}
    >
      <form
        class="reauth-form"
        novalidate
        onsubmit={(e) => {
          e.preventDefault();
          confirm();
        }}
      >
        <div class="kb-dialog-body">
          <h2 id="reauth-title">Aktion bestätigen</h2>
          <p id="reauth-desc">
            Für diese Aktion bitte erneut mit <strong>Daten-Passwort</strong> und
            <strong>App-Code</strong> bestätigen. Die Bestätigung gilt 5 Minuten.
          </p>
          <label for="reauth-pass">Daten-Passwort</label>
          <div class="pw">
            <input
              id="reauth-pass"
              type={showPass ? "text" : "password"}
              bind:value={pass}
              autocomplete="current-password"
              autocapitalize="off"
              spellcheck="false"
              enterkeyhint="next"
              aria-invalid={error ? "true" : undefined}
              aria-describedby={error ? "reauth-err" : undefined}
              disabled={busy}
            />
            <button
              type="button"
              class="pw-toggle"
              aria-controls="reauth-pass"
              aria-label={showPass ? "Passwort verbergen" : "Passwort anzeigen"}
              onclick={() => (showPass = !showPass)}
            >{showPass ? "Verbergen" : "Anzeigen"}</button>
          </div>
          <label for="reauth-code">
            {useRecovery ? "Einmal-Code" : "Code aus der Authenticator-App"}
          </label>
          <input
            id="reauth-code"
            type="text"
            bind:value={code}
            inputmode={useRecovery ? "text" : "numeric"}
            autocomplete="one-time-code"
            autocapitalize="off"
            spellcheck="false"
            enterkeyhint="done"
            aria-invalid={error ? "true" : undefined}
            aria-describedby={error ? "reauth-err" : undefined}
            disabled={busy}
          />
          <button type="button" class="link switch" onclick={() => (useRecovery = !useRecovery)}>
            {useRecovery ? "Code aus der App verwenden" : "Einmal-Code verwenden"}
          </button>
          {#if error}<p id="reauth-err" class="err" role="alert">{error}</p>{/if}
        </div>
        <div class="kb-dialog-foot">
          <button type="button" class="btn-secondary" disabled={busy} onclick={() => close(false)}>
            Abbrechen
          </button>
          <button type="submit" class="btn-primary" disabled={busy} aria-busy={busy}>
            {busy ? "Prüfe …" : "Bestätigen"}
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .reauth-backdrop { z-index: 2100; }
  .reauth { --dialog-w: 28rem; }
  .reauth-form {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1 1 auto;
  }
  h2 {
    margin: 0 0 0.75rem;
    font-size: var(--fs-lg);
  }
  label {
    display: block;
    margin: 0.9rem 0 0.3rem;
    font-size: var(--fs-sm);
    font-weight: 600;
  }
  input {
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
  .switch { margin-top: var(--sp-3); }
  .err {
    color: var(--status-danger-fg);
    font-weight: 600;
    margin: var(--sp-3) 0 0;
  }
</style>
