//! Tera-Template-Render für Mail-Subject + Body (Block 5).
//!
//! Schicht: überwiegend pure (Render + Parsing). Nur [`load_template`] macht
//! I/O (liest `inputs/mail-templates/{name}.txt`).
//!
//! ## Template-Konvention
//!
//! Das Template enthält in der **ersten Zeile** `Subject: …`. Diese Zeile wird
//! nach dem Rendern abgespalten und als Betreff verwendet; der Rest (ohne
//! führende Leerzeilen) ist der Body. Beispiel:
//! `inputs/mail-templates/invoice-de.txt`.
//!
//! KRITISCH: `inputs/` ist menschen-maintained und wird **nie** beschrieben.

use std::path::Path;

use serde::Serialize;

use crate::db::models::{InvoiceRow, QuoteRow, SellerProfileRow};
use crate::error::{Error, Result};

/// Render-Ergebnis: aufgespaltener Betreff + Body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedMail {
    pub subject: String,
    pub body: String,
}

// ---- Template-Kontext ------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct MailContext {
    pub invoice: InvoiceCtx,
    pub seller: SellerCtx,
    pub kleinunternehmer: KleinCtx,
}

#[derive(Debug, Clone, Serialize)]
pub struct InvoiceCtx {
    pub number: String,
    pub date: String,
    pub gross_amount_formatted: String,
    pub due_date: Option<String>,
    pub is_kleinunternehmer: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SellerCtx {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KleinCtx {
    pub hinweis_text: String,
}

/// Liest `inputs/mail-templates/{name}.txt`. Default-Name ist `invoice-de`.
pub fn load_template(inputs_dir: &Path, template_name: &str) -> Result<String> {
    let path = inputs_dir
        .join("mail-templates")
        .join(format!("{template_name}.txt"));
    std::fs::read_to_string(&path).map_err(|e| {
        Error::Config(format!(
            "Mail-Template '{template_name}' nicht ladbar ({}): {e}",
            path.display()
        ))
    })
}

/// Baut den Render-Kontext aus einer Rechnung + Verkäuferprofil. Beträge und
/// Daten werden hier deutsch formatiert, damit das Template „dumm" bleibt.
pub fn build_invoice_context(
    invoice: &InvoiceRow,
    seller: &SellerProfileRow,
    hinweis_text: &str,
) -> MailContext {
    MailContext {
        invoice: InvoiceCtx {
            number: invoice.invoice_number.clone(),
            date: format_german_date(&invoice.invoice_date),
            gross_amount_formatted: format_eur_cents(
                invoice.gross_amount_cents,
                &invoice.currency_code,
            ),
            due_date: invoice.due_date.as_deref().map(format_german_date),
            is_kleinunternehmer: invoice.is_kleinunternehmer == 1,
        },
        seller: SellerCtx {
            name: seller.name.clone(),
            email: seller.email.clone(),
            phone: seller.phone.clone(),
        },
        kleinunternehmer: KleinCtx {
            hinweis_text: hinweis_text.to_string(),
        },
    }
}

/// Rendert das Template und spaltet Betreff + Body.
///
/// `autoescape = false` — es ist eine Plain-Text-Mail, kein HTML.
pub fn render(template_source: &str, ctx: &MailContext) -> Result<RenderedMail> {
    render_ctx(template_source, ctx)
}

/// Generischer Render + Subject/Body-Split für beliebige (serialisierbare)
/// Mail-Kontexte. `autoescape = false` — Plain-Text-Mail, kein HTML.
fn render_ctx<C: Serialize>(template_source: &str, ctx: &C) -> Result<RenderedMail> {
    let context = tera::Context::from_serialize(ctx)
        .map_err(|e| Error::Mail(format!("Mail-Kontext nicht serialisierbar: {e}")))?;
    let rendered = tera::Tera::one_off(template_source, &context, false)
        .map_err(|e| Error::Mail(format!("Mail-Template-Render fehlgeschlagen: {e}")))?;
    split_subject_body(&rendered)
}

// ---- Angebots-Mail (Block 8) -----------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct QuoteMailContext {
    pub quote: QuoteCtx,
    pub seller: SellerCtx,
    pub kleinunternehmer: KleinCtx,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteCtx {
    pub number: String,
    pub date: String,
    pub valid_until: String,
    pub gross_amount_formatted: String,
    pub is_kleinunternehmer: bool,
}

/// Eingebettetes Default-Angebots-Mailtemplate. `inputs/` ist nach Block 1 für
/// Maschinen tabu — daher das Default im Binary; Override per
/// `inputs/mail-templates/quote-de.txt` möglich (menschen-maintained).
pub const DEFAULT_QUOTE_MAIL: &str = "Subject: Angebot {{ quote.number }} von {{ seller.name }}\n\
\n\
Sehr geehrte Damen und Herren,\n\
\n\
anbei erhalten Sie unser Angebot {{ quote.number }} vom {{ quote.date }} über \
{{ quote.gross_amount_formatted }}, gültig bis {{ quote.valid_until }}.\n\
\n\
Im Anhang finden Sie zusätzlich unsere AGB sowie unsere Datenschutzhinweise.\n\
{% if quote.is_kleinunternehmer -%}\n\
{{ kleinunternehmer.hinweis_text }}\n\
{% endif -%}\n\
\n\
Für Rückfragen stehen wir gerne zur Verfügung.\n\
\n\
Mit freundlichen Grüßen\n\
{{ seller.name }}\n\
\n\
{{ seller.email }}\n\
{% if seller.phone -%}{{ seller.phone }}{% endif -%}\n";

/// Lädt das Angebots-Mailtemplate: bevorzugt
/// `inputs/mail-templates/quote-de.txt`, sonst das eingebettete Default.
pub fn load_quote_template(inputs_dir: &Path) -> String {
    let path = inputs_dir.join("mail-templates").join("quote-de.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_QUOTE_MAIL.to_string())
}

/// Baut den Render-Kontext einer Angebots-Mail aus Angebot + Verkäuferprofil.
pub fn build_quote_context(
    quote: &QuoteRow,
    seller: &SellerProfileRow,
    hinweis_text: &str,
) -> QuoteMailContext {
    QuoteMailContext {
        quote: QuoteCtx {
            number: quote.quote_number.clone(),
            date: format_german_date(&quote.quote_date),
            valid_until: format_german_date(&quote.valid_until),
            gross_amount_formatted: format_eur_cents(
                quote.gross_amount_cents,
                &quote.currency_code,
            ),
            is_kleinunternehmer: quote.is_kleinunternehmer == 1,
        },
        seller: SellerCtx {
            name: seller.name.clone(),
            email: seller.email.clone(),
            phone: seller.phone.clone(),
        },
        kleinunternehmer: KleinCtx {
            hinweis_text: hinweis_text.to_string(),
        },
    }
}

/// Rendert eine Angebots-Mail (Betreff + Body).
pub fn render_quote_mail(template_source: &str, ctx: &QuoteMailContext) -> Result<RenderedMail> {
    render_ctx(template_source, ctx)
}

/// Trennt die `Subject:`-Zeile vom Body ab. Erste nicht-leere Zeile MUSS mit
/// `Subject:` (case-insensitive) beginnen.
fn split_subject_body(rendered: &str) -> Result<RenderedMail> {
    let mut lines = rendered.lines();
    let subject_line = lines
        .by_ref()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| Error::Mail("Mail-Template ist leer".into()))?;

    let trimmed = subject_line.trim_start();
    let subject = trimmed
        .strip_prefix("Subject:")
        .or_else(|| trimmed.strip_prefix("subject:"))
        .ok_or_else(|| {
            Error::Mail("Mail-Template: erste Zeile muss mit 'Subject:' beginnen".into())
        })?
        .trim()
        .to_string();

    // Body: alles nach der Subject-Zeile, führende Leerzeilen entfernt.
    let body = lines.collect::<Vec<_>>().join("\n");
    let body = body.trim_start_matches('\n').trim_start().to_string();

    Ok(RenderedMail { subject, body })
}

// ---- Formatierung ----------------------------------------------------------

/// Cent-Betrag → deutsche Schreibweise, z. B. `1.234,56 €`. Nicht-EUR-Währungen
/// zeigen den ISO-Code statt Symbol.
pub fn format_eur_cents(cents: i64, currency_code: &str) -> String {
    let negative = cents < 0;
    let abs = cents.unsigned_abs();
    let euros = abs / 100;
    let rem = abs % 100;
    let grouped = group_thousands(euros);
    let symbol = if currency_code.eq_ignore_ascii_case("EUR") {
        "€".to_string()
    } else {
        currency_code.to_string()
    };
    format!(
        "{}{},{:02} {}",
        if negative { "-" } else { "" },
        grouped,
        rem,
        symbol
    )
}

/// `1234567` → `1.234.567` (deutsche Tausenderpunkte).
fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(*b as char);
    }
    out
}

