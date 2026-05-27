<script lang="ts">
  // „Über Klein.Buch" — statischer Info-Dialog (Block G2-DOC.3.5).
  //
  // Hersteller-/Kontakt-, Versions-, Lizenz- und Drittanbieter-Block
  // gem. PRD-§G2-DOC.3.5. Wird genau einmal über das Root-Layout gemountet
  // und über `aboutModalStore` (siehe aboutModal.svelte.ts) ein-/ausgeblendet.
  //
  // Externe Links und der Lizenz-File-Pfad öffnen wir über das opener-Plugin
  // (Standard-Browser bzw. Standard-Programm des OS). Das Modal selbst hält
  // keine Eingaben — kein Form-Submit, kein Pflicht-State.

  import { onMount } from "svelte";
  import { openUrl, openPath } from "@tauri-apps/plugin-opener";
  import { aboutModalStore, closeAboutDialog } from "$lib/aboutModal.svelte";
  import { appInfo, thirdPartyLicensesPath } from "$lib/api";
  import { formatVersionDisplay } from "$lib/version";
  import { flash } from "$lib/toast.svelte";
  import { pushModal, popModal } from "$lib/modalStack.svelte";
  import type { AppInfo } from "$lib/types";

  let info = $state<AppInfo | null>(null);
  let infoError = $state<string | null>(null);
  let closeBtn = $state<HTMLButtonElement | null>(null);
  let triggerEl: HTMLElement | null = null;

  // App-Info beim ersten Mount einmal laden — die Werte sind Compile-Time-
  // konstant, ein Refresh pro App-Lauf reicht.
  onMount(async () => {
    try {
      info = await appInfo();
    } catch (e) {
      infoError = e instanceof Error ? e.message : String(e);
    }
  });

  // Beim Öffnen: aktuell fokussiertes Element merken, Close-Button fokussieren,
  // Modal-Stack pushen (R5-014: macht App-Shell `inert`).
  // Beim Schließen: Stack poppen, Fokus zurück auf den Trigger.
  $effect(() => {
    if (aboutModalStore.visible) {
      triggerEl = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      pushModal();
      // Im nächsten Tick fokussieren — `closeBtn` ist erst nach dem Render gesetzt.
      queueMicrotask(() => closeBtn?.focus());
      return () => {
        popModal();
        triggerEl?.focus();
        triggerEl = null;
      };
    }
  });

  function onKeydown(e: KeyboardEvent): void {
    if (!aboutModalStore.visible) return;
    if (e.key === "Escape") {
      e.preventDefault();
      closeAboutDialog();
    }
  }

  async function openExternal(url: string): Promise<void> {
    try {
      await openUrl(url);
    } catch (e) {
      flash(
        `Link konnte nicht geöffnet werden: ${e instanceof Error ? e.message : String(e)}`,
        "error",
      );
    }
  }

  async function openMail(): Promise<void> {
    await openExternal("mailto:schmidm@wildbach-computerhilfe.de");
  }

  async function openLicensesFile(): Promise<void> {
    try {
      const path = await thirdPartyLicensesPath();
      await openPath(path);
    } catch (e) {
      flash(
        `Drittanbieter-Lizenzliste konnte nicht geöffnet werden: ${e instanceof Error ? e.message : String(e)}`,
        "error",
      );
    }
  }

  // Drittanbieter-Hauptliste fest verdrahtet (Spec: sichtbar im Dialog selbst,
  // kein Verstecken hinter einem zusätzlichen Klick). Reihenfolge entspricht
  // den Gruppen in PRD §G2-DOC.3.5.
  type ThirdParty = { name: string; license: string; role: string };
  type ThirdPartyGroup = { label: string; items: ThirdParty[] };
  const thirdPartyGroups: ThirdPartyGroup[] = [
    {
      label: "Framework / Sprache",
      items: [
        { name: "Tauri 2", license: "MIT / Apache-2.0", role: "Desktop-Shell" },
        { name: "Rust-Toolchain", license: "MIT / Apache-2.0", role: "Backend" },
        { name: "Svelte 5", license: "MIT", role: "Frontend" },
      ],
    },
    {
      label: "Persistenz + Krypto",
      items: [
        { name: "SQLite", license: "Public Domain", role: "Datenbank-Engine" },
        { name: "SQLCipher", license: "BSD-3-Clause", role: "Verschlüsselung at Rest" },
        {
          name: "OpenSSL",
          license: "Apache-2.0",
          role: "Krypto-Primitives (via bundled-sqlcipher-vendored-openssl)",
        },
      ],
    },
    {
      label: "E-Rechnung + PDF",
      items: [
        { name: "Mustang Project", license: "Apache-2.0", role: "ZUGFeRD-/PDF-A-3-Erzeugung" },
        {
          name: "KoSIT-Validator",
          license: "Apache-2.0",
          role: "E-Rechnung-Prüfung gegen die offizielle XRechnung-Spezifikation",
        },
        { name: "Typst", license: "Apache-2.0", role: "PDF-Erzeugung" },
        {
          name: "OpenJDK",
          license: "GPL-2.0 mit Classpath-Exception",
          role: "jlink-JRE-Sidecar",
        },
      ],
    },
    {
      label: "Versand + Netz",
      items: [
        { name: "lettre", license: "MIT / Apache-2.0", role: "SMTP" },
        { name: "russh + russh-sftp", license: "Apache-2.0", role: "SFTP-Backup-Target" },
        { name: "oauth2", license: "MIT / Apache-2.0", role: "MS-Graph-Authentifizierung" },
      ],
    },
    {
      label: "OS-Integration",
      items: [
        { name: "keyring", license: "MIT / Apache-2.0", role: "OS-Keychain für Passwörter/Tokens" },
      ],
    },
    {
      label: "Handbuch-Frontend",
      items: [
        { name: "marked", license: "MIT", role: "Markdown-Renderer" },
        { name: "minisearch", license: "MIT", role: "Volltextsuche" },
      ],
    },
  ];
