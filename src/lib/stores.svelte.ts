// Globale Reactive-States (Svelte 5 Runes). Aktuell leer — wächst mit den Modulen.

export const appState = $state({
  ready: false,
  sellerLoaded: false,
});

// U7 (R3-06): Zähler der offenen Hinweise in Seitenleiste und Menüknopf. Das Layout
// lädt ihn bei jedem Seitenwechsel und, wenn sich `version` ändert, neu; Seiten, die
// Hinweise abhaken oder neu prüfen, rufen danach `notificationsChanged()` auf.
export const notificationsState = $state({ version: 0 });

export function notificationsChanged(): void {
  notificationsState.version++;
}
