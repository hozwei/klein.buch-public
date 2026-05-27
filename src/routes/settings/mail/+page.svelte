<script lang="ts">
  import { onMount } from "svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import {
    mailAccountsList,
    mailAccountCreate,
    mailAccountUpdate,
    mailAccountDelete,
    mailAccountTestConnection,
    mailSendTest,
    mailOauthStatus,
    mailOauthConnect,
    mailOauthDisconnect,
  } from "$lib/api";
  import type { MailAccount, MailAccountInput, OauthStatus } from "$lib/types";
  import { date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import Toggle from "$lib/Toggle.svelte";

  let accounts = $state<MailAccount[]>([]);
  let statuses = $state<Record<string, OauthStatus>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);

  let saving = $state(false);
  let testing = $state(false);
  let connectingId = $state<string | null>(null);

  // Test-Mail-Steuerung (verschickt über ein gespeichertes Konto).
  let testTo = $state("");
  let testAccountId = $state("");
  let sendingTest = $state(false);

  // Wenn gesetzt, bearbeitet die Form diesen Account statt einen neuen anzulegen.
  let editingId = $state<string | null>(null);

  // Form-State.
  let authType = $state<"smtp_password" | "oauth_microsoft">("smtp_password");
  let label = $state("");
  let fromName = $state("");
  let fromEmail = $state("");
  let smtpHost = $state("");
  let smtpPort = $state(587);
  let smtpUser = $state("");
  let smtpUseTls = $state(true);
  let isDefault = $state(false);
  let password = $state("");
  // OAuth (Microsoft).
  let oauthTenantId = $state("");
  let oauthClientId = $state("");

  async function load() {
    loading = true;
    error = null;
    try {
      accounts = await mailAccountsList();
      // OAuth-Status der Microsoft-Konten nachladen.
      const sx: Record<string, OauthStatus> = {};
      for (const a of accounts) {
        if (a.authType === "oauth_microsoft") {
          try {
            sx[a.id] = await mailOauthStatus(a.id);
          } catch {
            // ignorieren — Status ist nur Anzeige
          }
        }
      }
      statuses = sx;
      // Test-Mail-Konto vorbelegen: Default-Konto, sonst erstes.
      if (!testAccountId || !accounts.some((a) => a.id === testAccountId)) {
        const def = accounts.find((a) => a.isDefault === 1) ?? accounts[0];
        testAccountId = def?.id ?? "";
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function sendTest() {
    if (!testAccountId) {
      flash("Bitte ein Postfach wählen.", "error");
      return;
    }
    if (!testTo.trim()) {
      flash("Bitte eine Empfänger-Adresse für die Test-Mail angeben.", "error");
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
    authType = "smtp_password";
    label = "";
    fromName = "";
    fromEmail = "";
    smtpHost = "";
    smtpPort = 587;
    smtpUser = "";
    smtpUseTls = true;
    isDefault = false;
    password = "";
    oauthTenantId = "";
    oauthClientId = "";
  }

  function startEdit(a: MailAccount) {
    editingId = a.id;
    authType = a.authType === "oauth_microsoft" ? "oauth_microsoft" : "smtp_password";
    label = a.label;
    fromName = a.fromName;
    fromEmail = a.fromEmail;
    smtpHost = a.smtpHost ?? "";
    smtpPort = a.smtpPort ?? 587;
    smtpUser = a.smtpUser ?? "";
    smtpUseTls = a.smtpUseTls === 1;
    isDefault = a.isDefault === 1;
    password = ""; // leer = unverändert
    oauthTenantId = a.oauthTenantId ?? "";
    oauthClientId = a.oauthClientId ?? "";
    window.scrollTo({ top: document.body.scrollHeight, behavior: "smooth" });
  }

  async function remove(a: MailAccount) {
    if (
      !(await confirmDialog({
        title: `Postfach „${a.label}" löschen?`,
        body:
          a.authType === "oauth_microsoft"
            ? "Die gespeicherte Microsoft-Anmeldung wird aus dem Schlüsselbund entfernt."
            : "Das zugehörige Passwort wird aus dem Schlüsselbund entfernt.",
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

  async function testConnection() {
    testing = true;
    try {
      await mailAccountTestConnection({
        smtpHost,
        smtpPort,
        smtpUseTls,
        smtpUser: smtpUser || null,
        password: password || null,
      });
      flash("Verbindung erfolgreich.", "ok");
    } catch (e) {
      flash("Verbindung fehlgeschlagen: " + String(e), "error");
    } finally {
      testing = false;
    }
  }

  async function connect(a: MailAccount) {
    connectingId = a.id;
    flash("Browser wird geöffnet — bitte bei Microsoft anmelden …", "ok");
    try {
      const st = await mailOauthConnect(a.id);
      statuses[a.id] = st;
      flash(`Verbunden als ${st.accountEmail ?? "Microsoft-Konto"}.`, "ok");
      await load();
    } catch (e) {
      flash("Microsoft-Verbindung fehlgeschlagen: " + String(e), "error");
    } finally {
      connectingId = null;
    }
  }

  async function disconnect(a: MailAccount) {
    if (
      !(await confirmDialog({
        title: `Microsoft-Verbindung von „${a.label}" trennen?`,
        body: "Bis zur erneuten Anmeldung kann über dieses Postfach nicht versendet werden.",
        confirmLabel: "Trennen",
        danger: true,
      }))
    ) {
      return;
    }
    try {
      await mailOauthDisconnect(a.id);
      flash("Microsoft-Verbindung getrennt.", "ok");
      await load();
    } catch (e) {
      flash("Trennen fehlgeschlagen: " + String(e), "error");
    }
  }

  async function save() {
    if (!label || !fromName || !fromEmail) {
      flash("Bezeichnung, Absender-Name und Absender-Mail sind Pflicht.", "error");
      return;
    }
    if (authType === "smtp_password" && !smtpHost) {
      flash("Für ein Passwort-Postfach ist der Server (SMTP-Host) Pflicht.", "error");
      return;
    }
    if (authType === "oauth_microsoft" && !oauthClientId.trim()) {
      flash("Für ein Microsoft-Konto ist die Anwendungs-(Client-)ID Pflicht.", "error");
      return;
    }
    saving = true;
    try {
      const isOauth = authType === "oauth_microsoft";
      const input: MailAccountInput = {
        label,
        authType,
        smtpHost: isOauth ? null : smtpHost,
        smtpPort: isOauth ? null : smtpPort,
        smtpUser: isOauth ? null : smtpUser || null,
        smtpUseTls,
        fromEmail,
        fromName,
        isDefault,
        oauthTenantId: isOauth ? oauthTenantId.trim() || null : null,
        oauthClientId: isOauth ? oauthClientId.trim() || null : null,
      };
      // password geht in den OS-Keychain, NIEMALS in die DB. Bei OAuth: kein Passwort.
      const pass = isOauth ? null : password || null;
      if (editingId) {
        await mailAccountUpdate(editingId, input, pass);
        flash("Postfach aktualisiert.", "ok");
      } else {
        await mailAccountCreate(input, pass);
        flash(
          isOauth
            ? "Microsoft-Konto angelegt. Jetzt oben in der Liste auf „Mit Microsoft verbinden“ klicken."
            : "Postfach gespeichert. Das Passwort liegt sicher im Schlüsselbund.",
          "ok",
        );
      }
      resetForm();
      await load();
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      saving = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="E-Mail-Versand">
  {#snippet actions()}
    <a class="btn-secondary btn-sm" href="/settings/mail-log">E-Mail-Protokoll</a>
    <HelpAnchor slug="konten-und-mail-versand" />
  {/snippet}
</PageBar>
<p class="muted">
  Richte hier dein Postfach ein, um Rechnungen und Angebote per E-Mail zu
  verschicken. Anmeldedaten (Passwort bzw. Microsoft-Anmeldung) werden sicher im
  Schlüsselbund deines Computers gespeichert — niemals in der App-Datenbank.
  Den lückenlosen Nachweis jedes Versands findest du oben rechts unter
  „E-Mail-Protokoll".
</p>

{#if error}
  <Banner>Fehler: {error}</Banner>
{/if}

<section class="card">
  <h2>Postfächer</h2>
  {#if loading}
    <p class="muted">Lade …</p>
  {:else if accounts.length === 0}
    <p class="muted">Noch kein Postfach eingerichtet.</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th>Bezeichnung</th>
          <th>Absender</th>
          <th>Typ</th>
          <th>Verbindung</th>
          <th>Standard</th>
          <th>Zuletzt genutzt</th>
          <th>Aktionen</th>
        </tr>
      </thead>
      <tbody>
        {#each accounts as a (a.id)}
          <tr class:editing={editingId === a.id}>
            <td>{a.label}</td>
            <td>{a.fromName} &lt;{a.fromEmail}&gt;</td>
            <td>
              {#if a.authType === "oauth_microsoft"}
                Microsoft 365
              {:else}
                Passwort ({a.smtpHost ?? "—"}:{a.smtpPort ?? "—"})
              {/if}
            </td>
            <td>
              {#if a.authType === "oauth_microsoft"}
                {#if statuses[a.id]?.connected}
                  <span class="status-ok"
                    >✓ verbunden{statuses[a.id]?.accountEmail
                      ? ` (${statuses[a.id]?.accountEmail})`
                      : ""}</span
                  >
                {:else}
                  <span class="status-warn">nicht verbunden</span>
                {/if}
              {:else}
                {a.smtpUseTls === 1 ? "verschlüsselt" : "unverschlüsselt"}
              {/if}
            </td>
            <td>{a.isDefault === 1 ? "★" : ""}</td>
            <td>{date(a.lastUsedAt)}</td>
            <td class="row-actions">
              {#if a.authType === "oauth_microsoft"}
                {#if statuses[a.id]?.connected}
                  <button class="link" onclick={() => connect(a)} disabled={connectingId === a.id}>
                    {connectingId === a.id ? "Verbinde …" : "Neu verbinden"}
                  </button>
                  <button class="link danger" onclick={() => disconnect(a)}>Trennen</button>
                {:else}
                  <button class="link" onclick={() => connect(a)} disabled={connectingId === a.id}>
                    {connectingId === a.id ? "Verbinde …" : "Mit Microsoft verbinden"}
                  </button>
                {/if}
              {/if}
              <button class="link" onclick={() => startEdit(a)}>Bearbeiten</button>
              <button class="link danger" onclick={() => remove(a)}>Löschen</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if accounts.length > 0}
    <div class="testmail">
      <span class="testmail-label">Test-Mail senden:</span>
      {#if accounts.length > 1}
        <select bind:value={testAccountId}>
          {#each accounts as a (a.id)}
            <option value={a.id}>{a.label}{a.isDefault === 1 ? " ★" : ""}</option>
          {/each}
        </select>
      {/if}
      <input
        type="email"
        bind:value={testTo}
        placeholder="empfaenger@example.com"
        autocomplete="off"
      />
      <button class="btn-secondary" onclick={sendTest} disabled={sendingTest || !testTo}>
        {sendingTest ? "Sende …" : "Test-Mail senden"}
      </button>
    </div>
    <p class="muted hint">
      Verschickt eine kurze Test-Mail über das gewählte (gespeicherte) Konto —
      prüft den echten Versandweg (SMTP-Passwort bzw. Microsoft-Anmeldung aus dem
      Schlüsselbund).
    </p>
  {/if}
</section>

<section class="card">
  <h2>{editingId ? "Postfach bearbeiten" : "Neues Postfach"}</h2>

  <fieldset class="authtype">
    <legend>Postfach-Typ</legend>
    <label class="radio">
      <input type="radio" bind:group={authType} value="smtp_password" />
      Klassisch mit Passwort (SMTP) — funktioniert mit fast jedem Anbieter
    </label>
    <label class="radio">
      <input type="radio" bind:group={authType} value="oauth_microsoft" />
      Microsoft 365 / Exchange Online — moderne Anmeldung ohne Passwort
    </label>
  </fieldset>

  <div class="grid">
    <label>
      Bezeichnung
      <input type="text" bind:value={label} placeholder="z. B. Wildbach Geschäftsmail" />
    </label>
    <label>
      Absender-Name
      <input type="text" bind:value={fromName} placeholder="Wildbach Computerhilfe" />
    </label>
    <label>
      Absender-E-Mail
      <input type="email" bind:value={fromEmail} placeholder="rechnung@wildbach-computerhilfe.de" />
    </label>

    {#if authType === "smtp_password"}
      <label>
        Server (SMTP-Host)
        <input type="text" bind:value={smtpHost} placeholder="z. B. smtp.gmx.net" />
      </label>
      <label>
        Port
        <input type="number" bind:value={smtpPort} min="1" max="65535" />
      </label>
      <label>
        Benutzername
        <input type="text" bind:value={smtpUser} placeholder="meist deine E-Mail-Adresse" />
      </label>
      <Toggle
        bind:checked={smtpUseTls}
        label="Verschlüsselte Verbindung (TLS) verwenden"
      />
      <label>
        Passwort
        <input
          type="password"
          bind:value={password}
          placeholder={editingId ? "leer lassen = unverändert" : "Passwort deines Postfachs"}
          autocomplete="off"
        />
      </label>
    {:else}
      <label>
        Verzeichnis-/Mandant-ID (tenant)
        <input
          type="text"
          bind:value={oauthTenantId}
          placeholder="z. B. contoso.onmicrosoft.com oder GUID — leer = common"
        />
      </label>
      <label>
        Anwendungs-(Client-)ID
        <input
          type="text"
          bind:value={oauthClientId}
          placeholder="GUID aus deiner Azure-/Entra-App-Registrierung"
          autocomplete="off"
        />
      </label>
    {/if}

    <Toggle
      bind:checked={isDefault}
      label="Als Standard-Postfach"
    />
  </div>

  {#if authType === "oauth_microsoft"}
    <Banner kind="info">
      Für Microsoft-Postfächer registrierst du einmalig eine eigene App im
      Microsoft-Entra-(Azure-)Portal. Die Verbindung selbst stellst du danach in der
      Liste oben über „Mit Microsoft verbinden" her.
    </Banner>
    <details class="help">
      <summary>So richtest du die Microsoft-App ein (Schritt für Schritt)</summary>

      <p><strong>1. Register an application</strong> (App registrieren)</p>
      <ul>
        <li>Name: z. B. <code>Klein.Buch Mailversand</code></li>
        <li>Supported account types: <strong>Single tenant only</strong> (nur dein Verzeichnis)</li>
        <li>Redirect URI: <strong>Public client/native (mobile…)</strong> → <code>http://localhost</code> → <strong>Register</strong></li>
      </ul>

      <p><strong>2. IDs notieren</strong> — linkes Menü → <strong>Overview</strong> (Übersicht)</p>
      <ul>
        <li><strong>Application (client) ID</strong> → unten ins Feld <em>Anwendungs-(Client-)ID</em></li>
        <li><strong>Directory (tenant) ID</strong> → ins Feld <em>Mandant-ID</em> (oder deine Domain, z. B. <code>wildbach-computerhilfe.de</code>)</li>
      </ul>

      <p><strong>3. Öffentliche Client-Flows aktivieren</strong> — linkes Menü → <strong>Authentication</strong> (Authentifizierung)</p>
      <ul>
        <li>Tab <strong>Redirect URI configuration</strong>: prüfen, dass <code>Mobile and desktop applications → http://localhost</code> steht (ist schon da).</li>
        <li>Tab <strong>Settings</strong> → <strong>Allow public client flows</strong> (Öffentliche Clientflows zulassen) auf <strong>Yes / Enabled</strong> → <strong>Save</strong>.</li>
        <li>Ohne diesen Schalter bricht der Login mit <code>AADSTS7000218</code> ab.</li>
      </ul>

      <p><strong>4. API-Berechtigungen</strong> — linkes Menü → <strong>API permissions</strong></p>
      <ul>
        <li><strong>+ Add a permission</strong> → <strong>Microsoft Graph</strong> → <strong>Delegated permissions</strong></li>
        <li>Hinzufügen: <code>Mail.Send</code>, <code>User.Read</code>, <code>offline_access</code></li>
        <li>Optional <strong>Grant admin consent</strong> klicken (bei restriktivem Tenant nötig, sonst <code>AADSTS65001</code>)</li>
        <li><strong>Kein</strong> Client-Secret anlegen — Public Client + PKCE braucht keins.</li>
      </ul>

      <p><strong>5. In Klein.Buch eintragen</strong> (unten in diesem Formular)</p>
      <ul>
        <li>Anwendungs-(Client-)ID aus Schritt 2</li>
        <li>Mandant-ID = Directory ID (bei Single-Tenant Pflicht, nicht leer lassen)</li>
        <li>Speichern → oben in der Liste „Mit Microsoft verbinden" → Browser-Login → Test-Mail</li>
      </ul>

      <p class="muted">
        Der Versand läuft über Microsoft Graph — unabhängig davon, ob in deinem
        Tenant der klassische SMTP-Versand aktiviert ist.
      </p>
    </details>
  {/if}

  <div class="actions">
    {#if authType === "smtp_password"}
      <button class="btn-secondary" onclick={testConnection} disabled={testing || !smtpHost}>
        {testing ? "Teste …" : "Verbindung testen"}
      </button>
    {/if}
    {#if editingId}
      <button class="btn-secondary" onclick={resetForm} disabled={saving}>Abbrechen</button>
    {/if}
    <button class="btn-primary" onclick={save} disabled={saving}>
      {saving ? "Speichere …" : editingId ? "Änderungen speichern" : "Postfach speichern"}
    </button>
  </div>
</section>

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 0.75rem 1rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #374151; gap: 0.25rem; }
  label.radio { flex-direction: row; align-items: center; gap: 0.5rem; }
  input[type="text"], input[type="email"], input[type="password"], input[type="number"] {
    padding: 0.45rem 0.55rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem;
  }
  fieldset.authtype { border: 1px solid #e5e7eb; border-radius: 6px; padding: 0.5rem 1rem 0.75rem; margin: 0 0 1rem; display: flex; flex-direction: column; gap: 0.4rem; }
  fieldset.authtype legend { font-size: 0.85rem; color: #374151; font-weight: 600; padding: 0 0.4rem; }
  .actions { display: flex; gap: 0.5rem; margin-top: 1rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { background: #f3f4f6; font-weight: 600; }
  tr.editing { background: #eff6ff; }
  .status-ok { color: #15803d; font-weight: 600; }
  .status-warn { color: #b45309; }
  .testmail { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-top: 1rem; }
  .testmail-label { font-size: 0.9rem; color: #374151; font-weight: 600; }
  .testmail input[type="email"], .testmail select {
    padding: 0.45rem 0.55rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; min-width: 16rem;
  }
  .testmail select { min-width: 10rem; }
  .hint { font-size: 0.8rem; margin: 0.4rem 0 0; }
  .help { margin-top: 0.75rem; font-size: 0.85rem; color: var(--c-text-muted); }
  .help summary {
    cursor: pointer; font-weight: 600; color: var(--c-primary-700);
    display: inline-block; padding: 6px 11px; list-style: none;
    border: 1px solid var(--c-border-strong); border-radius: var(--r-md); background: var(--c-surface);
  }
  .help summary::-webkit-details-marker { display: none; }
  .help summary:hover { background: var(--c-primary-50); border-color: var(--c-primary-300); }
  .help[open] summary { margin-bottom: 0.5rem; }
  .help ul { margin: 0.25rem 0 0.5rem; padding-left: 1.25rem; }
  .help p { margin: 0.7rem 0 0.2rem; }
  .help li { margin: 0.25rem 0; }
  .help code { background: #f3f4f6; padding: 0.05rem 0.3rem; border-radius: 3px; font-size: 0.85em; }
  .row-actions { display: flex; gap: 0.75rem; white-space: nowrap; flex-wrap: wrap; }
  .link { background: none; border: 0; padding: 0; color: #2563eb; cursor: pointer; font-size: 0.85rem; text-decoration: underline; }
  .link.danger { color: #b91c1c; }
  .link:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
</style>
