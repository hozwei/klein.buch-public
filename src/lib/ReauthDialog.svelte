<script lang="ts">
  // Re-Auth-Dialog (Plan W6): Passphrase + TOTP-Code (oder Einmal-Code)
  // bestätigen eine kritische Aktion; gilt 5 Minuten.
  //
  // U1: Fokusfalle mit Startfokus im Passwortfeld, Escape = Abbrechen
  // (nicht während der Prüfung), Rücksprung zum Auslöser; Anzeigen/Verbergen
  // des Passworts (ASVS V2.1.12); Einfügen und Passwortmanager bleiben
  // erlaubt (3.3.8). ≤ 600 px Vollbild-Blatt.
  //
  // U4 (V1-09, V4-03; ADR 0041 Nr. 163, Nr. 33): Titel nennt die Aktion
  // („Aktion bestätigen: Backup wiederherstellen“). Während der Prüfung bleiben Felder und
  // Knöpfe fokussierbar (`readonly`/`aria-disabled` statt `disabled`); nach einer
  // Ablehnung steht der Fokus wieder im Passwortfeld, die Meldung ist verknüpft.
  // Abbrechen löst keinen Fehler aus (Transport wirft `REAUTH_CANCELED`, `flash`
  // zeigt „Abgebrochen – nichts geändert.“ als neutralen Hinweis).
  import { onMount, tick } from "svelte";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { setReauthHandler } from "#lib/transport.ts";
  import { authReauth } from "#lib/api.ts";
  import { reauthStore, requestReauth, resolveReauth } from "#lib/reauth.svelte.ts";
  import { reauthActionLabel } from "#lib/labels.ts";

  let pass = $state("");
  let code = $state("");
  let useRecovery = $state(false);
  let showPass = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let passEl = $state<HTMLInputElement | null>(null);

  onMount(() => {
    setReauthHandler((url) => requestReauth(reauthActionLabel(url)));
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
    if (busy) return;
    error = null;
    // U7 (R3-03): Enter im Passwortfeld schickte die Bestätigung mit leerem Code ab
    // (Fehlversuch beim Server, Eingaben weg). Leere Felder ohne Anfrage abfangen.
    if (!pass) {
      error = "Gib dein Daten-Passwort ein.";
      await tick();
      passEl?.focus();
      return;
    }
    if (!code.trim()) {
      if (document.activeElement !== passEl) {
        error = useRecovery ? "Gib einen noch unbenutzten Einmal-Code ein." : "Gib den aktuellen Code aus der Authenticator-App ein.";
      }
      await tick();
      document.getElementById("reauth-code")?.focus();
      return;
    }
    busy = true;
    let failed = false;
    try {
      const factor = useRecovery ? { recoveryCode: code.trim() } : { totpCode: code.trim() };
      if (await authReauth(pass, factor)) {
        close(true);
      } else {
        failed = true;
        error = useRecovery
          ? "Bestätigung fehlgeschlagen. Gib dein Daten-Passwort und einen noch unbenutzten Einmal-Code neu ein."
          : "Bestätigung fehlgeschlagen. Gib dein Daten-Passwort und den aktuellen Code der Authenticator-App neu ein (er wechselt alle 30 Sekunden).";
      }
    } catch (e) {
      failed = true;
      error = String(e);
    } finally {
      pass = "";
      code = "";
      busy = false;
    }
    // Fokus nicht auf <body> verlieren (2.4.3): zurück ins erste Feld.
    if (failed && reauthStore.visible) {
      await tick();
      passEl?.focus();
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
          <h2 id="reauth-title">
            {reauthStore.action ? `Aktion bestätigen: ${reauthStore.action}` : "Aktion bestätigen"}
          </h2>
          <p id="reauth-desc">
            Diese Aktion ist geschützt. Bestätige sie mit deinem <strong>Daten-Passwort</strong>
            und dem <strong>App-Code</strong>; danach gilt die Bestätigung 5 Minuten lang auch für
            weitere geschützte Aktionen.
          </p>
          <label for="reauth-pass">Daten-Passwort</label>
          <div class="pw">
            <input
              id="reauth-pass"
              bind:this={passEl}
              type={showPass ? "text" : "password"}
              bind:value={pass}
              autocomplete="current-password"
              autocapitalize="off"
              spellcheck="false"
              enterkeyhint="next"
              aria-invalid={error ? "true" : undefined}
              aria-describedby={error ? "reauth-err" : undefined}
              readonly={busy}
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
            readonly={busy}
          />
          <button type="button" class="link switch" onclick={() => (useRecovery = !useRecovery)}>
            {useRecovery ? "Code aus der App verwenden" : "Einmal-Code verwenden"}
          </button>
          {#if error}<p id="reauth-err" class="err" role="alert">{error}</p>{/if}
        </div>
        <div class="kb-dialog-foot">
          <button
            type="button"
            class="btn-secondary"
            aria-disabled={busy || undefined}
            onclick={() => {
              if (!busy) close(false);
            }}
          >
            Abbrechen
          </button>
          <button type="submit" class="btn-primary" aria-disabled={busy || undefined} aria-busy={busy || undefined}>
            {busy ? "Wird geprüft …" : "Bestätigen"}
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
