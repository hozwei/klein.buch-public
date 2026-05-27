//! `domain::package` — Functional Core für den Paket-Katalog (Block P2).
//!
//! Markup-Pipeline: **ein Markdown-Subset, zwei Ziele** — ein formatierter
//! Typst-Block fürs PDF (`to_typst`) und BT-154-tauglicher Klartext fürs
//! XRechnung-XML (`to_plaintext`). Pure, kein I/O.
//!
//! ## Sicherheit (Pflicht, gleiche Sorgfalt wie `pdf::klausel_check`)
//!
//! `to_typst` baut **ausschließlich aus dem geparsten pulldown-cmark-AST**.
//! Struktur entsteht nur aus Typst-**Funktionsaufrufen** (`#heading`, `#strong`,
//! `#emph`, `#list`, `#enum`, `#table`, `#raw`); jeder vom Nutzer stammende
//! Text-Run wird mit [`esc_typst`] gegen Typst-Steuerzeichen escaped. Damit kann
//! kein Markup-Inhalt zu Typst-Code werden (Code-Injection-Schutz). `Event::Html`
//! / `Event::InlineHtml` werden **verworfen** — kein HTML-Passthrough.
//!
//! Render-Kontrakt (P3): die Ausgabe von `to_typst` ist als Typst-**Markup**
//! gedacht und wird im Template via `eval(s, mode: "markup")` eingebettet.

use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Eingabe + Validierung
// ---------------------------------------------------------------------------

/// EN-16931-Tax-Category-Codes (wie in der Migration als CHECK hinterlegt).
pub const VALID_TAX_CATEGORY_CODES: [&str; 9] = ["S", "Z", "E", "AE", "K", "G", "O", "L", "M"];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRevisionInput {
    pub title: String,
    pub body_markup: String,
    pub default_unit_price_cents: i64,
    pub unit_code: String,
    pub tax_category_code: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
}

/// Reine Validierung einer Paket-Revision (vor dem Schreiben). §19-Erzwingung
/// (`'E'`/0) macht die Shell beim Materialisieren — hier nur strukturelle Checks.
pub fn validate_revision(input: &PackageRevisionInput) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if input.title.trim().is_empty() {
        issues.push(ValidationIssue {
            code: "title_empty".into(),
            message: "Der Paket-Titel darf nicht leer sein.".into(),
        });
    }
    if input.default_unit_price_cents < 0 {
        issues.push(ValidationIssue {
            code: "price_negative".into(),
            message: "Der Netto-Preis darf nicht negativ sein.".into(),
        });
    }
    if input.unit_code.trim().is_empty() {
        issues.push(ValidationIssue {
            code: "unit_empty".into(),
            message: "Die Einheit darf nicht leer sein.".into(),
        });
    }
    if !VALID_TAX_CATEGORY_CODES.contains(&input.tax_category_code.as_str()) {
        issues.push(ValidationIssue {
            code: "tax_category_invalid".into(),
            message: format!(
                "Unbekannter Steuer-Kategorie-Code '{}'.",
                input.tax_category_code
            ),
        });
    }
    issues
}

// ---------------------------------------------------------------------------
// Geparstes Dokument (AST) — einmal parsen, zwei Ziele
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emph(Vec<Inline>),
    Code(String),
    SoftBreak,
    HardBreak,
}

#[derive(Debug, Clone)]
enum Block {
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
}

/// Geparstes Paket-Markup. Wird einmal beim Parsen aufgebaut; `to_typst` und
/// `to_plaintext` rendern beide aus demselben AST.
#[derive(Debug, Clone)]
pub struct MarkupDoc {
    blocks: Vec<Block>,
}

fn parser(src: &str) -> Parser<'_> {
    // CommonMark-Basis ohne Erweiterungen — bewusst KEINE Tabellen/Footnotes/etc.
    // (Manuel-Entscheidung 2026-05-23: keine Tabellen). `|…|` bleibt normaler Text.
    Parser::new(src)
}

