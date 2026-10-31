<script lang="ts">
  // Plan W6 — Sicherheit & Anmeldung: zweiter Faktor (TOTP) neu einrichten,
  // Einmal-Codes neu erzeugen, Daten-Passwort ändern. Alle drei verlangen eine
  // frische Bestätigung (Re-Auth-Dialog öffnet sich automatisch).
  // Lauf F4a (W14-005): angemeldete Sitzungen, „andere abmelden“ (Re-Auth);
  // schlichte Darstellung, Feinschliff im UI/UX-Lauf. Neue Passwörter liegen
  // nur bis zum Absenden bzw. Abbrechen im Zustand der Seite.
  // Plan W12 — Versionsprüfung (opt-in, E14): Schalter für die tägliche
  // Abfrage der GitHub-Releases; Hinweise landen bei den Hinweisen.
  // U4 (V2-06/V2-07/V2-11): Knöpfe während eines Laufs `ariaDisabled` (Fokus bleibt),
  // Ladefehler je Abschnitt mit „Erneut laden“, „Abbrechen“ (secondary) vor der Hauptaktion.
  import { onMount, tick } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import { dateTime } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import { estimatePassphraseStrength } from "#lib/passphraseStrength.ts";
  import {
    authSecurity,
    authSessions,
    authRevokeOtherSessions,
    authTotpRenewOffer,
    authTotpRenewFinish,
    authRecoveryRenew,
    authChangePassphrase,
    PassphraseChangeUnclear,
    updateCheckGet,
    updateCheckSet,
    type AuthSession,
    type TotpOffer,
  } from "#lib/api.ts";
  import type { UpdateCheckSettings } from "#lib/types.ts";
  import { REAUTH_CANCELED } from "#lib/transport.ts";
  import { canCopyRecoveryCodes, copyRecoveryCodes, saveRecoveryCodes } from "#lib/recoveryCodes.ts";

  let updateCheck = $state<UpdateCheckSettings | null>(null);
  let recoveryLeft = $state<number | null>(null);
  let codes = $state<string[]>([]);
  // U7 (R3-09): Woher die angezeigten Codes kommen — „Notiert“ führt den Fokus dorthin zurück.
  let codesFrom = $state<"sec-renew-codes" | "sec-totp-start">("sec-renew-codes");

  /** Neue Codes sind da: Fokus auf die Überschrift, damit Screenreader sie ansagen (R3-09). */
  async function showCodes(list: string[], from: typeof codesFrom) {
    codes = list;
    codesFrom = from;
    codesInfo = "";
    await tick();
    document.getElementById("sec-codes-h")?.focus();
  }

  /** „Notiert“: Codes ausblenden, Fokus zurück zum auslösenden Knopf statt auf <body>. */
  async function codesNoted() {
    codes = [];
    await tick();
    document.getElementById(codesFrom)?.focus();
  }
  // U6-008: Kopieren/Speichern wie bei der Einrichtung (BackupGate, #lib/recoveryCodes.ts).
  let codesInfo = $state("");
  const canCopy = canCopyRecoveryCodes();
  let offer = $state<TotpOffer | null>(null);
  let totpCode = $state("");
  let newPass = $state("");
  let newPassConfirm = $state("");
  let busy = $state(false);
  let sessions = $state<AuthSession[] | null>(null);
  // U4 (V2-07): Ladefehler je Abschnitt mit „Erneut laden“ statt ewigem „Wird geladen …“.
  let recoveryError = $state<string | null>(null);
  let sessionsError = $state<string | null>(null);
  let updateError = $state<string | null>(null);
  // Lauf F8: Hinweis bei unklarem Ausgang des Passphrase-Wechsels.
  let passphraseUnclear = $state<string | null>(null);

  // U1: Feldfehler am Passwort statt Toast, Anzeigen-Schalter und
  // Stärke-Schätzung (ADR 0041 Nr. 13; gemeinsamer Helfer `passphraseStrength.ts`
  // mit BackupGate, nur Hinweis — verbindlich prüft der Server).
  let passError = $state("");
  let confirmError = $state("");
  let showNew = $state(false);
  let showConfirm = $state(false);
  let newPassEl: HTMLInputElement | undefined = $state();
  let confirmEl: HTMLInputElement | undefined = $state();
  let totpEl: HTMLInputElement | undefined = $state();

  const strength = $derived(estimatePassphraseStrength(newPass));
  const otherSessions = $derived(sessions ? sessions.filter((s) => !s.current).length : 0);
  onMount(() => {
    load();
    loadSessions();
    loadUpdateCheck();
  });

  async function loadSessions() {
    sessionsError = null;
    try {
      sessions = await authSessions();
    } catch (e) {
      if (!sessions) sessionsError = String(e);
      else flash(`Sitzungen neu laden fehlgeschlagen: ${String(e)}`, "error");
    }
  }

  async function revokeOthers() {
    if (busy) return;
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
      flash(`Sitzungen abmelden fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  function clearNewPass() {
    newPass = "";
    newPassConfirm = "";
  }

  async function load() {
    recoveryError = null;
    try {
      recoveryLeft = (await authSecurity()).recoveryLeft;
    } catch (e) {
      recoveryError = String(e);
    }
  }

  async function loadUpdateCheck() {
    updateError = null;
    try {
      updateCheck = await updateCheckGet();
    } catch (e) {
      if (!updateCheck) updateError = String(e);
      else flash(`Versionsprüfung laden fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
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
      flash(`Versionsprüfung umschalten fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      busy = false;
      await loadUpdateCheck();
      if (updateCheck) box.checked = updateCheck.enabled;
    }
  }


  async function renewRecovery() {
    if (busy) return;
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
      await showCodes(await authRecoveryRenew(), "sec-renew-codes");
      await load();
    } catch (e) {
      flash(`Einmal-Codes erzeugen fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  async function startTotp() {
    if (busy) return;
    busy = true;
    try {
      offer = await authTotpRenewOffer();
      totpCode = "";
    } catch (e) {
      flash(`Zweiten Faktor einrichten fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  async function finishTotp() {
    if (busy) return;
    // U7 (R3-03): leeren Code nicht abschicken; Fokus ins Code-Feld.
    if (!totpCode.trim()) {
      totpEl?.focus();
      return;
    }
    busy = true;
    try {
      const renewed = await authTotpRenewFinish(totpCode.trim());
      offer = null;
      await showCodes(renewed, "sec-totp-start");
      flash("Zweiter Faktor neu eingerichtet — die alte App-Verbindung gilt nicht mehr.", "ok");
      await load();
    } catch (e) {
      flash(`Zweiten Faktor einrichten fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      totpCode = "";
      busy = false;
    }
  }

  async function changePassphrase() {
    if (busy) return;
    passError = "";
    confirmError = "";
    if (newPass.length < 16) {
      passError = "Das neue Passwort muss mindestens 16 Zeichen haben — am besten 3–4 Wörter.";
      newPassEl?.focus();
      return;
    }
    if (newPass !== newPassConfirm) {
      clearNewPass();
      confirmError = "Die beiden Passwörter stimmen nicht überein. Bitte gib beide erneut ein.";
      newPassEl?.focus();
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
    passphraseUnclear = null;
    let keepFields = false;
    try {
      await authChangePassphrase(newPass);
      flash(
        "Passwort geändert. Ein Backup mit dem neuen Passwort wird erstellt (siehe Backup-Protokoll).",
        "ok",
      );
    } catch (e) {
      if (e instanceof PassphraseChangeUnclear) {
        // Nicht verwerfen: Die neue Passphrase kann schon gelten.
        passphraseUnclear = e.message;
        keepFields = true;
      } else if (String(e).includes(REAUTH_CANCELED)) {
        // Abgebrochene Bestätigung ist kein Fehler am Feld (V1-09): neutraler Hinweis.
        flash(String(e), "error");
      } else {
        // Ablehnung des Servers (z. B. zu häufiges Passwort) am Feld zeigen.
        passError = String(e);
      }
    } finally {
      if (!keepFields) clearNewPass();
      busy = false;
      await loadSessions();
      if (passError) newPassEl?.focus();
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Sicherheit & Anmeldung" />

<section class="card" aria-labelledby="sec-recovery-h">
  <h2 id="sec-recovery-h">Einmal-Codes</h2>
  <p>
    Einmal-Codes ersetzen den App-Code, falls das Gerät mit der Authenticator-App
    verloren geht. Noch übrig: <strong>{recoveryLeft ?? (recoveryError ? "unbekannt" : "…")}</strong>.
  </p>
  {#if recoveryError}
    <Banner title="Der Stand der Einmal-Codes konnte nicht geladen werden.">{recoveryError}
      {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
    </Banner>
  {/if}
  <Button id="sec-renew-codes" variant="secondary" ariaDisabled={busy} onclick={renewRecovery}>
    Neue Einmal-Codes erzeugen
  </Button>
</section>

{#if codes.length > 0}
  <section class="card" aria-labelledby="sec-codes-h">
    <h2 id="sec-codes-h" tabindex="-1">Neue Einmal-Codes</h2>
    <p>Jetzt notieren — sie werden nur einmal angezeigt.</p>
    <ul class="codes">
      {#each codes as c (c)}<li><code>{c}</code></li>{/each}
    </ul>
    <div class="actions">
      {#if canCopy}<Button variant="secondary" onclick={async () => (codesInfo = await copyRecoveryCodes(codes))}>Codes kopieren</Button>{/if}
      <Button variant="secondary" onclick={() => (codesInfo = saveRecoveryCodes(codes))}>Als Textdatei speichern</Button>
      <Button variant="secondary" onclick={codesNoted}>Notiert</Button>
    </div>
    <p class="codes-info" role="status">{codesInfo}</p>
  </section>
{/if}

<section class="card" aria-labelledby="sec-totp-h">
  <h2 id="sec-totp-h">Zweiter Faktor (Authenticator-App)</h2>
  {#if offer}
    <p>Neuen QR-Code scannen und den angezeigten Code eingeben.</p>
    <div class="qr" role="img" aria-label="QR-Code für die Authenticator-App">{@html offer.qrSvg}</div>
    <p>Schlüssel zum Abtippen: <code class="secret">{offer.secret}</code></p>
    <form
      novalidate
      onsubmit={(e) => {
        e.preventDefault();
        finishTotp();
      }}
    >
      <label for="sec-totp">Code aus der App</label>
      <input
        id="sec-totp"
        bind:this={totpEl}
        type="text"
        bind:value={totpCode}
        inputmode="numeric"
        autocomplete="one-time-code"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="done"
        aria-describedby="sec-totp-hint"
      />
      <p id="sec-totp-hint" class="hint">6 Ziffern, wie sie die App gerade anzeigt.</p>
      <div class="actions">
        <!-- U4 (V2-11): „Abbrechen“ (secondary) zuerst, Hauptaktion zuletzt. -->
        <Button variant="secondary" onclick={() => (offer = null)}>Abbrechen</Button>
        <Button type="submit" ariaDisabled={busy}>Neu einrichten</Button>
      </div>
    </form>
  {:else}
    <p>Neues Gerät oder neue App? Richte den zweiten Faktor neu ein.</p>
    <Button id="sec-totp-start" variant="secondary" ariaDisabled={busy} onclick={startTotp}>
      Zweiten Faktor neu einrichten
    </Button>
  {/if}
</section>

<section class="card" aria-labelledby="sec-sessions-h">
  <h2 id="sec-sessions-h">Angemeldete Sitzungen</h2>
  <p>
    Jede Anmeldung in einem Browser ist eine Sitzung. Unbekannte Einträge? Andere Sitzungen
    abmelden und das Daten-Passwort ändern. Die Kennung findest du im Server-Log als
    <code>sess=…</code> wieder.
  </p>
  {#if sessions}
    <table class="cards">
      <caption class="kb-visually-hidden">Angemeldete Sitzungen</caption>
      <thead>
        <tr><th>Kennung</th><th>Angemeldet</th><th>Zuletzt aktiv</th><th>Status</th></tr>
      </thead>
      <tbody>
        {#each sessions as s (s.id)}
          <tr class:current={s.current}>
            <td data-label="Kennung"><code class="sess">{s.id}</code></td>
            <td class="ts" data-label="Angemeldet">{dateTime(s.loginAt)}</td>
            <td class="ts" data-label="Zuletzt aktiv">{dateTime(s.lastSeenAt)}</td>
            <td data-label="Status">
              <span class="badges">
                {#if s.current}<Badge kind="tag">Diese Sitzung</Badge>{:else}<Badge kind="tag">Anderes Gerät oder Browser</Badge>{/if}
                {#if s.reauthActive}<Badge tone="info">Frisch bestätigt</Badge>{/if}
              </span>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <div class="actions">
      <Button
        variant="secondary"
        disabled={otherSessions === 0}
        ariaDisabled={busy}
        onclick={revokeOthers}
        ariaDescribedby={otherSessions === 0 ? "sec-sessions-none" : undefined}
      >
        Andere Sitzungen abmelden
      </Button>
      {#if otherSessions === 0}
        <span id="sec-sessions-none" class="muted">Nur diese Sitzung ist angemeldet.</span>
      {/if}
    </div>
  {:else if sessionsError}
    <Banner title="Die Sitzungen konnten nicht geladen werden.">{sessionsError}
      {#snippet actions()}<Button variant="secondary" onclick={loadSessions}>Erneut laden</Button>{/snippet}
    </Banner>
  {:else}
    <LoadingState />
  {/if}
</section>

<section class="card" aria-labelledby="sec-pass-h">
  <h2 id="sec-pass-h">Daten-Passwort ändern</h2>
  {#if passphraseUnclear}
    <!-- V6-R04 (ADR 0042 Nr. 16): sofort ansagen (role=alert) — offen ist, welche Passphrase
         gilt; Verlust = Totalverlust (ADR 0035). -->
    <Banner kind="warning" live="assertive" title="Ergebnis unklar.">
      {passphraseUnclear}
      Die Felder bleiben gefüllt, damit du die neue Passphrase notieren kannst.
    </Banner>
  {/if}
  <form
    novalidate
    onsubmit={(e) => {
      e.preventDefault();
      changePassphrase();
    }}
  >
    <label for="sec-new-pass">Neues Passwort (mindestens 16 Zeichen — am besten 3–4 Wörter)</label>
    <div class="pw">
      <input
        id="sec-new-pass"
        bind:this={newPassEl}
        type={showNew ? "text" : "password"}
        bind:value={newPass}
        autocomplete="new-password"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="next"
        aria-invalid={passError ? "true" : undefined}
        aria-describedby={[passError ? "sec-pass-err" : "", strength ? "sec-strength" : ""].filter(Boolean).join(" ") || undefined}
      />
      <button
        type="button"
        class="pw-toggle"
        aria-controls="sec-new-pass"
        aria-label={showNew ? "Passwort verbergen" : "Passwort anzeigen"}
        onclick={() => (showNew = !showNew)}
      >{showNew ? "Verbergen" : "Anzeigen"}</button>
    </div>
    {#if passError}<p id="sec-pass-err" class="err">{passError}</p>{/if}
    {#if strength}
      <div class="strength" data-level={strength.level}>
        <span class="bar" aria-hidden="true"><span></span><span></span><span></span></span>
        <span id="sec-strength">Stärke (Schätzung): {strength.text}</span>
      </div>
    {/if}
    <label for="sec-new-pass2">Neues Passwort wiederholen</label>
    <div class="pw">
      <input
        id="sec-new-pass2"
        bind:this={confirmEl}
        type={showConfirm ? "text" : "password"}
        bind:value={newPassConfirm}
        autocomplete="new-password"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="done"
        aria-invalid={confirmError ? "true" : undefined}
        aria-describedby={confirmError ? "sec-confirm-err" : undefined}
      />
      <button
        type="button"
        class="pw-toggle"
        aria-controls="sec-new-pass2"
        aria-label={showConfirm ? "Wiederholung verbergen" : "Wiederholung anzeigen"}
        onclick={() => (showConfirm = !showConfirm)}
      >{showConfirm ? "Verbergen" : "Anzeigen"}</button>
    </div>
    {#if confirmError}<p id="sec-confirm-err" class="err">{confirmError}</p>{/if}
    <div class="actions">
      <Button type="submit" ariaDisabled={busy}>Passwort ändern</Button>
    </div>
  </form>
</section>

<section class="card" aria-labelledby="sec-update-h">
  <h2 id="sec-update-h">Versionsprüfung</h2>
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
        aria-disabled={busy ? "true" : undefined}
        onclick={(e) => { if (busy) e.preventDefault(); }}
        onchange={(e) => toggleUpdateCheck(e.currentTarget)}
      />
      Nach neuen Versionen suchen
    </label>
    {#if updateCheck.enabled}
      <p class="muted">Zuletzt geprüft: {dateTime(updateCheck.lastCheckAt, "noch nie")}</p>
    {/if}
  {:else if updateError}
    <Banner title="Die Versionsprüfung konnte nicht geladen werden.">{updateError}
      {#snippet actions()}<Button variant="secondary" onclick={loadUpdateCheck}>Erneut laden</Button>{/snippet}
    </Banner>
  {:else}
    <LoadingState />
  {/if}
</section>

<style>
  label {
    display: block;
    margin: 0.75rem 0 0.25rem;
    font-weight: 600;
    font-size: var(--fs-sm);
  }
  input:not([type="checkbox"]) {
    display: block;
    width: 100%;
    max-width: 24rem;
    padding: 0.5rem 0.65rem;
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
    max-width: 30rem;
  }
  .pw input { flex: 1 1 auto; min-width: 0; max-width: none; }
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
  @media (hover: hover) {
    .pw-toggle:hover { background: var(--surface-hover); }
  }
  .strength {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
    font-size: var(--fs-sm);
    color: var(--text-muted);
  }
  .strength .bar { display: inline-flex; gap: 3px; flex: none; }
  .strength .bar span { width: 22px; height: 6px; border-radius: var(--r-pill); background: var(--line); }
  .strength[data-level="0"] .bar span:nth-child(-n + 1) { background: var(--status-danger-line); }
  .strength[data-level="1"] .bar span:nth-child(-n + 1) { background: var(--status-warning-line); }
  .strength[data-level="2"] .bar span:nth-child(-n + 2) { background: var(--status-success-line); }
  .strength[data-level="3"] .bar span { background: var(--status-success-fg); }
  .err { color: var(--status-danger-fg); font-weight: 600; font-size: var(--fs-sm); margin: 0.3rem 0 0; }
  .hint { color: var(--text-muted); font-size: var(--fs-sm); margin: 0.3rem 0 0; }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem 0.75rem; margin-top: 0.9rem; }
  .qr :global(svg) {
    width: min(220px, 100%);
    height: auto;
    aspect-ratio: 1;
  }
  .secret { overflow-wrap: anywhere; }
  /* U1-Sichtprüfung: Codes nie an den Bindestrichen umbrechen (Abtippen/Abschreiben);
     bis 420 px einspaltig, sonst blieb bei 320 px „aaaa-bbbb-/cccc“. */
  .codes { columns: 2; max-width: 24rem; }
  .codes code { white-space: nowrap; }
  @media (max-width: 420px) { .codes { columns: 1; } }
  .codes-info { margin: 0.5rem 0 0; min-height: 1.5em; color: var(--text-muted); font-size: var(--fs-sm); }
  .codes-info:empty { min-height: 0; margin: 0; }
  table { width: 100%; border-collapse: collapse; margin-top: 0.5rem; }
  th, td { text-align: left; font-size: var(--fs-sm); }
  tr.current td { background: var(--surface-hover); }
  .sess { font-family: var(--font-mono); font-size: var(--fs-sm); letter-spacing: .04em; }
  .ts { font-variant-numeric: tabular-nums; }
  .badges { display: inline-flex; flex-wrap: wrap; gap: 0.3rem; justify-content: flex-end; }
  label.check {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-height: var(--target);
    width: fit-content;
    font-weight: 400;
    font-size: var(--fs-base);
  }
  .muted { color: var(--text-muted); font-size: var(--fs-sm); }
</style>
