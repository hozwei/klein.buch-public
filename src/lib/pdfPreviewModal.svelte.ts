// Plan W7: PDF-Vorschau im Browser. Vorschauen (Vorlage, Paket, Broschüre,
// Angebots-Bundle) und eigene Archiv-PDFs (Rechnung, Storno, Angebot) liefert
// der Server inline aus (`/files/download/{id}`, `/files/archive/{id}`); der
// Dialog zeigt sie in einem iframe. Genau einmal im Root-Layout gemountet
// (`<PdfPreviewDialog />`, AboutDialog-Pattern).

export const pdfPreviewStore = $state<{
  visible: boolean;
  url: string;
  title: string;
  fileName: string;
}>({ visible: false, url: "", title: "", fileName: "" });

export function openPdfPreview(url: string, title: string, fileName: string): void {
  pdfPreviewStore.url = url;
  pdfPreviewStore.title = title;
  pdfPreviewStore.fileName = fileName;
  pdfPreviewStore.visible = true;
}

export function closePdfPreview(): void {
  pdfPreviewStore.visible = false;
  pdfPreviewStore.url = "";
}
