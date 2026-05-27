<script lang="ts">
  import "$lib/styles/tokens.css";
  import { goto, onNavigate } from "$app/navigation";
  import { page } from "$app/stores";
  import BackupGate from "$lib/BackupGate.svelte";
  import ToastHost from "$lib/ToastHost.svelte";
  import ConfirmDialog from "$lib/ConfirmDialog.svelte";
  import AboutDialog from "$lib/AboutDialog.svelte";
  import XmlViewerDialog from "$lib/XmlViewerDialog.svelte";
  import { appState } from "$lib/stores.svelte";
  import { modalStack } from "$lib/modalStack.svelte";
  import { notificationsUnreadCount, appInfo } from "$lib/api";
  import { openAboutDialog } from "$lib/aboutModal.svelte";
  import { formatVersionDisplay } from "$lib/version";
  let { children } = $props();

  // Badge mit der Anzahl offener Hinweise. Wird beim Navigieren aktualisiert.
  let unread = $state(0);
  async function refreshUnread() {
    try {
      unread = await notificationsUnreadCount();
    } catch {
      // vor dem Entsperren / Bootstrap noch nicht verfügbar — ignorieren.
    }
  }
  $effect(() => {
    $page.url.pathname; // bei Navigation neu laden
    // Erst nach dem Entsperren (G1-ENC Schritt 2) liegt der DB-Pool im State —
    // vorher keine Command-Aufrufe absetzen.
    if (appState.ready) refreshUnread();
  });

  // G4.1-Fix (Manuel 2026-05-26): App-Version im Sidebar-Footer und als
  // Suffix am „Über"-Eintrag. Format kommt aus `formatVersionDisplay`
  // (CalVer YYYY.M.PATCH → „V2026.5" bzw. „V2026.5.<patch>" bei Bugfix).
  // Footer ist NICHT mehr klickbar — „Über" bleibt der einzige Einstieg
  // in den AboutDialog.
  let displayVersion = $state<string>("…");
  $effect(() => {
    if (!appState.ready) return;
    appInfo()
      .then((info) => {
        displayVersion = formatVersionDisplay(info.appVersion);
      })
      .catch(() => {
        // Fallback: leer statt Fehlertext im Footer.
        displayVersion = "";
      });
  });

  // G2-UX.3.1b — Sanfter Seitenwechsel via View-Transitions-API.
  // Greift nur, wenn die Engine die API hat (WebView2 / Chromium 111+).
  // Ohne API navigiert SvelteKit weiterhin direkt — kein Fallback nötig,
  // kein Schaden, nur kein Effekt. Animations-Definitionen liegen in
  // tokens.css unter "Page-Transitions (G2-UX.3.1b)".
  onNavigate((navigation) => {
    if (!("startViewTransition" in document)) return;
    return new Promise((resolve) => {
      // TS-Cast: startViewTransition ist in lib.dom (.experimental) deklariert,
      // aber nicht in jedem TS-Target. Feature-Check oben schützt zur Laufzeit.
      (document as Document & {
        startViewTransition: (cb: () => Promise<void>) => unknown;
      }).startViewTransition(async () => {
        resolve();
        await navigation.complete;
      });
    });
  });

  // Sidebar-Icons (Manuel 2026-05-26): Inline-SVG-Paths im Lucide-Stil,
  // 24x24-viewBox, currentColor-stroke (greift das Sidebar-Weiß).
  // Hardcoded Konstanten, daher ist `{@html}` weiter unten unkritisch
  // (kein User-Input). Falls die Sammlung wächst, in $lib/NavIcon.svelte
  // ausziehen — für 14 Einträge ist Inline ok.
  const ICONS: Record<string, string> = {
    home: '<path d="M3 12L12 4l9 8"/><path d="M5 10v10h14V10"/><path d="M10 20v-6h4v6"/>',
    user: '<circle cx="12" cy="8" r="3.5"/><path d="M5 20c0-3.5 3-6 7-6s7 2.5 7 6"/>',
    fileText:
      '<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="M8 13h8"/><path d="M8 17h5"/>',
    filePlus:
      '<path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="M12 12v6"/><path d="M9 15h6"/>',
    package:
      '<path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0L4 6.27A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><path d="M3.3 7L12 12l8.7-5"/><path d="M12 22V12"/>',
    receipt:
      '<path d="M4 4v18l3-2 3 2 3-2 3 2 4-3V4z"/><path d="M8 9h8"/><path d="M8 13h8"/><path d="M8 17h5"/>',
    briefcase:
      '<rect x="3" y="7" width="18" height="13" rx="2"/><path d="M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M3 13h18"/>',
    wallet:
      '<rect x="3" y="6" width="18" height="14" rx="2"/><path d="M3 10h18"/><circle cx="17" cy="15" r="1.3" fill="currentColor" stroke="none"/>',
    calculator:
      '<rect x="5" y="3" width="14" height="18" rx="2"/><rect x="8" y="6" width="8" height="3"/><path d="M9 13h.01"/><path d="M12 13h.01"/><path d="M15 13h.01"/><path d="M9 17h.01"/><path d="M12 17h.01"/><path d="M15 17h.01"/>',
    calendar:
      '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18"/><path d="M8 3v4"/><path d="M16 3v4"/>',
    bell: '<path d="M18 16v-5a6 6 0 1 0 -12 0v5l-2 2h16z"/><path d="M10 21a2 2 0 0 0 4 0"/>',
    settings:
      '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2 12h3M19 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>',
    help: '<circle cx="12" cy="12" r="10"/><path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3"/><path d="M12 17h.01"/>',
    info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
  };

  const nav: Array<{ href: string; label: string; icon: string }> = [
    { href: "/", label: "Start", icon: "home" },
    { href: "/contacts", label: "Kontakte", icon: "user" },
    { href: "/invoices", label: "Rechnungen", icon: "fileText" },
    { href: "/quotes", label: "Angebote", icon: "filePlus" },
    { href: "/packages", label: "Pakete", icon: "package" },
    { href: "/expenses", label: "Kosten", icon: "receipt" },
    { href: "/assets", label: "Anschaffungen", icon: "briefcase" },
    { href: "/private-movements", label: "Privat-Geld", icon: "wallet" },
    { href: "/euer", label: "Steuer (EÜR)", icon: "calculator" },
    { href: "/fiscal-year", label: "Geschäftsjahr", icon: "calendar" },
    { href: "/notifications", label: "Hinweise", icon: "bell" },
    { href: "/settings", label: "Einstellungen", icon: "settings" },
    // G2-DOC.3.4: finaler Handbuch-Eintrag — ersetzt den TEMP-Marker
    // „Hilfe (3.1)" aus G2-DOC.3.1. F1 öffnet zusätzlich `/help`
    // (Listener weiter unten, außer in Eingabefeldern).
    { href: "/help", label: "Hilfe", icon: "help" },
  ];

  function isActive(pathname: string, href: string): boolean {
    if (href === "/") return pathname === "/";
    return pathname === href || pathname.startsWith(href + "/");
  }

  // --- F1-Keybinding (G2-DOC.3.4) ------------------------------------------
  //
  // F1 öffnet `/help`. In Eingabefeldern (input/textarea/contenteditable
  // oder eine Combobox-Rolle) wird F1 NICHT abgefangen, damit der Nutzer
  // ein etwaiges OS-/Browser-Default-Verhalten dort behält und das
  // Tippen nicht durcheinandergeht.
  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    const tag = target.tagName;
    if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
    if (target.isContentEditable) return true;
    const role = target.getAttribute("role");
    if (role === "textbox" || role === "combobox" || role === "searchbox") {
      return true;
    }
    return false;
  }
  function onWindowKeydown(ev: KeyboardEvent): void {
    if (ev.key !== "F1") return;
    if (isEditableTarget(ev.target)) return;
    // Modifikatoren ignorieren — Ctrl+F1 / Shift+F1 sind nicht „unser" F1.
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.shiftKey) return;
    ev.preventDefault();
    if (!$page.url.pathname.startsWith("/help")) {
      void goto("/help");
    }
  }
  $effect(() => {
    if (!appState.ready) return;
    window.addEventListener("keydown", onWindowKeydown);
    return () => window.removeEventListener("keydown", onWindowKeydown);
  });
