# ADR 0029 — Design-System: zentrale Tokens, wiederverwendbare Komponenten, Toast-Default

**Status:** Akzeptiert · 2026-05-22 · Block DS. Keine Migration.

## Kontext

Die UI war über die Blöcke organisch gewachsen: pro Seite eigenes inline
`<style>`, uneinheitliche Buttons/Tabellen/Status, gemischte Feedback-Wege
(inline `<p class="error">`, Browser-`confirm()`). Manuels Urteil: „zu
unterschiedlich und teilweise hässlich". Gewünscht ist ein moderner SaaS-Look
mit etwas Farbe, aber seriös — eine Mischung aus Finanzguru (SaaS) und der
Wildbach-Marke. Constraint: **local-first** → kein Web-Font-Download.

## Entscheidung

1. **Zentrale Design-Tokens** (`src/lib/styles/tokens.css`, CSS Custom
   Properties): Petrol **`#176b87`** (Wildbach-Marke) als Primärfarbe + Akzent,
   Finanzguru-SaaS-Anmutung; **System-Font-Stack (Segoe UI …)** statt Web-Font
   (local-first); Spacing/Radien/Schatten/Typo-Skala.
2. **Wiederverwendbare Komponenten** (`src/lib/`): `Button`, `Card`, `Badge`,
   `FormField`, `Table`, dazu `PageBar` (sticky Aktionsleiste), `ToastHost`,
   `ConfirmDialog`, `Banner`.
3. **Feedback-Konvention** (siehe ADR 0021): **Toast ist der Standard** für fast
   alles — Erfolg UND Fehler. `confirmDialog()` statt Browser-`confirm()`;
   **Banner** für legitim persistente Zustände („Firmendaten fehlen"); schwer-
   gewichtige Aktionen behalten einen bewussten **Modal** (§19-Verzicht mit
   5-Jahres-Bindung).
4. **Sticky `PageBar`**: oben links Zurück + Titel, oben rechts die
   Hauptaktionen — auf allen Seiten, scrollt mit.
5. **Pragmatische Migration:** globale `!important`-Overrides für Legacy-Klassen
   (`.btn-*`, `table`/`th`/`td`, `.status-*`) ziehen ~30 Seiten **app-weit** auf
   den DS-Look, ohne jede Seite anzufassen. **Strukturelle** Unterschiede
   (eigene `.badge`-Klassen, fehlende „Öffnen"-Spalten, Formular-Save in die
   Bar) werden **pro Seite** umgebaut.

## Konsequenzen

- Einheitliches Look-and-Feel; neue Seiten erben Tokens + Komponenten
  automatisch.
- Die globalen `!important`-Overrides sind ein bewusster Specificity-Trade-off
  (global schlägt Legacy, scoped-lokale Regeln schlagen global wieder) —
  dokumentiert, damit spätere Edits das Verhalten nicht überraschend finden.
- Tote lokale `.btn-*`/`.badge`/`th`-Regeln werden nach der Migration
  aufgeräumt; `pnpm check` (svelte-check) hält ungenutzte Selektoren grün.
- Kein Web-Font → kein Netzzugriff, konsistent mit local-first.

## Alternativen

| Option | Contra |
|---|---|
| CSS-Framework (Tailwind/Bootstrap) | Overkill + Build-/Bundle-Gewicht; eigene Tokens reichen und bleiben schlank |
| Web-Font (z. B. Inter) | Download/Netz nötig — verletzt local-first; System-Font ist nativ + schnell |
| Jede Seite einzeln stylen (Status quo) | genau das Problem (Inkonsistenz, Wartungslast) |

## Referenzen

`src/lib/styles/tokens.css`, `src/lib/{Button,Card,Badge,FormField,Table,
PageBar,ToastHost,ConfirmDialog,Banner}.svelte`, `src/lib/toast.svelte.ts`;
ADR 0021 (UI-Feedback-/Interaktions-Konventionen). Commits `block-ds-1` /
`block-ds-2`.
