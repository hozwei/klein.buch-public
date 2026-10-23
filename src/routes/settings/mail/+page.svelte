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
    mailAccountTestSaved,
    mailSendTest,
  } from "$lib/api";
  import type { MailAccount, MailAccountInput } from "$lib/types";
  import { date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import Toggle from "$lib/Toggle.svelte";

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
  let smtpPort = $state(587);
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
    original = editingOauth
      ? null
      : { host: smtpHost.trim(), port: smtpPort, user: smtpUser.trim(), tls: smtpUseTls };
    window.scrollTo({ top: document.body.scrollHeight, behavior: "smooth" });
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
      flash("Bitte das Passwort eingeben, um diese Verbindung zu testen.", "error");
      return;
    }
    testing = true;
    try {
      await mailAccountTestConnection({
        smtpHost: smtpHost.trim(),
        smtpPort,
        smtpUseTls,
        smtpUser: smtpUser.trim() || null,
        password: password || null,
      });
      flash("Verbindung erfolgreich.", "ok");
    } catch (e) {
      flash("Verbindung fehlgeschlagen: " + String(e), "error");
    } finally {
      testing = false;
    }
  }

  async function save() {
    if (!label || !fromName || !fromEmail) {
      flash("Bezeichnung, Absender-Name und Absender-Mail sind Pflicht.", "error");
      return;
    }
    if (!smtpHost.trim()) {
      flash("Der Server (SMTP-Host) ist Pflicht.", "error");
      return;
    }
    if (plaintextWithCredentials) {
      flash("Unverschlüsselt nur ohne Benutzer und Passwort — bitte TLS einschalten.", "error");
      return;
    }
    if ((needsNewPassword || (editingOauth && smtpUser.trim() !== "")) && password === "") {
      flash("Server, Port oder Benutzer geändert — bitte das Passwort neu eingeben.", "error");
      return;
    }
    saving = true;
    try {
      const input: MailAccountInput = {
        label,
        authType: "smtp_password",
        smtpHost: smtpHost.trim(),
        smtpPort,
        smtpUser: smtpUser.trim() || null,
        smtpUseTls,
        fromEmail,
        fromName,
        isDefault,
        oauthTenantId: null,
        oauthClientId: null,
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
  Richte hier dein Postfach ein, um Rechnungen und Angebote per E-Mail (SMTP)
  zu verschicken. Das Passwort wird verschlüsselt in der Datenbank dieser
  Instanz gespeichert — nie in Protokollen oder Exporten — und gilt nur für den
  eingetragenen Server, Port und Benutzer. Den lückenlosen Nachweis jedes
  Versands findest du oben rechts unter „E-Mail-Protokoll".
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
              {#if a.authType === "smtp_password"}
                SMTP ({a.smtpHost ?? "—"}:{a.smtpPort ?? "—"})
              {:else}
                Microsoft 365
              {/if}
            </td>
            <td>
              {#if a.authType === "smtp_password"}
                {a.smtpUseTls === 1 ? "verschlüsselt" : "unverschlüsselt"}
              {:else}
                <span class="status-warn">im Server-Betrieb nicht verfügbar</span>
              {/if}
            </td>
            <td>{a.isDefault === 1 ? "★" : ""}</td>
            <td>{date(a.lastUsedAt)}</td>
            <td class="row-actions">
              {#if a.authType === "smtp_password"}
                <button class="link" onclick={() => testSaved(a)} disabled={testingId === a.id}>
                  {testingId === a.id ? "Teste …" : "Testen"}
                </button>
                <button class="link" onclick={() => startEdit(a)}>Bearbeiten</button>
              {:else}
                <button class="link" onclick={() => startEdit(a)}>Auf SMTP umstellen</button>
              {/if}
              <button class="link danger" onclick={() => remove(a)}>Löschen</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if smtpAccounts.length > 0}
    <div class="testmail">
      <span class="testmail-label">Test-Mail senden:</span>
      {#if smtpAccounts.length > 1}
        <select bind:value={testAccountId}>
          {#each smtpAccounts as a (a.id)}
            <option value={a.id}>{a.label}{a.isDefault === 1 ? " ★" : ""}</option>
          {/each}
        </select>
      {/if}
      <input
        type="email"
        bind:value={testTo}
        placeholder="empfaenger@example.org"
        autocomplete="off"
      />
      <button class="btn-secondary" onclick={sendTest} disabled={sendingTest || !testTo}>
        {sendingTest ? "Sende …" : "Test-Mail senden"}
      </button>
    </div>
    <p class="muted hint">
      Verschickt eine kurze Test-Mail über das gewählte (gespeicherte) Konto —
      prüft den echten Versandweg mit dem gespeicherten Passwort.
    </p>
  {/if}
</section>

<section class="card">
  <h2>{editingId ? "Postfach bearbeiten" : "Neues Postfach"}</h2>

  {#if editingOauth}
    <Banner kind="info">
      Die Microsoft-Anmeldung steht im Server-Betrieb nicht zur Verfügung. Trage die
      SMTP-Daten deines Postfachs ein (bei Microsoft 365 z. B.
      <code>smtp.office365.com</code>, Port 587, falls SMTP-Versand im Tenant erlaubt
      ist); beim Speichern wird das Konto auf SMTP umgestellt.
    </Banner>
  {/if}

  <div class="grid">
    <label>
      Bezeichnung
      <input type="text" bind:value={label} placeholder="z. B. Geschäftspostfach" />
    </label>
    <label>
      Absender-Name
      <input type="text" bind:value={fromName} placeholder="z. B. Beispiel IT-Service" />
    </label>
    <label>
      Absender-E-Mail
      <input type="email" bind:value={fromEmail} placeholder="rechnung@example.org" />
    </label>

    <label>
      Server (SMTP-Host)
      <input type="text" bind:value={smtpHost} placeholder="z. B. smtp.example.org" />
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
        placeholder={needsNewPassword
          ? "neues Passwort erforderlich"
          : editingId && !editingOauth
            ? "leer lassen = unverändert"
            : "Passwort deines Postfachs"}
        autocomplete="new-password"
      />
    </label>

    <Toggle
      bind:checked={isDefault}
      label="Als Standard-Postfach"
    />
  </div>

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
    <button class="btn-secondary" onclick={testConnection} disabled={testing || !smtpHost.trim()}>
      {testing ? "Teste …" : "Verbindung testen"}
    </button>
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
  input[type="text"], input[type="email"], input[type="password"], input[type="number"] {
    padding: 0.45rem 0.55rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem;
  }
  .actions { display: flex; gap: 0.5rem; margin-top: 1rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { background: #f3f4f6; font-weight: 600; }
  tr.editing { background: #eff6ff; }
  .status-warn { color: #b45309; }
  .testmail { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; margin-top: 1rem; }
  .testmail-label { font-size: 0.9rem; color: #374151; font-weight: 600; }
  .testmail input[type="email"], .testmail select {
    padding: 0.45rem 0.55rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; min-width: 16rem;
  }
  .testmail select { min-width: 10rem; }
  .hint { font-size: 0.8rem; margin: 0.4rem 0 0; }
  .row-actions { display: flex; gap: 0.75rem; white-space: nowrap; flex-wrap: wrap; }
  .link { background: none; border: 0; padding: 0; color: #2563eb; cursor: pointer; font-size: 0.85rem; text-decoration: underline; }
  .link.danger { color: #b91c1c; }
  .link:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
</style>