pub fn parse_markup(src: &str) -> MarkupDoc {
    let mut p = parser(src);
    MarkupDoc {
        blocks: build_blocks(&mut p),
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Konsumiert Block-Level-Events bis zum Stream-Ende (Top-Level).
fn build_blocks(p: &mut Parser<'_>) -> Vec<Block> {
    let mut blocks = Vec::new();
    while let Some(ev) = p.next() {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                blocks.push(Block::Heading(heading_level(level), build_inlines(p)));
            }
            Event::Start(Tag::Paragraph) => {
                blocks.push(Block::Paragraph(build_inlines(p)));
            }
            Event::Start(Tag::List(start)) => {
                blocks.push(build_list(p, start.is_some()));
            }
            // Nicht unterstützte Block-Container (Blockquote, CodeBlock, …):
            // Inhalt überspringen, nichts ausgeben.
            Event::Start(_) => skip_until_matching_end(p),
            _ => {}
        }
    }
    blocks
}

/// Liest Inline-Events bis zum schließenden `End` des aktuellen Containers
/// (Caller hat das `Start` bereits konsumiert). Verschachtelte Inline-Container
/// (Strong/Emph) werden rekursiv eingesammelt.
fn build_inlines(p: &mut Parser<'_>) -> Vec<Inline> {
    let mut out = Vec::new();
    while let Some(ev) = p.next() {
        match ev {
            Event::Text(t) => out.push(Inline::Text(t.to_string())),
            Event::Code(t) => out.push(Inline::Code(t.to_string())),
            Event::SoftBreak => out.push(Inline::SoftBreak),
            Event::HardBreak => out.push(Inline::HardBreak),
            Event::Start(Tag::Strong) => out.push(Inline::Strong(build_inlines(p))),
            Event::Start(Tag::Emphasis) => out.push(Inline::Emph(build_inlines(p))),
            // Links: nur den Text behalten (keine klickbaren Links).
            Event::Start(Tag::Link { .. }) => out.extend(build_inlines(p)),
            // Bilder, Code-/sonstige Container: Inhalt verwerfen.
            Event::Start(_) => skip_until_matching_end(p),
            // Kein HTML-Passthrough.
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::End(_) => break,
            _ => {}
        }
    }
    out
}

fn build_list(p: &mut Parser<'_>, ordered: bool) -> Block {
    let mut items = Vec::new();
    while let Some(ev) = p.next() {
        match ev {
            Event::Start(Tag::Item) => items.push(build_item(p)),
            Event::End(TagEnd::List(_)) => break,
            Event::End(_) => break,
            _ => {}
        }
    }
    Block::List { ordered, items }
}

/// Inhalt eines Listen-Items bis `End(Item)`. Unterstützt sowohl „tight" Listen
/// (direkte Inlines) als auch „loose" (Absätze) und einfache Unterlisten.
fn build_item(p: &mut Parser<'_>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut pending: Vec<Inline> = Vec::new();
    macro_rules! flush {
        () => {
            if !pending.is_empty() {
                blocks.push(Block::Paragraph(std::mem::take(&mut pending)));
            }
        };
    }
    while let Some(ev) = p.next() {
        match ev {
            Event::End(TagEnd::Item) => break,
            Event::Start(Tag::Paragraph) => {
                flush!();
                blocks.push(Block::Paragraph(build_inlines(p)));
            }
            Event::Start(Tag::List(start)) => {
                flush!();
                blocks.push(build_list(p, start.is_some()));
            }
            // Tight-List-Inlines:
            Event::Text(t) => pending.push(Inline::Text(t.to_string())),
            Event::Code(t) => pending.push(Inline::Code(t.to_string())),
            Event::SoftBreak => pending.push(Inline::SoftBreak),
            Event::HardBreak => pending.push(Inline::HardBreak),
            Event::Start(Tag::Strong) => pending.push(Inline::Strong(build_inlines(p))),
            Event::Start(Tag::Emphasis) => pending.push(Inline::Emph(build_inlines(p))),
            Event::Start(Tag::Link { .. }) => pending.extend(build_inlines(p)),
            Event::Start(_) => skip_until_matching_end(p),
            Event::Html(_) | Event::InlineHtml(_) => {}
            _ => {}
        }
    }
    flush!();
    blocks
}

