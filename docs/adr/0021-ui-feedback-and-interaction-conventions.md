# ADR 0021 — UI-Feedback & Interaktions-Konventionen (Block DS, Feedback-Teil)

**Status:** Akzeptiert · 2026-05-20 · Block DS (Feedback-Teil).

## Kontext

Das Feedback war je Feature uneinheitlich gewachsen: mal Toast oben rechts, mal
Inline-`<p class="error">` mitten im Seitenfluss, mal Browser-`confirm()` und
native `required`-Sprechblasen im OS-Stil. Detail-Aktionsleisten waren zudem
farblich überladen (mehrere konkurrierende blaue Buttons + knallrote Flächen).
Manuel forderte app-weite Konsistenz. Der volle Design-System-Teil von Block DS
(Tokens + wiederverwendbare Komponenten) bleibt davon getrennt und offen; hier
geht es nur um Feedback- und Interaktions-Verhalten.

## Entscheidung

- **Toast = Standard-Feedback für fast alles** (Erfolg UND Fehler einer Aktion).
  Globaler Store `$lib/toast.svelte.ts` (`toastStore` + `flash(msg, kind, ttl?)`
  + `dismissToast`), gerendert genau einmal über den Host `$lib/ToastHost.svelte`
  in `routes/+layout.svelte`. Auto-Dismiss (Fehler länger als Erfolg), gestapelt.
  Der Host heißt bewusst **ToastHost** (nicht `Toast`) — `Toast.svelte` würde auf
  case-insensitivem Windows-FS mit dem Modul `toast.svelte.ts` kollidieren.
- **`confirmDialog()` statt Browser-`confirm()`** — `$lib/confirm.svelte.ts`
  (Promise<boolean>), gerendert über `$lib/ConfirmDialog.svelte` im Layout;
  `danger`-Variante für destruktive Aktionen (Esc/Backdrop = Abbrechen).
  **Ausnahme:** der §19→Regelbesteuerung-Warn-Dialog (5-Jahres-Bindung) bleibt
  ein eigener, bewusst gestalteter Modal in `settings/seller`.
- **`$lib/Banner.svelte` (error/warning/info) für legitim PERSISTENTE Zustände**,
  die kein flüchtiger Toast sein dürfen: „… nicht gefunden", „Firmendaten fehlen",
  „kein Postfach", „AGB fehlen", Status-Sperren (Storno/Zahlung). Bleibt im
  Seitenfluss stehen, bis der Zustand behoben ist.
- **Pflichtfeld-Validierung = ebenfalls Toast (kein nativer Browser-Bubble).**
  Jedes `<form>` trägt `novalidate`; der Submit-Handler prüft Pflichtfelder selbst
  und gibt `flash(…, "error")` + `return`. `required` bleibt im Markup nur für
  Screenreader-Semantik. Geschäftsregeln (Betrag > 0, ≤ offener Rest) sind ohnehin
  Toast. Ausnahme: button-getriebene Formulare ohne `<form>` flashen direkt;
  `settings/payment-accounts` lässt den Anlegen-Button deaktiviert bis Pflichtfeld.
- **Button-Hierarchie in Detail-Aktionsleisten:** genau EIN `btn-primary` (blau)
  je Zustand = der nächste Status-Schritt (Angebot Fertigstellen→Annehmen→In
  Rechnung umwandeln; Rechnung Ausstellen→`+ Zahlung`). Versenden/PDF-Aktionen
  sind `btn-secondary`. Destruktiver Trigger (Stornieren) = `btn-ghost-danger`
  (transparent, rote Schrift), einmal am Ende. Die volle `btn-danger`-Fläche bleibt
  nur dem echten Bestätigen-Button im Storno-Panel/-Dialog vorbehalten.

## Konsequenzen

- App-weit einheitliches Feedback; Seiten mounten **kein** eigenes `<Toast>` und
  definieren **kein** lokales `flash`/Inline-Error mehr.
- **Neue Features müssen diese Helfer nutzen** (`flash`, `confirmDialog`,
  `<Banner>`, `novalidate`+Toast-Validierung, 1-Primär/Ghost-Storno). Verbindliche
  Kurzfassung im Cowork-Memory `feedback/ui-feedback-pattern`.
- Reiner **Frontend-Layer — kein Schema-/Backend-Impact** (Schema bleibt v9, keine
  Migration).
- **Offen (restlicher Block DS):** Design-Tokens (Farben/Spacing/Typo/Radien) und
  wiederverwendbare Komponenten (Button/Card/Table/Badge/FormField) inkl. Ablösung
  der per-Seite-`<style>`. Muss vor dem v0.1.0-Release (Block 17) landen.

## Alternativen

| Option | Contra |
|---|---|
| Inline-Errors + `confirm()` + native `required`-Bubbles behalten | uneinheitlich, OS-Look statt App-Stil, zusammengewürfelt |
| Pflichtfeld-Validierung als native `required` (statt Toast) | Browser-Bubble passt nicht zum App-Look; Manuel wählte Toast für app-weite Einheitlichkeit |
| Überfüllte Aktionsleiste in ein „Mehr ⋯"-Overflow-Menü | mehr UI-Maschinerie (Dropdown-Komponente); verworfen zugunsten Einfärben + Gruppieren |

## Referenzen

`lib/{toast.svelte.ts, confirm.svelte.ts, ToastHost.svelte, ConfirmDialog.svelte, Banner.svelte}`,
`routes/+layout.svelte`; betroffene Routen unter `routes/{contacts,invoices,quotes,expenses,private-movements,settings}`;
Cowork-Memory `feedback/ui-feedback-pattern`.