</script>

<!-- App-Shell (Nav + Seiteninhalt) erst nach dem Entsperren rendern. Vorher
     existiert kein DB-Pool, und die Seiten würden ins Leere laufen. Das
     BackupGate-Overlay liegt darüber und holt die Passphrase. -->
{#if appState.ready}
  <!--
    Kein Sidebar-Branding-Header mehr (Manuel 2026-05-26): die Marke
    sitzt in der Window-Titelleiste (Icon + „Klein.Buch") und in den
    Bundle-Icons. Die Sidebar ist reine Navigations-Schiene — Discord/
    Slack/VS Code-Muster. `padding-top` ist etwas großzügiger, damit
    der erste Nav-Eintrag nicht am Fensterrand klebt.
  -->
  <!-- R5-014: `inert` solange ein Modal offen ist — Tab kann nicht aus dem
       Modal in die Sidebar springen. `|| undefined` weil Svelte `inert=false`
       sonst als String-Attribut rendert, was die a11y-Semantik invertiert. -->
  <nav class="sidebar" inert={modalStack.count > 0 || undefined}>
    <ul>
      {#each nav as item}
        <li class:active={isActive($page.url.pathname, item.href)}>
          <a href={item.href}>
            <svg
              class="nav-icon"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true">{@html ICONS[item.icon]}</svg>
            <span class="nav-label">{item.label}</span>
            {#if item.href === "/notifications" && unread > 0}
              <span class="nav-badge">{unread}</span>
            {/if}
          </a>
        </li>
      {/each}
      <!--
        G2-DOC.3.5 — „Über"-Eintrag neben „Hilfe".
        Bewusst Button statt Link, weil der Über-Dialog ein Modal ist
        (keine eigene Route, kein URL-State zu reflektieren).
      -->
      <li>
        <button type="button" class="nav-button" onclick={openAboutDialog}>
          <svg
            class="nav-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true">{@html ICONS.info}</svg>
          <span class="nav-label">Über</span>
        </button>
      </li>
    </ul>
    <!--
      Sidebar-Footer (G4.1-Fix): reine Versions-Anzeige, unten gepinnt
      (Sidebar als flex-column, footer mit margin-top: auto).
      Nicht klickbar — der „Über"-Eintrag im Nav ist der einzige
      Einstieg in den AboutDialog.
    -->
    <footer>
      <span class="version-label" aria-label="App-Version">
        {displayVersion}
      </span>
    </footer>
  </nav>

  <main inert={modalStack.count > 0 || undefined}>
    {@render children?.()}
  </main>
{/if}

<BackupGate />

<!-- Global, einmalig: app-weites Feedback (Toast) + Bestätigungs-Dialog +
     statischer „Über"-Dialog (G2-DOC.3.5). -->
<ToastHost />
<ConfirmDialog />
<AboutDialog />
<XmlViewerDialog />

<style>
  :global(body) {
    display: flex;
  }
  .sidebar {
    width: 208px;
    background: var(--c-primary-800);
    color: #cfe0e6;
    /* Padding-top etwas großzügiger, weil der frühere h1-Marken-Header
       weggefallen ist und der erste Nav-Eintrag sonst direkt am
       Fensterrand kleben würde. */
    padding: 1.5rem 0.75rem 1rem;
    box-sizing: border-box;
    height: 100vh;
    overflow-y: auto;
    /* G4.1-Fix: flex-column, damit der Versions-Footer per
       `margin-top: auto` an den unteren Sidebar-Rand gepinnt wird. */
    display: flex;
    flex-direction: column;
  }
  .sidebar ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .sidebar li {
    margin: 0.15rem 0;
  }
  .sidebar a,
  .sidebar .nav-button {
    color: #bcd4dc;
    text-decoration: none;
    display: flex;
    align-items: center;
    gap: 0.7rem;
    padding: 0.5rem 0.6rem;
    border-radius: var(--r-md);
    font-size: 0.9rem;
    font-weight: 500;
  }
  .sidebar a:hover,
  .sidebar .nav-button:hover {
    background: rgba(255, 255, 255, 0.07);
    color: #fff;
    text-decoration: none;
  }
  .sidebar li.active a {
    background: rgba(255, 255, 255, 0.13);
    color: #fff;
  }
  /* Pro-Eintrag-Icon (Manuel 2026-05-26). Etwas gedimmt im Ruhezustand,
     volle Deckkraft bei Hover/Active — gibt der Sidebar visuelle Tiefe
     ohne von den Labels abzulenken. */
  .sidebar .nav-icon {
    width: 18px;
    height: 18px;
    flex: none;
    opacity: 0.78;
    transition: opacity var(--t-fast) var(--ease-apple);
  }
  .sidebar a:hover .nav-icon,
  .sidebar .nav-button:hover .nav-icon,
  .sidebar li.active .nav-icon {
    opacity: 1;
  }
  .sidebar .nav-label {
    flex: 1 1 auto;
    min-width: 0;
  }
  /* Sidebar-„Über"-Eintrag (G2-DOC.3.5): visuell wie ein Nav-Link, technisch
     Button — Modal hat keine eigene Route, also keinen `href`. */
  .sidebar .nav-button {
    width: 100%;
    background: transparent;
    border: 0;
    cursor: pointer;
    text-align: left;
    font-family: inherit;
  }
  .nav-badge {
    background: var(--c-danger-500);
    color: #fff;
    font-size: 0.7rem;
    font-weight: 700;
    line-height: 1;
    padding: 0.15rem 0.4rem;
    border-radius: var(--r-pill);
    min-width: 1rem;
    text-align: center;
  }
  .sidebar footer {
    color: #7fa3ae;
    font-size: 0.75rem;
    /* G4.1-Fix: an den unteren Rand pinnen (sidebar ist flex-column). */
    margin-top: auto;
    padding: 0.75rem 0.5rem 0.25rem;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }
  /* Reine Versions-Anzeige, nicht klickbar (G4.1-Fix). */
  .sidebar .version-label {
    display: block;
    padding: 0.15rem 0.3rem;
    color: inherit;
    font: inherit;
    user-select: text;
  }
  main {
    flex: 1;
    /* G2-UX.3.1b: Ziel der View-Transition. Sidebar (<nav> nebenan) hat
       absichtlich KEINEN Namen — sie bleibt beim Seitenwechsel ruhig stehen.
       Keyframes liegen in tokens.css unter „Page-Transitions". */
    view-transition-name: page-content;
    /* Kein Padding oben: damit eine sticky PageBar bündig am oberen Rand klebt
       (kein Streifen, in dem Inhalt darüber durchscrollt). Seiten ohne PageBar
       bekommen den oberen Abstand über die :first-child-Regel unten zurück. */
    padding: 0 var(--main-pad) var(--main-pad);
    overflow-y: auto;
    height: 100vh;
    box-sizing: border-box;
    background: var(--c-bg);
  }
  /* Erster Inhalt einer Seite bekommt oben Luft … */
  main > :global(:first-child) {
    margin-top: var(--main-pad);
  }
  /* … außer die sticky PageBar, die soll bündig oben sitzen. */
  main > :global(.pagebar) {
    margin-top: 0;
  }

  /* --- Druck (G2-DOC.3.4) -----------------------------------------------
     Beim Drucken aus einem Handbuch-Kapitel (oder einer beliebigen Seite)
     fallen die App-Nav und das overflow-Scroll-Verhalten weg. Sonst würde
     der Browser entweder die Sidebar mitdrucken oder nur den sichtbaren
     Viewport-Ausschnitt aufs Papier bringen. */
  @media print {
    :global(body) {
      display: block;
    }
    .sidebar {
      display: none;
    }
    main {
      padding: 0;
      overflow: visible;
      height: auto;
    }
    main > :global(:first-child) {
      margin-top: 0;
    }
  }
</style>