/// Überspringt Events bis zum `End`, das das zuletzt offene `Start` schließt.
fn skip_until_matching_end(p: &mut Parser<'_>) {
    let mut depth = 1i32;
    for ev in p.by_ref() {
        match ev {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Ziel 1: Typst (AST-only, escaped)
// ---------------------------------------------------------------------------

/// Escaped einen Klartext-Run gegen Typst-Steuerzeichen. Kein Zeichen darf als
/// Typst-Syntax interpretiert werden (Injection-Schutz).
fn esc_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            '@' => out.push_str("\\@"),
            '`' => out.push_str("\\`"),
            _ => out.push(c),
        }
    }
    out
}

/// Typst-String-Literal (für `#raw("…")`): nur `"` und `\` escapen.
fn typst_string_lit(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn typst_inlines(inls: &[Inline]) -> String {
    let mut s = String::new();
    for i in inls {
        match i {
            Inline::Text(t) => s.push_str(&esc_typst(t)),
            Inline::Strong(c) => {
                s.push_str("#strong[");
                s.push_str(&typst_inlines(c));
                s.push(']');
            }
            Inline::Emph(c) => {
                s.push_str("#emph[");
                s.push_str(&typst_inlines(c));
                s.push(']');
            }
            Inline::Code(t) => {
                s.push_str("#raw(");
                s.push_str(&typst_string_lit(t));
                s.push(')');
            }
            Inline::SoftBreak => s.push(' '),
            Inline::HardBreak => s.push_str(" \\ "),
        }
    }
    s
}

fn typst_block(b: &Block, out: &mut String) {
    match b {
        Block::Heading(level, inl) => {
            out.push_str(&format!(
                "#heading(level: {})[{}]\n\n",
                level.clamp(&1, &6),
                typst_inlines(inl)
            ));
        }
        Block::Paragraph(inl) => {
            out.push_str(&typst_inlines(inl));
            out.push_str("\n\n");
        }
        Block::List { ordered, items } => {
            out.push_str(if *ordered { "#enum(\n" } else { "#list(\n" });
            for item in items {
                let mut buf = String::new();
                for ib in item {
                    typst_block(ib, &mut buf);
                }
                out.push_str("  [");
                out.push_str(buf.trim());
                out.push_str("],\n");
            }
            out.push_str(")\n\n");
        }
    }
}

pub fn to_typst(doc: &MarkupDoc) -> String {
    let mut out = String::new();
    for b in &doc.blocks {
        typst_block(b, &mut out);
    }
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Ziel 2: Klartext (BT-154-tauglich)
// ---------------------------------------------------------------------------

fn plain_inlines(inls: &[Inline]) -> String {
    let mut s = String::new();
    for i in inls {
        match i {
            Inline::Text(t) => s.push_str(t),
            Inline::Strong(c) | Inline::Emph(c) => s.push_str(&plain_inlines(c)),
            Inline::Code(t) => s.push_str(t),
            Inline::SoftBreak => s.push(' '),
            Inline::HardBreak => s.push(' '),
        }
    }
    s
}

fn plain_block(b: &Block, lines: &mut Vec<String>) {
    match b {
        Block::Heading(_, inl) | Block::Paragraph(inl) => {
            lines.push(plain_inlines(inl).trim().to_string());
        }
        Block::List { ordered, items } => {
            for (idx, item) in items.iter().enumerate() {
                let mut buf: Vec<String> = Vec::new();
                for ib in item {
                    plain_block(ib, &mut buf);
                }
                let text = buf.join(" ").trim().to_string();
                if *ordered {
                    lines.push(format!("{}. {}", idx + 1, text));
                } else {
                    lines.push(format!("- {text}"));
                }
            }
        }
    }
}

pub fn to_plaintext(doc: &MarkupDoc) -> String {
    let mut lines: Vec<String> = Vec::new();
    for b in &doc.blocks {
        plain_block(b, &mut lines);
    }
    lines
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn typ(src: &str) -> String {
        to_typst(&parse_markup(src))
    }
    fn plain(src: &str) -> String {
        to_plaintext(&parse_markup(src))
    }

    #[test]
    fn heading_bold_italic_to_typst() {
        let t = typ("# Hochzeit klein\n\nMit **viel** und *etwas*.");
        assert!(t.contains("#heading(level: 1)[Hochzeit klein]"));
        assert!(t.contains("#strong[viel]"));
        assert!(t.contains("#emph[etwas]"));
    }

    #[test]
    fn unordered_list_to_typst_and_plain() {
        let src = "- Vorbesprechung\n- Shooting\n- Bildauswahl";
        let t = typ(src);
        assert!(t.contains("#list("));
        assert!(t.contains("[Vorbesprechung]"));
        assert!(t.contains("[Bildauswahl]"));
        let p = plain(src);
        assert!(p.contains("- Vorbesprechung"));
        assert!(p.contains("- Shooting"));
    }

    #[test]
    fn ordered_list_uses_enum() {
        let t = typ("1. Erstens\n2. Zweitens");
        assert!(t.contains("#enum("));
        assert!(t.contains("[Erstens]"));
    }

    #[test]
    fn html_is_not_passed_through() {
        let t = typ("Text <script>alert(1)</script> Ende");
        // Roh-HTML-Tags werden verworfen (kein Passthrough); der dazwischen
        // liegende Klartext bleibt als harmloser, escapeter Text bestehen.
        assert!(!t.contains("<script>"), "HTML-Tag durchgerutscht: {t}");
        assert!(!t.contains("</script>"));
        assert!(t.contains("Text") && t.contains("Ende"));
    }

    #[test]
    fn typst_injection_in_text_is_escaped() {
        // Ein Versuch, Typst-Code einzuschleusen, landet escaped — nie als Aufruf.
        let t = typ(r#"Achtung #read("/etc/passwd") und [link]"#);
        // Der eingeschleuste Aufruf landet escaped (`\#read(`), nie als echter
        // Typst-Funktionsaufruf; eckige Klammern ebenfalls escaped.
        assert!(t.contains("\\#read("), "‚#read(' muss escaped sein: {t}");
        assert!(t.contains("\\[link\\]"), "‚[link]' muss escaped sein: {t}");
    }

    #[test]
    fn empty_markup_is_empty() {
        assert_eq!(typ(""), "");
        assert_eq!(plain(""), "");
        assert_eq!(typ("   \n  "), "");
    }

    #[test]
    fn plaintext_idempotent_on_plain_input() {
        assert_eq!(
            plain("Nur ein schlichter Satz."),
            "Nur ein schlichter Satz."
        );
    }

    #[test]
    fn plaintext_strips_emphasis_markers() {
        let p = plain("Mit **fett** und *kursiv*.");
        assert_eq!(p, "Mit fett und kursiv.");
        assert!(!p.contains('*'));
    }

    #[test]
    fn validate_revision_flags_problems() {
        let bad = PackageRevisionInput {
            title: "  ".into(),
            body_markup: "x".into(),
            default_unit_price_cents: -1,
            unit_code: "".into(),
            tax_category_code: "ZZ".into(),
            note: None,
        };
        let issues = validate_revision(&bad);
        let codes: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"title_empty"));
        assert!(codes.contains(&"price_negative"));
        assert!(codes.contains(&"unit_empty"));
        assert!(codes.contains(&"tax_category_invalid"));

        let good = PackageRevisionInput {
            title: "Hochzeit klein".into(),
            body_markup: "# Titel".into(),
            default_unit_price_cents: 90000,
            unit_code: "C62".into(),
            tax_category_code: "E".into(),
            note: None,
        };
        assert!(validate_revision(&good).is_empty());
    }
}
