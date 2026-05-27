// Globaler „Über"-Dialog (Svelte 5 Runes), Block G2-DOC.3.5.
//
// Genau einmal über <AboutDialog/> im Root-Layout gemountet; der Nav-Button
// (Hauptnav „Über") öffnet den Dialog per `openAboutDialog()`. Es gibt keine
// Promise-Rückgabe wie beim `confirmDialog()` — der Über-Dialog ist rein
// informativ; das Schließen bedeutet nichts, was der Caller wissen müsste.

export const aboutModalStore = $state<{ visible: boolean }>({ visible: false });

export function openAboutDialog(): void {
  aboutModalStore.visible = true;
}

export function closeAboutDialog(): void {
  aboutModalStore.visible = false;
}
