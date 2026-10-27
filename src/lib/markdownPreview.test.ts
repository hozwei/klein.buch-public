// U4 (U1-067/U3-038 Rest): Live-Vorschau des Paket-Markups — Hervorhebung wie im
// PDF (pulldown-cmark) und Escaping vor jeder Ersetzung.
import { describe, expect, it } from "vitest";
import { mdPreview } from "./markdownPreview.ts";

describe("mdPreview – Hervorhebung", () => {
  it("setzt _text_ und *text* kursiv, **text** und __text__ fett", () => {
    expect(mdPreview("Ein _kursives_ Wort")).toBe("<p>Ein <em>kursives</em> Wort</p>");
    expect(mdPreview("Ein *kursives* Wort")).toBe("<p>Ein <em>kursives</em> Wort</p>");
    expect(mdPreview("**fett** und __auch fett__")).toBe("<p><strong>fett</strong> und <strong>auch fett</strong></p>");
    expect(mdPreview("_ganze Zeile_")).toBe("<p><em>ganze Zeile</em></p>");
    expect(mdPreview("(_in Klammern_), _a_ _b_")).toBe("<p>(<em>in Klammern</em>), <em>a</em> <em>b</em></p>");
  });

  it("lässt Unterstriche mitten im Wort stehen", () => {
    expect(mdPreview("snake_case und Datei_v2_final")).toBe("<p>snake_case und Datei_v2_final</p>");
    expect(mdPreview("Größe_neu_")).toBe("<p>Größe_neu_</p>");
    expect(mdPreview("_snake_case_")).toBe("<p><em>snake_case</em></p>");
    expect(mdPreview("_ kein Kursiv _")).toBe("<p>_ kein Kursiv _</p>");
  });

  it("hebt in Code-Spannen nichts hervor", () => {
    expect(mdPreview("Feld `_intern_` und `*x*`")).toBe("<p>Feld <code>_intern_</code> und <code>*x*</code></p>");
  });

  it("wirkt auch in Überschriften und Listen", () => {
    expect(mdPreview("## Paket _Basis_\n- Punkt _eins_\n1. Schritt __zwei__")).toBe(
      "<h2>Paket <em>Basis</em></h2><ul><li>Punkt <em>eins</em></li></ul><ol><li>Schritt <strong>zwei</strong></li></ol>",
    );
  });
});

describe("mdPreview – Sicherheit", () => {
  it("escapt HTML vor jeder Ersetzung, auch innerhalb einer Hervorhebung", () => {
    expect(mdPreview("<script>alert(1)</script>")).toBe("<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>");
    expect(mdPreview('_<img src=x onerror="a()">_')).toBe(
      "<p><em>&lt;img src=x onerror=&quot;a()&quot;&gt;</em></p>",
    );
    expect(mdPreview("`<b>` & 'x'")).toBe("<p><code>&lt;b&gt;</code> &amp; &#39;x&#39;</p>");
  });

  it("erzeugt nur eigene Tags ohne Attribute", () => {
    const html = mdPreview('# _a_ **b**\n- `c` *d* __e__\n<a href="javascript:x">f</a>');
    const tags = html.match(/<[^>]+>/g) ?? [];
    for (const t of tags) expect(t).toMatch(/^<\/?(p|h[1-3]|ul|ol|li|em|strong|code)>$/);
  });
});
