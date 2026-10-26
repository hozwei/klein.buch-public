<script lang="ts">
  import { onMount } from "svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import {
    mailAccountsList,
    mailAccountCreate,
    mailAccountUpdate,
    mailAccountDelete,
    mailAccountTestConnection,
    mailAccountTestSaved,
    mailSendTest,
  } from "#lib/api.ts";
  import type { MailAccount, MailAccountInput } from "#lib/types.ts";
  import { date } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import Toggle from "#lib/Toggle.svelte";
  import Badge from "#lib/Badge.svelte";
  import Button from "#lib/Button.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";

  let accounts = $state<MailAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let saving = $state(false);
  let testing = $state(false);
  let testingId = $state<string | null>(null);

  // Test-Mail-Steuerung (verschickt über ein gespeichertes Konto).
  let testTo = $state("");
  let testAccountId = $state("");
  let sendingTest = $state(false);

  // Wenn gesetzt, bearbeitet die Form diesen Account statt einen neuen anzulegen.
  let editingId = $state<string | null>(null);
  // Verbindung des bearbeiteten Kontos beim Öffnen: Das gespeicherte Passwort
  // gilt nur für (Server, Port, Benutzer) — Plan W8.
  let original = $state<{ host: string; port: number; user: string; tls: boolean } | null>(null);
  // Ein (aus der Desktop-Fassung übernommenes) Microsoft-Konto wird beim
  // Speichern auf SMTP umgestellt — die Microsoft-Anmeldung gibt es im
  // Server-Betrieb nicht (E9).
  let editingOauth = $state(false);

  // Form-State.
  let label = $state("");
  let fromName = $state("");
  let fromEmail = $state("");
  let smtpHost = $state("");
  let smtpPort = $state<number | null>(587);
  let smtpUser = $state("");
  let smtpUseTls = $state(true);
  let isDefault = $state(false);
  let password = $state("");

  const smtpAccounts = $derived(accounts.filter((a) => a.authType === "smtp_password"));
  const connectionChanged = $derived(
    original !== null &&
      (smtpHost.trim() !== original.host ||
        smtpPort !== original.port ||
        smtpUser.trim() !== original.user),
  );
  // Neues Passwort nötig: geänderte Verbindung mit Benutzer (das alte wird gelöscht).
  const needsNewPassword = $derived(
    editingId !== null && !editingOauth && connectionChanged && smtpUser.trim() !== "",
  );
  const plaintextWithCredentials = $derived(
    !smtpUseTls && (smtpUser.trim() !== "" || password !== ""),
  );

  async function load() {
    loading = true;
    error = null;
    try {
      accounts = await mailAccountsList();
      // Test-Mail-Konto vorbelegen: Default-Konto, sonst erstes SMTP-Konto.
      const usable = accounts.filter((a) => a.authType === "smtp_password");
      if (!testAccountId || !usable.some((a) => a.id === testAccountId)) {
        const def = usable.find((a) => a.isDefault === 1) ?? usable[0];
        testAccountId = def?.id ?? "";
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // U1: Feldfehler statt Toast (3.3.1), erstes fehlerhaftes Feld fokussieren.
  type FieldKey = "label" | "fromName" | "fromEmail" | "smtpHost" | "smtpPort" | "password";
  let fieldErrors = $state<Partial<Record<FieldKey, string>>>({});
  let testToError = $state("");
  let portInput: QuantityInput | undefined = $state();
  let formHeading: HTMLHeadingElement | undefined = $state();

  const FIELD_IDS: Record<FieldKey, string> = {
    label: "mail-label",
    fromName: "mail-from-name",
    fromEmail: "mail-from-email",
    smtpHost: "mail-host",
    smtpPort: "mail-port",
    password: "mail-pass",
  };
  function focusField(k: FieldKey) {
    if (k === "smtpPort") portInput?.focus();
    else document.getElementById(FIELD_IDS[k])?.focus();
  }
  /** Meldung des Servers zum Host (422 „Ungültiger Server-Name …“, Lauf F4) am Feld zeigen. */
  function hostErrorFrom(msg: string): boolean {
    if (/server-name|smtp-host|hostname/i.test(msg)) {
      fieldErrors = { ...fieldErrors, smtpHost: msg };
      focusField("smtpHost");
      return true;
    }
    return false;
  }

  /** Anderes Konto, das beim Speichern als Standard abgelöst würde (R5-SUB4-006). */
  const replacedDefault = $derived(
    isDefault ? accounts.find((a) => a.isDefault === 1 && a.id !== editingId) ?? null : null,
  );

  async function sendTest() {
    testToError = "";
    if (!testAccountId) {
      flash("Bitte ein Postfach wählen.", "error");
      return;
    }
    if (!testTo.trim()) {
      testToError = "Bitte gib eine Empfänger-Adresse für die Test-Mail an.";
      document.getElementById("mail-test-to")?.focus();
      return;
    }
    sendingTest = true;
    try {
      await mailSendTest(testAccountId, testTo.trim());
      flash(`Test-Mail an ${testTo.trim()} gesendet.`, "ok");
    } catch (e) {
      flash("Test-Mail fehlgeschlagen: " + String(e), "error");
    } finally {
      sendingTest = false;
    }
  }

  onMount(load);

  function resetForm() {
    editingId = null;
    original = null;
    editingOauth = false;
    label = "";
    fromName = "";
    fromEmail = "";
    smtpHost = "";
    smtpPort = 587;
    smtpUser = "";
    smtpUseTls = true;
    isDefault = false;
    password = "";
    fieldErrors = {};
  }

  function startEdit(a: MailAccount) {
    editingId = a.id;
    editingOauth = a.authType !== "smtp_password";
    label = a.label;
    fromName = a.fromName;
    fromEmail = a.fromEmail;
    smtpHost = a.smtpHost ?? "";
    smtpPort = a.smtpPort ?? 587;
    smtpUser = a.smtpUser ?? "";
    smtpUseTls = a.smtpUseTls === 1;
    isDefault = a.isDefault === 1;
    password = ""; // leer = unverändert
    fieldErrors = {};
    original = editingOauth
      ? null
      : { host: smtpHost.trim(), port: smtpPort ?? 587, user: smtpUser.trim(), tls: smtpUseTls };
    // Zum Formular springen und den Fokus auf seine Überschrift legen.
    requestAnimationFrame(() => {
      formHeading?.scrollIntoView({ behavior: "smooth", block: "start" });
      formHeading?.focus({ preventScroll: true });
    });
  }

  async function remove(a: MailAccount) {
    if (
      !(await confirmDialog({
        title: `Postfach „${a.label}" löschen?`,
        body: "Das gespeicherte Passwort wird mit gelöscht.",
        confirmLabel: "Löschen",
        danger: true,
      }))
    ) {
      return;
    }
    try {
      await mailAccountDelete(a.id);
      if (editingId === a.id) resetForm();
      flash("Postfach gelöscht.", "ok");
      await load();
    } catch (e) {
      flash("Löschen fehlgeschlagen: " + String(e), "error");
    }
  }

  /** Gespeichertes Konto mit seinem gespeicherten Passwort testen (nur per ID). */
  async function testSaved(a: MailAccount) {
    testingId = a.id;
    try {
      await mailAccountTestSaved(a.id);
      flash(`Verbindung von „${a.label}“ erfolgreich.`, "ok");
    } catch (e) {
      flash("Verbindung fehlgeschlagen: " + String(e), "error");
    } finally {
      testingId = null;
    }
  }

  async function testConnection() {
    fieldErrors = {};
    if (!smtpHost.trim()) {
      fieldErrors = { smtpHost: "Bitte gib den Server (SMTP-Host) ein." };
      focusField("smtpHost");
      return;
    }
    if (portInput && !portInput.validate()) {
      portInput.focus();
      return;
    }
    if (smtpPort == null) {
      fieldErrors = { smtpPort: "Bitte gib den Port ein, meist 587 oder 465." };
      focusField("smtpPort");
      return;
    }
    // Unveränderte Verbindung ohne neues Passwort: das gespeicherte Konto
    // testen. Das gespeicherte Passwort geht nie an einen anderen Server.
    if (
      editingId &&
      original &&
      !connectionChanged &&
      smtpUseTls === original.tls &&
      password === ""
    ) {
      testing = true;
      try {
        await mailAccountTestSaved(editingId);
        flash("Verbindung erfolgreich.", "ok");
      } catch (e) {
        flash("Verbindung fehlgeschlagen: " + String(e), "error");
      } finally {
        testing = false;
      }
      return;
    }
    if (smtpUser.trim() !== "" && password === "") {
      fieldErrors = { password: "Bitte gib das Passwort ein, um diese Verbindung zu testen." };
      focusField("password");
      return;
    }
    testing = true;
    try {
      await mailAccountTestConnection({
        smtpHost: smtpHost.trim(),
        smtpPort: smtpPort as number,
        smtpUseTls,
        smtpUser: smtpUser.trim() || null,
        password: password || null,
      });
      flash("Verbindung erfolgreich.", "ok");
    } catch (e) {
      const msg = String(e);
      if (!hostErrorFrom(msg)) flash("Verbindung fehlgeschlagen: " + msg, "error");
    } finally {
      testing = false;
    }
  }

  async function save() {
    const errs: Partial<Record<FieldKey, string>> = {};
    if (!label.trim()) errs.label = "Bitte gib eine Bezeichnung ein, z. B. „Geschäftspostfach“.";
    if (!fromName.trim()) errs.fromName = "Bitte gib den Absender-Namen ein.";
    if (!fromEmail.trim()) errs.fromEmail = "Bitte gib die Absender-E-Mail ein.";
    if (!smtpHost.trim()) errs.smtpHost = "Bitte gib den Server (SMTP-Host) ein.";
    const portOk = portInput ? portInput.validate() : true;
    if (portOk && smtpPort == null) errs.smtpPort = "Bitte gib den Port ein, meist 587 oder 465.";
    if (
      !plaintextWithCredentials &&
      (needsNewPassword || (editingOauth && smtpUser.trim() !== "")) &&
      password === ""
    ) {
      errs.password = "Server, Port oder Benutzer geändert — bitte gib das Passwort neu ein.";
    }
    fieldErrors = errs;
    const order: FieldKey[] = ["label", "fromName", "fromEmail", "smtpHost", "smtpPort", "password"];
    const first = order.find((k) => errs[k] || (k === "smtpPort" && !portOk));
    if (first) {
      focusField(first);
      return;
    }
    if (plaintextWithCredentials) {
      flash("Unverschlüsselt nur ohne Benutzer und Passwort — bitte TLS einschalten.", "error");
      return;
    }
    if (smtpPort == null) return;
    saving = true;
    try {
      const input: MailAccountInput = {
        label,
        authType: "smtp_password",
        smtpHost: smtpHost.trim(),
        smtpPort: smtpPort as number,
        smtpUser: smtpUser.trim() || null,
        smtpUseTls,
        fromEmail,
        fromName,
        isDefault,
      };
      // Das Passwort liegt verschlüsselt in der Datenbank der Instanz (Tabelle
      // `secrets`), nie im Konto-Datensatz, in Protokollen oder Exporten.
      const pass = password || null;
      if (editingId) {
        await mailAccountUpdate(editingId, input, pass);
        flash("Postfach aktualisiert.", "ok");
      } else {
        await mailAccountCreate(input, pass);
        flash("Postfach gespeichert.", "ok");
      }
      resetForm();
      await load();
    } catch (e) {
      const msg = String(e);
      if (!hostErrorFrom(msg)) flash("Speichern fehlgeschlagen: " + msg, "error");
    } finally {
      saving = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="E-Mail-Versand">
  {#snippet actions()}
    <Button variant="secondary" href="/settings/mail-log">E-Mail-Protokoll</Button>
    <HelpAnchor slug="konten-und-mail-versand" />
  {/snippet}
</PageBar>
<p class="lead">
  Richte hier dein Postfach ein, um Rechnungen und Angebote per E-Mail (SMTP)
  zu verschicken. Das Passwort wird verschlüsselt in der Datenbank dieser
  Instanz gespeichert — nie in Protokollen oder Exporten — und gilt nur für den
  eingetragenen Server, Port und Benutzer. Den lückenlosen Nachweis jedes
  Versands findest du unter <a class="inline" href="/settings/mail-log">E-Mail-Protokoll</a>.
</p>

{#if error}
  <Banner>Fehler: {error}</Banner>
{/if}

<section class="card" aria-labelledby="mail-accounts-h">
  <h2 id="mail-accounts-h">Postfächer</h2>
  {#if loading}
    <p class="muted" aria-busy="true">Lädt …</p>
  {:else if accounts.length === 0}
    <p class="muted">Noch kein Postfach eingerichtet. Trage unten unter „Neues Postfach“ die Daten deines Postfachs ein.</p>
  {:else}
    <table class="cards">
      <caption class="kb-visually-hidden">Eingerichtete Postfächer</caption>
      <thead>
        <tr>
          <th>Bezeichnung</th>
          <th>Absender</th>
          <th>Typ</th>
          <th>Verbindung</th>
          <th>Standard</th>
          <th>Zuletzt genutzt</th>
          <th><span class="kb-visually-hidden">Aktionen</span></th>
        </tr>
      </thead>
      <tbody>
        {#each accounts as a (a.id)}
          <tr class:editing={editingId === a.id}>
            <td data-label="Bezeichnung"><strong>{a.label}</strong></td>
            <td class="cards-full wrap" data-label="Absender">{a.fromName} &lt;{a.fromEmail}&gt;</td>
            <td class="wrap" data-label="Typ">
              {#if a.authType === "smtp_password"}
                SMTP ({a.smtpHost ?? "—"}:{a.smtpPort ?? "—"})
              {:else}
                Microsoft 365
              {/if}
            </td>
            <td data-label="Verbindung">
              {#if a.authType === "smtp_password"}
                {a.smtpUseTls === 1 ? "verschlüsselt" : "unverschlüsselt"}
              {:else}
                <span class="status-warn">im Server-Betrieb nicht verfügbar</span>
              {/if}
            </td>
            <td data-label="Standard">{#if a.isDefault === 1}<Badge tone="primary">Standard</Badge>{:else}—{/if}</td>
            <td data-label="Zuletzt genutzt">{date(a.lastUsedAt)}</td>
            <td>
              <div class="row-actions">
                {#if a.authType === "smtp_password"}
                  <Button variant="secondary" onclick={() => testSaved(a)} disabled={testingId === a.id} ariaLabel={`Verbindung testen, Postfach ${a.label}`}>
                    {testingId === a.id ? "Teste …" : "Testen"}
                  </Button>
                  <Button variant="secondary" onclick={() => startEdit(a)} ariaLabel={`Bearbeiten, Postfach ${a.label}`}>Bearbeiten</Button>
                {:else}
                  <Button variant="secondary" onclick={() => startEdit(a)} ariaLabel={`Auf SMTP umstellen, Postfach ${a.label}`}>Auf SMTP umstellen</Button>
                {/if}
                <Button variant="danger" onclick={() => remove(a)} ariaLabel={`Löschen, Postfach ${a.label}`}>Löschen</Button>
              </div>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if smtpAccounts.length > 0}
    <form class="testmail" novalidate onsubmit={(e) => { e.preventDefault(); sendTest(); }} aria-labelledby="mail-test-h">
      <h3 id="mail-test-h" class="testmail-h">Test-Mail senden</h3>
      <div class="testmail-row">
        {#if smtpAccounts.length > 1}
          <label class="field">
            Postfach
            <select bind:value={testAccountId}>
              {#each smtpAccounts as a (a.id)}
                <option value={a.id}>{a.label}{a.isDefault === 1 ? " (Standard)" : ""}</option>
              {/each}
            </select>
          </label>
        {/if}
        <div class="field grow">
          <label for="mail-test-to">Empfänger</label>
          <input
            id="mail-test-to"
            type="email"
            bind:value={testTo}
            placeholder="z. B. du@example.org"
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            enterkeyhint="send"
            aria-invalid={testToError ? "true" : undefined}
            aria-describedby={testToError ? "mail-test-to-err" : "mail-test-hint"}
          />
          {#if testToError}<span id="mail-test-to-err" class="err">{testToError}</span>{/if}
        </div>

        <div class="testmail-btn">
          <Button type="submit" variant="secondary" disabled={sendingTest}>
            {sendingTest ? "Sende …" : "Test-Mail senden"}
          </Button>
        </div>
      </div>
      <p id="mail-test-hint" class="muted hint">
        Verschickt eine kurze Test-Mail über das gewählte (gespeicherte) Konto —
        prüft den echten Versandweg mit dem gespeicherten Passwort.
      </p>
    </form>
  {/if}
</section>

<section class="card" aria-labelledby="mail-form-h">
  <h2 id="mail-form-h" tabindex="-1" bind:this={formHeading}>{editingId ? "Postfach bearbeiten" : "Neues Postfach"}</h2>

  {#if editingOauth}
    <Banner kind="info">
      Die Microsoft-Anmeldung steht im Server-Betrieb nicht zur Verfügung. Trage die
      SMTP-Daten deines Postfachs ein (bei Microsoft 365 z. B.
      <code>smtp.office365.com</code>, Port 587, falls SMTP-Versand im Tenant erlaubt
      ist); beim Speichern wird das Konto auf SMTP umgestellt.
    </Banner>
  {/if}

  <p class="muted legend"><span class="req" aria-hidden="true">*</span> Pflichtfeld</p>
  <form class="grid" novalidate onsubmit={(e) => { e.preventDefault(); save(); }} id="mail-form">
    <div class="f">
      <label for="mail-label">Bezeichnung<span class="req" aria-hidden="true">*</span></label>
      <input
        id="mail-label"
        type="text"
        bind:value={label}
        autocomplete="off"
        required
        aria-required="true"
        aria-invalid={fieldErrors.label ? "true" : undefined}
        aria-describedby={fieldErrors.label ? "mail-label-err" : undefined}
        placeholder="z. B. Geschäftspostfach"
      />
      {#if fieldErrors.label}<span id="mail-label-err" class="err">{fieldErrors.label}</span>{/if}
    </div>
    <div class="f">
      <label for="mail-from-name">Absender-Name<span class="req" aria-hidden="true">*</span></label>
      <input
        id="mail-from-name"
        type="text"
        bind:value={fromName}
        autocomplete="organization"
        required
        aria-required="true"
        aria-invalid={fieldErrors.fromName ? "true" : undefined}
        aria-describedby={fieldErrors.fromName ? "mail-from-name-err" : undefined}
        placeholder="z. B. Beispiel IT-Service"
      />
      {#if fieldErrors.fromName}<span id="mail-from-name-err" class="err">{fieldErrors.fromName}</span>{/if}
    </div>
    <div class="f">
      <label for="mail-from-email">Absender-E-Mail<span class="req" aria-hidden="true">*</span></label>
      <input
        id="mail-from-email"
        type="email"
        bind:value={fromEmail}
        autocomplete="email"
        autocapitalize="off"
        spellcheck="false"
        required
        aria-required="true"
        aria-invalid={fieldErrors.fromEmail ? "true" : undefined}
        aria-describedby={fieldErrors.fromEmail ? "mail-from-email-err" : undefined}
        placeholder="z. B. rechnung@example.org"
      />
      {#if fieldErrors.fromEmail}<span id="mail-from-email-err" class="err">{fieldErrors.fromEmail}</span>{/if}
    </div>

    <div class="f">
      <label for="mail-host">Server (SMTP-Host)<span class="req" aria-hidden="true">*</span></label>
      <input
        id="mail-host"
        type="text"
        bind:value={smtpHost}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        inputmode="url"
        required
        aria-required="true"
        aria-invalid={fieldErrors.smtpHost ? "true" : undefined}
        aria-describedby={fieldErrors.smtpHost ? "mail-host-err" : "mail-host-hint"}
        placeholder="z. B. smtp.example.org"
      />
      {#if fieldErrors.smtpHost}
        <span id="mail-host-err" class="err">{fieldErrors.smtpHost}</span>
      {:else}
        <span id="mail-host-hint" class="hint">Hostname oder IP-Adresse, ohne Port und ohne „smtp://“.</span>
      {/if}
    </div>
    <QuantityInput
      bind:this={portInput}
      bind:value={smtpPort}
      id="mail-port"
      label="Port"
      integer
      grouping={false}
      max={65535}
      noun="Port"
      required
      hint="Meist 587 (STARTTLS) oder 465 (TLS)."
      error={fieldErrors.smtpPort}
    />
    <div class="f">
      <label for="mail-user">Benutzername</label>
      <input
        id="mail-user"
        type="text"
        bind:value={smtpUser}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        aria-describedby="mail-user-hint"
      />
      <span id="mail-user-hint" class="hint">Meist deine E-Mail-Adresse.</span>
    </div>
    <Toggle
      bind:checked={smtpUseTls}
      label="Verschlüsselte Verbindung (TLS) verwenden"
    />
    <div class="f">
      <label for="mail-pass">Passwort</label>
      <input
        id="mail-pass"
        type="password"
        bind:value={password}
        autocomplete="new-password"
        autocapitalize="off"
        spellcheck="false"
        aria-invalid={fieldErrors.password ? "true" : undefined}
        aria-describedby={fieldErrors.password ? "mail-pass-err" : "mail-pass-hint"}
      />
      {#if fieldErrors.password}
        <span id="mail-pass-err" class="err">{fieldErrors.password}</span>
      {:else}
        <span id="mail-pass-hint" class="hint">
          {needsNewPassword
            ? "Neues Passwort erforderlich."
            : editingId && !editingOauth
              ? "Leer lassen = unverändert."
              : "Passwort deines Postfachs."}
        </span>
      {/if}
    </div>

    <div class="default-toggle">
      <Toggle
        bind:checked={isDefault}
        label="Als Standard-Postfach"
      />
      {#if replacedDefault}
        <p class="hint">Ersetzt beim Speichern das bisherige Standard-Postfach „{replacedDefault.label}“.</p>
      {/if}
    </div>
  </form>


  {#if needsNewPassword}
    <Banner kind="warning">
      Server, Port oder Benutzer geändert: Das gespeicherte Passwort gilt nur für die
      bisherige Verbindung und wird beim Speichern gelöscht. Bitte das Passwort für
      die neue Verbindung eingeben.
    </Banner>
  {/if}
  {#if plaintextWithCredentials}
    <Banner kind="warning">
      Unverschlüsselt (ohne TLS) nur ohne Benutzer und Passwort — sonst gingen die
      Zugangsdaten im Klartext übers Netz.
    </Banner>
  {/if}

  <div class="actions">
    <Button variant="secondary" onclick={testConnection} disabled={testing}>
      {testing ? "Teste …" : "Verbindung testen"}
    </Button>
    {#if editingId}
      <Button variant="secondary" onclick={resetForm} disabled={saving}>Abbrechen</Button>
    {/if}
    <Button type="submit" form="mail-form" disabled={saving}>
      {saving ? "Speichere …" : editingId ? "Änderungen speichern" : "Postfach speichern"}
    </Button>
  </div>
</section>

<style>
  /* .card / .lead — globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(260px, 100%), 1fr)); gap: 0.75rem 1rem; align-items: start; }
  label { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text); gap: 0.25rem; min-width: 0; }
  .f { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  input[type="text"], input[type="email"], input[type="password"], select {
    padding: 0.45rem 0.55rem; border: 1px solid var(--line-control); border-radius: var(--r-sm); font: inherit; font-weight: 400;
    min-height: var(--target); background: var(--surface-card); color: var(--text);
  }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .legend { font-size: var(--fs-sm); margin: 0 0 0.5rem; }
  .hint { font-weight: 400; color: var(--text-muted); font-size: var(--fs-sm); margin: 0; }
  .err { font-weight: 600; color: var(--status-danger-fg); font-size: var(--fs-sm); }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 1rem; }
  .inline { text-decoration: underline; }
  h2[tabindex="-1"]:focus { outline: none; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; font-size: var(--fs-sm); }
  tr.editing td { background: var(--surface-hover); }
  .wrap { overflow-wrap: anywhere; }
  .status-warn { color: var(--status-warning-fg); }
  .testmail { margin-top: 1.25rem; padding-top: 1rem; border-top: 1px solid var(--line); }
  .testmail-h { font-size: var(--fs-md); margin: 0 0 0.5rem; }
  .testmail-row { display: flex; gap: 0.75rem; align-items: flex-start; flex-wrap: wrap; }
  .testmail .field { flex: 0 1 14rem; }
  .testmail .field.grow { flex: 1 1 16rem; display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .testmail .field.grow > label { display: block; }
  .testmail-btn { align-self: flex-end; }
  .testmail .hint { margin: 0.4rem 0 0; }
  .default-toggle { display: flex; flex-direction: column; gap: 0.25rem; }
  .row-actions { display: flex; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }
  .muted { color: var(--text-muted); }
  @media (max-width: 820px) {
    td { overflow-wrap: anywhere; }
  }
</style>
