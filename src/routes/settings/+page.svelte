<script lang="ts">
  import PageBar from "#lib/PageBar.svelte";

  // Einstellungs-Hub. U1: Querverweise auf Hinweis-Regeln, EÜR-Export und die
  // Protokolle (R5-SUB4-007), Hover nur mit Zeigegerät, Rollen-Tokens.
  const groups: { title: string; items: { href: string; title: string; sub: string }[] }[] = [
    {
      title: "Firma und Belege",
      items: [
        { href: "/settings/seller", title: "Meine Firmendaten", sub: "Name, Adresse, Steuernummer und Kleinunternehmer-Regelung" },
        { href: "/settings/payment-accounts", title: "Konten", sub: "Bankkonto, Bargeld und weitere Zahlungswege" },
        { href: "/settings/travel", title: "Anfahrtspauschale", sub: "Kilometersatz für Anfahrten in Angeboten und Rechnungen" },
        { href: "/settings/pdf-templates", title: "Rechnungs-Layout", sub: "Wie deine PDF-Rechnungen aussehen" },
        { href: "/settings/legal", title: "AGB & Datenschutz", sub: "Eigene Dokumente hinterlegen, die du Angeboten beilegen kannst" },
      ],
    },
    {
      title: "Versand und Hinweise",
      items: [
        { href: "/settings/mail", title: "E-Mail-Versand", sub: "Postfach einrichten und Testmail senden" },
        { href: "/settings/mail-log", title: "Versandprotokoll", sub: "Alle gesendeten E-Mails mit Empfänger und Ergebnis" },
        { href: "/notifications/rules", title: "Hinweis-Regeln", sub: "Welche Hinweise im Posteingang erscheinen" },
      ],
    },
    {
      title: "Sicherheit und Daten",
      items: [
        { href: "/settings/security", title: "Sicherheit & Anmeldung", sub: "Zweiter Faktor, Einmal-Codes, Daten-Passwort, Sitzungen und Versionsprüfung" },
        { href: "/settings/backup", title: "Backups", sub: "Automatische Sicherungen, Ziele und Wiederherstellung" },
        { href: "/settings/backup-log", title: "Backup-Protokoll", sub: "Jede Sicherung mit Ziel, Größe und Ergebnis" },
        { href: "/settings/audit-trail", title: "Protokoll & Datensicherheit", sub: "Lückenlose Aufzeichnung aller Vorgänge und Prüfung des Archivs" },
        { href: "/settings/migration-export", title: "Daten exportieren", sub: "Alle Daten als Paket sichern, z. B. für einen Programmwechsel" },
        { href: "/euer/export", title: "EÜR-Export", sub: "Einnahmen-Überschuss-Rechnung für Finanzamt und Steuerberater exportieren" },
      ],
    },
  ];
</script>

<PageBar title="Einstellungen" />

{#each groups as g, gi (g.title)}
  <section class="group" aria-labelledby="grp-{gi}">
    <h2 id="grp-{gi}">{g.title}</h2>
    <ul class="settings-list">
      {#each g.items as it (it.href)}
        <li>
          <a href={it.href}>
            <span class="s-title">{it.title}</span>
            <span class="s-sub">{it.sub}</span>
          </a>
        </li>
      {/each}
    </ul>
  </section>
{/each}

<style>
  .group { margin-bottom: var(--sp-5); }
  .group h2 {
    font-size: var(--fs-sm); font-weight: 600; text-transform: uppercase; letter-spacing: .05em;
    color: var(--text-muted); margin: 0 0 var(--sp-2);
  }
  .settings-list {
    list-style: none; margin: 0; padding: 0;
    display: grid; grid-template-columns: repeat(auto-fill, minmax(min(18rem, 100%), 1fr)); gap: 0.75rem;
  }
  .settings-list a {
    display: flex; flex-direction: column; gap: 0.25rem; height: 100%;
    min-height: var(--target);
    background: var(--surface-card); border: 1px solid var(--line); border-radius: var(--r-lg);
    padding: 0.9rem 1.1rem; text-decoration: none; box-shadow: var(--sh-sm);
    transition: border-color var(--t-fast), box-shadow var(--t-fast);
  }
  @media (hover: hover) {
    .settings-list a:hover { border-color: var(--c-primary-300); box-shadow: var(--sh-md); text-decoration: none; }
  }
  .s-title { font-weight: 600; color: var(--accent); }
  .s-sub { color: var(--text-muted); font-size: var(--fs-sm); }
</style>