/// `2026-05-20` → `20.05.2026`. Akzeptiert auch `YYYY-MM-DD HH:MM:SS` und
/// nimmt die ersten 10 Zeichen. Bei unerwartetem Format: unverändert zurück.
pub fn format_german_date(iso: &str) -> String {
    let date_part = &iso[..iso.len().min(10)];
    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() == 3 && parts[0].len() == 4 {
        format!("{}.{}.{}", parts[2], parts[1], parts[0])
    } else {
        iso.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> MailContext {
        MailContext {
            invoice: InvoiceCtx {
                number: "RE-2026-0001".into(),
                date: "20.05.2026".into(),
                gross_amount_formatted: "595,00 €".into(),
                due_date: Some("19.06.2026".into()),
                is_kleinunternehmer: true,
            },
            seller: SellerCtx {
                name: "Wildbach Computerhilfe".into(),
                email: "schmidm@wildbach-computerhilfe.de".into(),
                phone: Some("+49 871 1234567".into()),
            },
            kleinunternehmer: KleinCtx {
                hinweis_text: "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.".into(),
            },
        }
    }

    const TEMPLATE: &str = "Subject: Rechnung {{ invoice.number }} von {{ seller.name }}\n\
\n\
Sehr geehrte Damen und Herren,\n\
\n\
anbei Rechnung {{ invoice.number }} über {{ invoice.gross_amount_formatted }}.\n\
{% if invoice.is_kleinunternehmer -%}\n\
{{ kleinunternehmer.hinweis_text }}\n\
{% endif -%}\n";

    #[test]
    fn render_splits_subject_and_body() {
        let r = render(TEMPLATE, &ctx()).unwrap();
        assert_eq!(
            r.subject,
            "Rechnung RE-2026-0001 von Wildbach Computerhilfe"
        );
        assert!(r.body.starts_with("Sehr geehrte Damen und Herren,"));
        assert!(r.body.contains("595,00 €"));
        assert!(r
            .body
            .contains("Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."));
        // Subject darf nicht im Body auftauchen.
        assert!(!r.body.contains("Subject:"));
    }

    #[test]
    fn render_errors_without_subject_line() {
        let r = render("Kein Betreff hier\nBody", &ctx());
        assert!(r.is_err());
    }

    #[test]
    fn format_eur_cents_german() {
        assert_eq!(format_eur_cents(59500, "EUR"), "595,00 €");
        assert_eq!(format_eur_cents(123456789, "EUR"), "1.234.567,89 €");
        assert_eq!(format_eur_cents(5, "EUR"), "0,05 €");
        assert_eq!(format_eur_cents(-1099, "EUR"), "-10,99 €");
        assert_eq!(format_eur_cents(100000, "CHF"), "1.000,00 CHF");
    }

    #[test]
    fn format_german_date_variants() {
        assert_eq!(format_german_date("2026-05-20"), "20.05.2026");
        assert_eq!(format_german_date("2026-05-20 13:45:00"), "20.05.2026");
        assert_eq!(format_german_date("kaputt"), "kaputt");
    }

    #[test]
    fn quote_mail_renders_subject_body_and_klausel() {
        let ctx = QuoteMailContext {
            quote: QuoteCtx {
                number: "AN-2026-0001".into(),
                date: "20.05.2026".into(),
                valid_until: "19.06.2026".into(),
                gross_amount_formatted: "595,00 €".into(),
                is_kleinunternehmer: true,
            },
            seller: SellerCtx {
                name: "Wildbach Computerhilfe".into(),
                email: "schmidm@wildbach-computerhilfe.de".into(),
                phone: None,
            },
            kleinunternehmer: KleinCtx {
                hinweis_text: "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.".into(),
            },
        };
        let r = render_quote_mail(DEFAULT_QUOTE_MAIL, &ctx).unwrap();
        assert_eq!(r.subject, "Angebot AN-2026-0001 von Wildbach Computerhilfe");
        assert!(r.body.contains("gültig bis 19.06.2026"));
        assert!(r.body.contains("AGB"));
        assert!(r
            .body
            .contains("Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."));
        assert!(!r.body.contains("Subject:"));
    }
}
