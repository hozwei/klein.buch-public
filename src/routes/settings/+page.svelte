<script lang="ts">
  import PageBar from "$lib/PageBar.svelte";
</script>

<PageBar title="Einstellungen" />

<div class="settings-list">
  <a href="/settings/seller">
    <span class="s-title">Meine Firmendaten</span>
    <span class="s-sub">Name, Adresse, Steuernummer und Kleinunternehmer-Regelung</span>
  </a>
  <a href="/settings/mail">
    <span class="s-title">E-Mail-Versand</span>
    <span class="s-sub">Postfach einrichten und Versandprotokoll einsehen</span>
  </a>
  <a href="/settings/legal">
    <span class="s-title">AGB &amp; Datenschutz</span>
    <span class="s-sub">Eigene Dokumente hinterlegen, die du Angeboten beilegen kannst</span>
  </a>
  <a href="/settings/payment-accounts">
    <span class="s-title">Konten</span>
    <span class="s-sub">Bankkonto, Bargeld und weitere Zahlungswege</span>
  </a>
  <a href="/settings/travel">
    <span class="s-title">Anfahrtspauschale</span>
    <span class="s-sub">Kilometersatz für Anfahrten in Angeboten und Rechnungen</span>
  </a>
  <a href="/settings/drop-folder">
    <span class="s-title">Rechnungs-Eingang</span>
    <span class="s-sub">Ordner, aus dem eingehende E-Rechnungen automatisch übernommen werden</span>
  </a>
  <a href="/settings/pdf-templates">
    <span class="s-title">Rechnungs-Layout</span>
    <span class="s-sub">Wie deine PDF-Rechnungen aussehen</span>
  </a>
  <a href="/settings/backup">
    <span class="s-title">Backups</span>
    <span class="s-sub">Automatische Sicherungen, Wiederherstellung und Protokoll</span>
  </a>
  <a href="/settings/migration-export">
    <span class="s-title">Daten exportieren</span>
    <span class="s-sub">Alle Daten als Paket sichern – z. B. für den Steuerberater</span>
  </a>
  <a href="/settings/audit-trail">
    <span class="s-title">Protokoll &amp; Datensicherheit</span>
    <span class="s-sub">Lückenlose Aufzeichnung aller Vorgänge + Archiv-Integrität prüfen</span>
  </a>
  <a href="/settings/reset" class="danger">
    <span class="s-title">Zurücksetzen</span>
    <span class="s-sub">Alle Daten löschen und neu beginnen – z. B. zur Geräte-Weitergabe</span>
  </a>
</div>

<style>
  .settings-list { display: grid; grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr)); gap: 0.75rem; }
  .settings-list a {
    display: flex; flex-direction: column; gap: 0.25rem;
    background: var(--c-surface); border: 1px solid var(--c-border); border-radius: var(--r-lg);
    padding: 0.9rem 1.1rem; text-decoration: none; box-shadow: var(--sh-sm);
    transition: border-color 0.12s, box-shadow 0.12s;
  }
  .settings-list a:hover { border-color: var(--c-primary-300); box-shadow: var(--sh-md); }
  .settings-list a.danger { border-color: #fecaca; }
  .settings-list a.danger:hover { border-color: #f87171; }
  .settings-list a.danger .s-title { color: #b91c1c; }
  .s-title { font-weight: 600; color: var(--c-text); }
  .s-sub { color: var(--c-text-muted); font-size: 0.85rem; }
</style>