</script>

<svelte:window onkeydown={onKeydown} />

{#if aboutModalStore.visible}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closeAboutDialog();
    }}
  >
    <div
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="about-title"
    >
      <header>
        <h2 id="about-title">Über Klein.Buch</h2>
        <button
          bind:this={closeBtn}
          type="button"
          class="close"
          aria-label="Dialog schließen"
          onclick={closeAboutDialog}>×</button
        >
      </header>

      <div class="body">
        <!-- Hersteller / Kontakt — festverdrahtet, kein Settings-Lookup -->
        <section class="block">
          <h3>Hersteller</h3>
          <p class="addr">
            Wildbach Computerhilfe<br />
            Manuel Schmid<br />
            Wildbachstraße 2<br />
            84036 Landshut
          </p>
          <p class="links">
            <button
              type="button"
              class="linklike"
              onclick={() => openExternal("https://wildbach-computerhilfe.de")}
              >wildbach-computerhilfe.de</button
            >
            <span class="sep">·</span>
            <button type="button" class="linklike" onclick={openMail}
              >schmidm@wildbach-computerhilfe.de</button
            >
          </p>
        </section>

        <!-- Versions-Block -->
        <section class="block">
          <h3>Version</h3>
          {#if infoError}
            <p class="error">App-Info konnte nicht gelesen werden: {infoError}</p>
          {:else if info}
            <dl>
              <dt>App-Version</dt>
              <dd>
                {formatVersionDisplay(info.appVersion)}
                <span class="version-raw">({info.appVersion})</span>
              </dd>
              <dt>Schema-Version</dt>
              <dd>{info.schemaVersion}</dd>
              <dt>Build-Commit</dt>
              <dd><code>{info.buildCommit}</code></dd>
              <dt>Identifier</dt>
              <dd><code>{info.identifier}</code></dd>
            </dl>
          {:else}
            <p class="muted">Lade …</p>
          {/if}
        </section>

        <!-- Lizenz-Block (AGPL §13 — Source-Link Pflicht) -->
        <section class="block">
          <h3>Lizenz</h3>
          <p>
            Klein.Buch ist freie Software unter <strong>AGPL-3.0</strong>. Du
            darfst die Software nutzen, weitergeben und verändern. Wenn du eine
            veränderte Version öffentlich bereitstellst — z. B. als Webdienst —
            musst du den Quelltext mitliefern.
          </p>
          <p class="links">
            <button
              type="button"
              class="linklike"
              onclick={() =>
                openExternal(
                  "https://github.com/hozwei/klein.buch-public/blob/main/LICENSE",
                )}>Lizenztext (LICENSE)</button
            >
            <span class="sep">·</span>
            <button
              type="button"
              class="linklike"
              onclick={() =>
                openExternal("https://github.com/hozwei/klein.buch-public")}
              >Quellcode (github.com/hozwei/klein.buch-public)</button
            >
          </p>
        </section>

        <!-- Drittanbieter — Hauptliste sichtbar + Button zur vollen Liste -->
        <section class="block">
          <h3>Open-Source-Bibliotheken</h3>
          <p class="muted">
            Diese Projekte halten Klein.Buch am Leben. Die vollständige Liste
            mit Lizenztexten steht hinter dem Button am Ende.
          </p>
          {#each thirdPartyGroups as group (group.label)}
            <h4>{group.label}</h4>
            <ul class="tp-list">
              {#each group.items as item (item.name)}
                <li>
                  <strong>{item.name}</strong> — {item.license} — {item.role}
                </li>
              {/each}
            </ul>
          {/each}

          <p class="notice">
            <strong>Hinweis:</strong>
            <code>OpenJDK</code> wird unter GPL-2.0 mit Classpath-Exception
            ausgeliefert — die GPL schlägt durch die Sidecar-Trennung und die
            Classpath-Ausnahme nicht auf Klein.Buch durch. Für
            <code>Mustang</code> und den <code>KoSIT-Validator</code> liegen
            die Apache-NOTICE-Dateien im Sidecar-Bundle bei.
          </p>

          <button type="button" class="btn-secondary" onclick={openLicensesFile}>
            Vollständige Liste der Open-Source-Bibliotheken öffnen
          </button>
        </section>

        <!-- Footer-Disclaimer (Spec: identischer Wortlaut zu G3) -->
        <p class="footer-disclaimer">
          Klein.Buch ist ein Werkzeug, kein Steuerberater.
        </p>
      </div>

      <footer>
        <button type="button" class="btn-primary" onclick={closeAboutDialog}>
          Schließen
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  /* G2-UX.3.2 — macOS-Sheet-Look (parallel zu ConfirmDialog): gedimmter Backdrop
     mit weichem Blur, großzügiger Radius (--r-xl), zweilagiger XL-Schatten,
     Sheet-Drift mit Apple-Easing. Header-Trennlinie als Hairline (rgba). */
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(16, 40, 50, 0.32);
    -webkit-backdrop-filter: blur(6px) saturate(140%);
    backdrop-filter: blur(6px) saturate(140%);
    display: grid;
    place-items: center;
    z-index: 1000;
    padding: 2rem;
    box-sizing: border-box;
    animation: about-fade-in var(--t-base) var(--ease-apple);
  }
  .dialog {
    background: var(--c-surface);
    border-radius: var(--r-xl);
    box-shadow: var(--sh-xl);
    width: min(640px, 100%);
    max-height: calc(100vh - 4rem);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: about-sheet-in var(--t-slow) var(--ease-apple);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem 0.85rem;
    border-bottom: 1px solid rgba(16, 36, 44, 0.08);
  }
  @keyframes about-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes about-sheet-in {
    from { transform: translateY(16px) scale(0.97); opacity: 0; }
    to { transform: translateY(0) scale(1); opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .backdrop, .dialog { animation: none; }
  }
  header h2 {
    margin: 0;
    font-size: var(--fs-lg);
    color: var(--c-text);
  }
  .close {
    background: transparent;
    border: 0;
    font-size: 1.5rem;
    line-height: 1;
    padding: 0.15rem 0.55rem;
    border-radius: var(--r-md);
    cursor: pointer;
    color: var(--c-text-muted);
  }
  .close:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
  }
  .body {
    padding: 1rem 1.25rem 0.75rem;
    overflow-y: auto;
    color: var(--c-text);
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .block {
    padding: 0.5rem 0 1rem;
    border-bottom: 1px solid var(--c-border);
  }
  .block:last-of-type {
    border-bottom: 0;
  }
  .block h3 {
    margin: 0 0 0.4rem;
    font-size: var(--fs-md);
    color: var(--c-text);
  }
  .block h4 {
    margin: 0.85rem 0 0.25rem;
    font-size: var(--fs-sm);
    color: var(--c-text-subtle);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .addr {
    margin: 0.25rem 0 0.5rem;
  }
  .links {
    margin: 0.35rem 0 0;
  }
  .sep {
    color: var(--c-text-subtle);
    margin: 0 0.45rem;
  }
  .linklike {
    background: transparent;
    border: 0;
    padding: 0;
    color: var(--c-primary-700);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
  .linklike:hover {
    color: var(--c-primary-800);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    column-gap: 1rem;
    row-gap: 0.25rem;
    margin: 0.25rem 0 0;
  }
  dt {
    color: var(--c-text-subtle);
    font-weight: 600;
  }
  dd {
    margin: 0;
  }
  /* G4.1-Fix: Roh-Semver klein neben dem CalVer-Display anzeigen —
     für Support/Bug-Reports die exakte Cargo-Version sichtbar lassen. */
  .version-raw {
    color: var(--c-text-subtle);
    font-size: 0.85em;
    margin-left: 0.35rem;
  }
  code {
    background: var(--c-surface-2);
    padding: 0.05em 0.4em;
    border-radius: var(--r-sm);
    font-size: 0.9em;
  }
  .tp-list {
    margin: 0.25rem 0 0;
    padding-left: 1.1rem;
  }
  .tp-list li {
    margin-bottom: 0.2rem;
  }
  .notice {
    margin-top: 0.85rem;
    padding: 0.5rem 0.7rem;
    background: var(--c-surface-2);
    border-left: 3px solid var(--c-primary-300);
    border-radius: var(--r-sm);
    font-size: var(--fs-xs);
    color: var(--c-text-muted);
  }
  .muted {
    color: var(--c-text-muted);
    margin: 0.25rem 0;
  }
  .error {
    color: var(--c-danger-700, #b91c1c);
    margin: 0.25rem 0;
  }
  .footer-disclaimer {
    margin: 1rem 0 0.25rem;
    text-align: center;
    color: var(--c-text-subtle);
    font-size: var(--fs-xs);
    font-style: italic;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 0.75rem 1.25rem;
    border-top: 1px solid var(--c-border);
    background: var(--c-surface-2);
  }
</style>
