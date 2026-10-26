<script lang="ts">
  // „Über Klein.Buch" — statischer Info-Dialog (Block G2-DOC.3.5).
  //
  // Hersteller-/Kontakt-, Versions-, Lizenz- und Drittanbieter-Block
  // gem. PRD-§G2-DOC.3.5. Wird genau einmal über das Root-Layout gemountet
  // und über `aboutModalStore` (siehe aboutModal.svelte.ts) ein-/ausgeblendet.
  //
  // Externe Links sind normale Links (neuer Tab). Quell-URL, Commit, Tag und
  // der Link auf den exakten Quellstand kommen vom Server (`/api/public/info`,
  // AGPL §13 — Forks setzen KLEIN_BUCH_SOURCE_URL); ohne bekannten Commit
  // steht „Entwicklungs-Build, Quellstand unbekannt" (Plan W12). Lizenztexte
  // und Hinweise liefert der Server unter `/legal/` aus.
  // Das Modal selbst hält keine Eingaben — kein Form-Submit, kein Pflicht-State.

  import { aboutModalStore, closeAboutDialog } from "#lib/aboutModal.svelte.ts";
  import { appInfo } from "#lib/api.ts";
  import { publicInfo, type PublicInfo } from "#lib/transport.ts";
  import { formatVersionDisplay } from "#lib/version.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { DEV_SOURCE_TEXT, safeSourceLink, shortCommit, sourceLabel } from "#lib/sourceInfo.ts";
  import type { AppInfo } from "#lib/types.ts";

  let info = $state<AppInfo | null>(null);
  let infoError = $state<string | null>(null);
  let source = $state<PublicInfo | null>(null);
  let sourceError = $state(false);
  // Quellangaben aus /api/public/info; scheitert die Abfrage (z. B. 429), aus
  // app_info (gleiche Quelle im Server) — nie fälschlich „Entwicklungs-Build“.
  const effectiveSource = $derived<PublicInfo | null>(
    source ??
      (info
        ? {
            version: info.appVersion,
            commit: info.buildCommit,
            tag: info.buildTag,
            sourceUrl: info.repositoryUrl,
            sourceLink: info.sourceLink,
            licenseSpdx: info.licenseSpdx,
          }
        : null),
  );
  const sourceLink = $derived(safeSourceLink(effectiveSource));

  // App-Info beim ersten Öffnen einmal laden — die Werte sind Compile-Time-
  // konstant, ein Abruf pro App-Lauf reicht. Seit Plan W5 braucht `app_info`
  // eine Sitzung; der Dialog ist erst nach der Anmeldung erreichbar, ein
  // Laden beim Mount (vor der Anmeldung) endete in 401.
  let infoRequested = false;
  $effect(() => {
    if (!aboutModalStore.visible || infoRequested) return;
    infoRequested = true;
    appInfo()
      .then((i) => {
        info = i;
      })
      .catch((e) => {
        infoError = e instanceof Error ? e.message : String(e);
      });
    publicInfo()
      .then((p) => {
        source = p;
      })
      .catch(() => {
        sourceError = true;
      });
  });

  // Modal-Stack (R5-014: App-Shell `inert`). Fokus, Escape und Rücksprung
  // zum Auslöser übernimmt `focusTrap` (U1); Startfokus „Dialog schließen“.
  $effect(() => {
    if (aboutModalStore.visible) {
      pushModal();
      return () => popModal();
    }
  });

  // Drittanbieter-Hauptliste fest verdrahtet (Spec: sichtbar im Dialog selbst,
  // kein Verstecken hinter einem zusätzlichen Klick). Reihenfolge entspricht
  // den Gruppen in PRD §G2-DOC.3.5; Stand Server-Image (Plan W12). Die
  // vollständige Liste mit Lizenztexten steht unter `/legal/`.
  type ThirdParty = { name: string; license: string; role: string };
  type ThirdPartyGroup = { label: string; items: ThirdParty[] };
  const thirdPartyGroups: ThirdPartyGroup[] = [
    {
      label: "Framework / Sprache",
      items: [
        { name: "axum / tower-http / hyper", license: "MIT", role: "Web-Server" },
        { name: "tokio", license: "MIT", role: "Asynchrone Laufzeit" },
        { name: "Rust-Toolchain", license: "MIT / Apache-2.0", role: "Backend" },
        { name: "Svelte 5 / SvelteKit", license: "MIT", role: "Frontend" },
      ],
    },
    {
      label: "Persistenz + Krypto",
      items: [
        { name: "SQLite", license: "Public Domain", role: "Datenbank-Engine" },
        { name: "SQLCipher (Zetetic)", license: "BSD-3-Clause", role: "Verschlüsselung at Rest" },
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
        { name: "Saxon-HE", license: "MPL-2.0", role: "XSLT/Schematron in KoSIT und Mustang" },
        { name: "Apache PDFBox / Apache FOP", license: "Apache-2.0", role: "PDF/A-3 in Mustang" },
        { name: "Typst", license: "Apache-2.0", role: "PDF-Erzeugung" },
        {
          name: "Eclipse Temurin (OpenJDK)",
          license: "GPL-2.0 mit Classpath-Exception",
          role: "Java-Laufzeit (jlink) für KoSIT und Mustang",
        },
      ],
    },
    {
      label: "Versand + Netz",
      items: [
        { name: "lettre", license: "MIT / Apache-2.0", role: "SMTP" },
        { name: "russh + russh-sftp", license: "Apache-2.0", role: "SFTP-Backup-Ziel" },
        {
          name: "reqwest + rustls",
          license: "MIT / Apache-2.0",
          role: "HTTPS-Abfrage der opt-in Versionsprüfung",
        },
      ],
    },
    {
      label: "Laufzeit-Image",
      items: [
        {
          name: "Debian GNU/Linux (trixie-slim)",
          license: "je Paket, siehe /usr/share/doc/*/copyright",
          role: "Basis des Docker-Images (u. a. tzdata, fontconfig, FreeType, DejaVu-Schriften)",
        },
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

{#if aboutModalStore.visible}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="kb-backdrop"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) closeAboutDialog();
    }}
  >
    <div
      class="kb-dialog about"
      role="dialog"
      aria-modal="true"
      aria-labelledby="about-title"
      use:focusTrap={{ onEscape: closeAboutDialog, initialFocus: ".kb-dialog-close" }}
    >
      <header class="kb-dialog-head">
        <h2 id="about-title">Über Klein.Buch</h2>
        <button
          type="button"
          class="kb-dialog-close"
          aria-label="Dialog schließen"
          onclick={closeAboutDialog}><span aria-hidden="true">×</span></button
        >
      </header>

      <div class="kb-dialog-body body">
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
            <a
              class="linklike"
              href="https://wildbach-computerhilfe.de"
              target="_blank"
              rel="noopener noreferrer">wildbach-computerhilfe.de</a
            >
            <span class="sep">·</span>
            <a class="linklike" href="mailto:schmidm@wildbach-computerhilfe.de"
              >schmidm@wildbach-computerhilfe.de</a
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
              <dd>
                {#if info.buildCommit}
                  <code title={info.buildCommit}>{shortCommit(info.buildCommit)}</code>
                {:else}
                  {DEV_SOURCE_TEXT}
                {/if}
              </dd>
              {#if info.buildTag}
                <dt>Release</dt>
                <dd><code>{info.buildTag}</code></dd>
              {/if}
              <dt>Identifier</dt>
              <dd><code>{info.identifier}</code></dd>
            </dl>
          {:else}
            <p class="muted">Lade …</p>
          {/if}
        </section>

        <!-- Lizenz-Block (AGPL §13 — Link auf den exakten Quellstand, Plan W12) -->
        <section class="block">
          <h3>Lizenz</h3>
          <p>
            Klein.Buch ist freie Software unter <strong>AGPL-3.0</strong>. Du
            darfst die Software nutzen, weitergeben und verändern. Wenn du
            Klein.Buch veränderst und anderen über ein Netzwerk zur Nutzung
            bereitstellst (z. B. als Webdienst, auch intern), musst du ihnen den
            Quelltext deiner Fassung anbieten — die Adresse trägst du in
            <code>KLEIN_BUCH_SOURCE_URL</code> ein.
          </p>
          <p class="links">
            <a
              class="linklike"
              href="/legal/LICENSE.txt"
              target="_blank"
              rel="noopener noreferrer"
              data-sveltekit-reload>Lizenztext (AGPL-3.0)</a
            >
            <span class="sep">·</span>
            {#if sourceLink}
              <a class="linklike" href={sourceLink} target="_blank" rel="noopener noreferrer"
                >Quellcode dieser Version ({sourceLabel(effectiveSource)})</a
              >
            {:else if effectiveSource}
              <span>Quellcode: {DEV_SOURCE_TEXT}</span>
            {:else if sourceError && infoError}
              <span class="muted">Quellangaben nicht verfügbar</span>
            {:else}
              <span class="muted">Quellangaben werden geladen …</span>
            {/if}
          </p>
          {#if effectiveSource}
            <p class="muted source-url">
              Quell-Repository: <code>{effectiveSource.sourceUrl.replace(/^https?:\/\//, "")}</code>
            </p>
          {/if}
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
            Lizenztexte und Hinweise (NOTICE) aller Bestandteile — Rust-Crates,
            npm-Pakete, Java-Bibliotheken des Sidecars und Debian-Pakete des
            Images — stehen unter <code>/legal/</code> (auch ohne Anmeldung) und
            im Image unter <code>/usr/share/doc/klein-buch</code>. Die
            Java-Laufzeit (Eclipse Temurin, OpenJDK) steht unter GPL-2.0 mit
            Classpath-Exception; ihre Hinweise liegen im Image unter
            <code>/opt/klein-buch/sidecar/jre/legal</code>. Für die unveränderten
            Teile unter MPL-2.0 (Saxon-HE, Rhino, veraPDF), EPL-1.0
            (logback-core), EUPL-1.2 (CEN-Schematron-Regeln zu EN 16931) und
            GPL-3.0 mit Font-Ausnahme (Schrift GNU FreeSans in Mustang) sowie für
            Temurin und die Debian-Pakete liegt jedem Release ein Quellarchiv bei
            (<code>klein-buch-&lt;Version&gt;-quellen-dritter.tar.zst</code>).
          </p>

          <!-- `/legal/` liefert der Server ohne Sitzung aus (Plan W12). -->
          <p class="legal-links">
            <a
              class="btn-secondary"
              href="/legal/third-party-licenses.html"
              target="_blank"
              rel="noopener noreferrer"
              data-sveltekit-reload
            >
              Vollständige Liste der Open-Source-Bibliotheken öffnen
            </a>
            <a
              class="btn-secondary"
              href="/legal/quellen.html"
              target="_blank"
              rel="noopener noreferrer"
              data-sveltekit-reload
            >
              Quellen Dritter
            </a>
          </p>
        </section>

        <!-- Footer-Disclaimer (Spec: identischer Wortlaut zu G3) -->
        <p class="footer-disclaimer">
          Klein.Buch ist ein Werkzeug, kein Steuerberater.
        </p>
      </div>

      <footer class="kb-dialog-foot">
        <button type="button" class="btn-primary" onclick={closeAboutDialog}>
          Schließen
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .about { --dialog-w: 640px; }
  .body {
    color: var(--text);
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
  /* Trefferfläche ≥ --target, ohne die Zeile optisch zu sprengen. */
  .linklike {
    display: inline-flex;
    align-items: center;
    min-height: var(--target);
    background: transparent;
    border: 0;
    padding: 0;
    color: var(--c-primary-700);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
  @media (hover: hover) {
    .linklike:hover {
      color: var(--c-primary-800);
    }
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
  .legal-links {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin: 0.75rem 0 0;
  }
  .source-url code {
    word-break: break-all;
  }
  .muted {
    color: var(--c-text-muted);
    margin: 0.25rem 0;
  }
  .error {
    color: var(--status-danger-fg);
    margin: 0.25rem 0;
  }
  .footer-disclaimer {
    margin: 1rem 0 0.25rem;
    text-align: center;
    color: var(--c-text-subtle);
    font-size: var(--fs-xs);
    font-style: italic;
  }
</style>
