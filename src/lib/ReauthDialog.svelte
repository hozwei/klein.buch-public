<script lang="ts">
  // Re-Auth-Dialog (Plan W6): Passphrase + TOTP-Code (oder Einmal-Code)
  // bestätigen eine kritische Aktion; gilt 5 Minuten.
  import { onMount } from "svelte";
  import { pushModal, popModal } from "$lib/modalStack.svelte";
  import { setReauthHandler } from "$lib/transport";
  import { authReauth } from "$lib/api";
  import { reauthStore, requestReauth, resolveReauth } from "$lib/reauth.svelte";

  let pass = $state("");
  let code = $state("");
  let useRecovery = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let firstInput = $state<HTMLInputElement | null>(null);

  onMount(() => {
    setReauthHandler(requestReauth);
    return () => setReauthHandler(null);
  });

  $effect(() => {
    if (reauthStore.visible) {
      pushModal();
      queueMicrotask(() => firstInput?.focus());
      return () => popModal();
    }
  });

  function close(ok: boolean) {
    pass = "";
    code = "";
    error = null;
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
        error = "Bestätigung fehlgeschlagen.";
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
  <div class="overlay" role="dialog" aria-modal="true" aria-labelledby="reauth-title">
    <div class="modal-card">
      <h2 id="reauth-title">Aktion bestätigen</h2>
      <p>
        Für diese Aktion bitte erneut mit <strong>Daten-Passwort</strong> und
        <strong>App-Code</strong> bestätigen. Die Bestätigung gilt 5 Minuten.
      </p>
      <form
        novalidate
        onsubmit={(e) => {
          e.preventDefault();
          confirm();
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
          {useRecovery ? "Code aus der App verwenden" : "Einmal-Code verwenden"}
        </button>
        {#if error}<p class="err">{error}</p>{/if}
        <div class="actions">
          <button type="button" class="btn-secondary" disabled={busy} onclick={() => close(false)}>
            Abbrechen
          </button>
          <button type="submit" class="btn-primary" disabled={busy}>
            {busy ? "Prüfe …" : "Bestätigen"}
          </button>
        </div>
      </form>
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
    z-index: 2100;
  }
  .modal-card {
    background: var(--c-surface);
    border-radius: var(--r-xl);
    padding: 1.5rem;
    width: 28rem;
    max-width: calc(100vw - 2rem);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }
  h2 {
    margin: 0 0 0.75rem;
  }
  label {
    display: block;
    margin: 0.9rem 0 0.2rem;
    font-size: 0.85rem;
    font-weight: 600;
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
  button.link {
    background: none;
    border: none;
    padding: 0.3rem 0;
    color: var(--c-primary-700);
    cursor: pointer;
    font-size: 0.85rem;
    text-decoration: underline;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }
</style>
