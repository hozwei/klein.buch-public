// Minimaler, sicherer Markdown-Subset → HTML für die Live-Vorschau im
// Paket-Editor (Block P2b). Bewusst grob: die EXAKTE Darstellung liefert die
// serverseitige „PDF-Vorschau" (domain::package::to_typst). Hier geht es nur um
// sofortiges Feedback beim Tippen.
//
// Sicherheit: Es wird ZUERST jeglicher HTML-Input escaped, danach werden nur
// eigene, kontrollierte Tags ergänzt. Damit ist {@html mdPreview(x)} XSS-sicher,
// auch wenn der Nutzer rohes HTML eintippt.

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function inline(t: string): string {
  return t
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    .replace(/`(.+?)`/g, "<code>$1</code>");
}

export function mdPreview(src: string): string {
  const lines = escapeHtml(src ?? "").split(/\r?\n/);
  let html = "";
  let list: "ul" | "ol" | null = null;
  const closeList = () => {
    if (list) {
      html += `</${list}>`;
      list = null;
    }
  };
  for (const raw of lines) {
    const line = raw.trimEnd();
    let m: RegExpMatchArray | null;
    if ((m = line.match(/^(#{1,3})\s+(.*)$/))) {
      closeList();
      const lvl = m[1].length;
      html += `<h${lvl}>${inline(m[2])}</h${lvl}>`;
    } else if ((m = line.match(/^[-*]\s+(.*)$/))) {
      if (list !== "ul") {
        closeList();
        html += "<ul>";
        list = "ul";
      }
      html += `<li>${inline(m[1])}</li>`;
    } else if ((m = line.match(/^\d+\.\s+(.*)$/))) {
      if (list !== "ol") {
        closeList();
        html += "<ol>";
        list = "ol";
      }
      html += `<li>${inline(m[1])}</li>`;
    } else if (line.trim() === "") {
      closeList();
    } else {
      closeList();
      html += `<p>${inline(line)}</p>`;
    }
  }
  closeList();
  return html;
}
