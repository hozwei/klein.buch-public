// Minimaler, sicherer Markdown-Subset → HTML für die Live-Vorschau im
// Paket-Editor (Block P2b). Bewusst grob: die EXAKTE Darstellung liefert die
// serverseitige „PDF-Vorschau" (domain::package::to_typst). Hier geht es nur um
// sofortiges Feedback beim Tippen.
//
// Sicherheit: Es wird ZUERST jeglicher HTML-Input escaped, danach werden nur
// eigene, kontrollierte Tags ergänzt. Damit ist {@html mdPreview(x)} XSS-sicher,
// auch wenn der Nutzer rohes HTML eintippt. Alle Ersetzungen unten setzen nur
// Teile des bereits escapten Texts in feste Tags ohne Attribute ein; die Ausgabe
// steht nie in einem Attribut (Anführungszeichen werden trotzdem escaped).
//
// Hervorhebung wie im PDF (pulldown-cmark, CommonMark): `**fett**` und
// `__fett__`, `*kursiv*` und `_kursiv_`. Unterstriche wirken nicht innerhalb
// eines Worts (`snake_case`, `Datei_v2_final` bleiben Text), Sternchen schon.
// In Code-Spannen (`…`) wird nichts hervorgehoben (U4).

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// Wortzeichen für die Unterstrich-Regel (Buchstaben/Ziffern aller Schriften und
// der Unterstrich selbst): `_` davor bzw. danach heißt „mitten im Wort“.
const W = "\\p{L}\\p{N}_";
// Inhalt einer Hervorhebung: beginnt und endet nicht mit Leerraum (CommonMark:
// links-/rechtsbündiger Begrenzer); `_` innerhalb eines Worts darf vorkommen.
const BODY = "([^\\s_](?:.*?[^\\s_])??)";
const STRONG_UNDERSCORE = new RegExp(`(^|[^${W}])__${BODY}__(?![${W}])`, "gu");
const EM_UNDERSCORE = new RegExp(`(^|[^${W}])_${BODY}_(?![${W}])`, "gu");

function emphasis(t: string): string {
  return t
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(STRONG_UNDERSCORE, "$1<strong>$2</strong>")
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    .replace(EM_UNDERSCORE, "$1<em>$2</em>");
}

function inline(t: string): string {
  // Code-Spannen zuerst herauslösen; Hervorhebung nur im Text dazwischen.
  return t
    .split(/(`[^`]+`)/)
    .map((part) => (/^`[^`]+`$/.test(part) ? `<code>${part.slice(1, -1)}</code>` : emphasis(part)))
    .join("");
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
