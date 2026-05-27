// PV1-A5: Globaler Roh-XML-Viewer (Svelte 5 Runes).
//
// Wird genau einmal über `<XmlViewerDialog />` im Root-Layout gemountet — wie
// AboutDialog. Muss dort liegen, weil `<main>` per `inert={modalStack.count>0}`
// gesperrt wird, sobald ein Modal offen ist: ein Dialog innerhalb von `<main>`
// würde sich selbst lähmen.
//
// Aufrufer (Eingangsbeleg-Detail-Seite) ruft `openXmlViewer(payload)` mit dem
// bereits geladenen Payload. Laden/Fehler-Handling bleibt beim Aufrufer
// (Toast bei Tamper / kein E-Rechnungs-Original).

import type { XmlViewerPayload } from "$lib/types";

export const xmlViewerStore = $state<{
  visible: boolean;
  payload: XmlViewerPayload | null;
}>({ visible: false, payload: null });

export function openXmlViewer(payload: XmlViewerPayload): void {
  xmlViewerStore.payload = payload;
  xmlViewerStore.visible = true;
}

export function closeXmlViewer(): void {
  xmlViewerStore.visible = false;
  xmlViewerStore.payload = null;
}
