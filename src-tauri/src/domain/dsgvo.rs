//! Functional Core für die **DSGVO-Auskunft nach Art. 15 DSGVO** (Block 18).
//!
//! Reine, deterministische Aufbereitung: nimmt die von der Shell
//! ([`crate::db::repo::dsgvo`]) gesammelten Roh-Zeilen zu **genau einem Kontakt**
//! und baut daraus einen serialisierbaren [`DataSubjectReport`]. Kein I/O, keine
//! DB, kein Dateisystem.
//!
//! Der Report ist die **einzige Wahrheit**: er wird sowohl als maschinenlesbares
//! JSON (Art. 20 Datenportabilität) als auch — über `serde_json::to_value` — als
//! Daten-JSON für das Typst-PDF (lesbare Auskunft, Art. 15) verwendet. Geld
//! bleibt als Integer-Cent; die PDF-Vorlage formatiert selbst (Konvention:
//! „Template rechnet/formatiert die Anzeige, der Core liefert Rohwerte").
//!
//! ## Entscheidungen (Block 18, 2026-05-24, mit Manuel abgestimmt)
//! - **Interne Notizen werden ausgelassen** ([`INCLUDE_INTERNAL_NOTES`]). Der
//!   rechtliche Vorbehalt steht im [`DataSubjectReport::disclaimer`].
//! - **Privatbewegungen** (`private_movements`) sind **nicht** enthalten — die
//!   Tabelle hat keinen Kontaktbezug (kein `contact_id`), also keinen
//!   Personenbezug zu einem Kontakt.
//! - Die Art.-15(1)-Pflichtangaben (Zwecke, Rechtsgrundlagen, Empfänger,
//!   Speicherdauer, Betroffenenrechte) liefert [`ProcessingInfo::standard`] als
//!   vorausgefüllten Standardtext — juristisch gegenzuprüfen (PRD R4.3).

use serde::Serialize;

use crate::db::models::{
    ContactRow, EmailLogRow, ExpenseRow, InvoiceItemRow, InvoiceRow, QuoteItemRow, QuoteRow,
};

/// Steuert, ob rein interne `notes`-Felder (Kontakt/Rechnung/Angebot/Kosten) in
/// die Auskunft aufgenommen werden. Manuel-Entscheidung Block 18: **false**.
/// Bewusst eine sichtbare Konstante statt einer stillen Auslassung.
pub const INCLUDE_INTERNAL_NOTES: bool = false;

// ============================================================================
// Eingabe (von der Shell befüllt) — Borrows, damit der Core nichts kopiert.
// ============================================================================

/// Eine Rechnung samt Positionen.
pub struct RawInvoice<'a> {
    pub invoice: &'a InvoiceRow,
    pub items: &'a [InvoiceItemRow],
}

/// Ein Angebot samt Positionen.
pub struct RawQuote<'a> {
    pub quote: &'a QuoteRow,
    pub items: &'a [QuoteItemRow],
}

/// Metadaten eines archivierten Dokuments (PDF/XML/Beleg/Anhang). Die Bytes
/// selbst bündelt die Shell ins ZIP; der Core kennt nur die Metadaten.
#[derive(Debug, Clone)]
pub struct RawDocument {
    pub kind: String,
    pub related_label: Option<String>,
    pub archive_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub created_at: String,
}

/// Ein Audit-Log-Bezug (gefiltert auf den Kontakt + seine Belege). Bewusst ohne
/// `details_json` — das ist interner technischer Kontext, kein Personendatum.
#[derive(Debug, Clone)]
pub struct RawAudit {
    pub timestamp_utc: String,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

/// Gesamte Eingabe für [`assemble`].
pub struct RawData<'a> {
    pub contact: &'a ContactRow,
    pub invoices: &'a [RawInvoice<'a>],
    pub quotes: &'a [RawQuote<'a>],
    pub expenses: &'a [ExpenseRow],
    pub documents: &'a [RawDocument],
    pub emails: &'a [EmailLogRow],
    pub audit: &'a [RawAudit],
    /// Verantwortliche Stelle (Verkäuferprofil) — Art. 15 Abs. 1 lit. a.
    pub controller: ControllerInfo,
    /// Erzeugungszeitpunkt (ISO-8601, Europe/Berlin) für den Report-Kopf.
    pub generated_at: String,
}

// ============================================================================
// Ausgabe — der serialisierbare Report (= JSON-Auskunft = PDF-Daten).
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataSubjectReport {
    /// Erzeugungszeitpunkt der Auskunft (ISO-8601).
    pub generated_at: String,
    /// Stammdaten der betroffenen Person/Firma (Kontakt).
    pub subject: SubjectInfo,
    /// Verantwortliche Stelle (Art. 15 Abs. 1 lit. a).
    pub controller: ControllerInfo,
    /// Art.-15(1)-Pflichtangaben (Zwecke/Rechtsgrundlagen/Empfänger/…).
    pub processing_info: ProcessingInfo,
    pub invoices: Vec<InvoiceEntry>,
    pub quotes: Vec<QuoteEntry>,
    /// Kosten, bei denen der Kontakt **Lieferant** ist (Eingangsseite).
    pub expenses: Vec<ExpenseEntry>,
    /// Archivierte Dokumente (PDF/XML/Belege/Anhänge) — Metadaten.
    pub documents: Vec<DocumentEntry>,
    /// Versandprotokoll-Einträge mit Bezug zur Person.
    pub emails: Vec<EmailEntry>,
    /// Protokoll-/Audit-Bezüge.
    pub audit_events: Vec<AuditEventEntry>,
    /// Rechtlicher Hinweis (keine Rechtsberatung, Aufbewahrungspflichten …).
    pub disclaimer: String,
    /// Ob interne Notizen aufgenommen wurden (= [`INCLUDE_INTERNAL_NOTES`]).
    pub notes_included: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectInfo {
    pub id: String,
    pub contact_type: String,
    pub name: String,
    pub legal_form: Option<String>,
    pub vat_id: Option<String>,
    pub tax_number: Option<String>,
    pub street: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country_code: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub accepts_einvoice: bool,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInfo {
    pub name: String,
    pub address: String,
    pub tax_number: Option<String>,
    pub vat_id: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingInfo {
    pub purposes: Vec<String>,
    pub legal_bases: Vec<String>,
    pub recipients: Vec<String>,
    pub retention: String,
    pub rights: Vec<String>,
    pub data_source: String,
}

impl ProcessingInfo {
    /// Vorausgefüllter Standardtext für einen §19-Kleinunternehmer. Juristisch
    /// vor Echtbetrieb zu prüfen (PRD R4.3).
    pub fn standard() -> Self {
        Self {
            purposes: vec![
                "Anbahnung und Abwicklung von Verträgen (Angebote, Rechnungen, Leistungserbringung)"
                    .into(),
                "Erfüllung gesetzlicher Aufbewahrungs- und Buchführungspflichten".into(),
                "Zahlungsabwicklung und Forderungsmanagement".into(),
            ],
            legal_bases: vec![
                "Art. 6 Abs. 1 lit. b DSGVO (Vertragserfüllung)".into(),
                "Art. 6 Abs. 1 lit. c DSGVO (rechtliche Verpflichtung, u. a. § 147 AO, § 14b UStG)"
                    .into(),
            ],
            recipients: vec![
                "Steuerberater (sofern beauftragt)".into(),
                "Finanzbehörden im Rahmen gesetzlicher Pflichten".into(),
                "Kreditinstitute/Zahlungsdienstleister im Rahmen der Zahlungsabwicklung".into(),
            ],
            retention: "Steuerlich relevante Belege werden gemäß § 147 AO und § 14b UStG 10 Jahre \
                 aufbewahrt; eine frühere Löschung ist insoweit gesetzlich ausgeschlossen. \
                 Nicht aufbewahrungspflichtige Stammdaten werden nach Wegfall des Zwecks gesperrt \
                 bzw. anonymisiert."
                .into(),
            rights: vec![
                "Auskunft (Art. 15 DSGVO)".into(),
                "Berichtigung (Art. 16 DSGVO)".into(),
                "Löschung (Art. 17 DSGVO, eingeschränkt durch gesetzliche Aufbewahrungspflichten)"
                    .into(),
                "Einschränkung der Verarbeitung (Art. 18 DSGVO)".into(),
                "Datenübertragbarkeit (Art. 20 DSGVO)".into(),
                "Widerspruch (Art. 21 DSGVO)".into(),
                "Beschwerde bei einer Aufsichtsbehörde (Art. 77 DSGVO)".into(),
            ],
            data_source:
                "Die Daten stammen aus der Geschäftsbeziehung — überwiegend von Ihnen selbst \
                 bzw. im Rahmen der Vertrags- und Zahlungsabwicklung erhoben."
                    .into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemEntry {
    pub position: i64,
    pub description: String,
    pub quantity: f64,
    pub unit_code: String,
    pub unit_price_cents: i64,
    pub net_amount_cents: i64,
    pub tax_rate_percent: f64,
    pub tax_category_code: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuyerSnapshot {
    pub name: Option<String>,
    pub street: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub country_code: Option<String>,
    pub vat_id: Option<String>,
    pub email: Option<String>,
}

impl BuyerSnapshot {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.street.is_none()
            && self.postal_code.is_none()
            && self.city.is_none()
            && self.country_code.is_none()
            && self.vat_id.is_none()
            && self.email.is_none()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceEntry {
    pub invoice_number: String,
    pub direction: String,
    pub fiscal_year: i64,
    pub invoice_date: String,
    pub delivery_date: Option<String>,
    pub due_date: Option<String>,
    pub status: String,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub is_kleinunternehmer: bool,
    pub paid_amount_cents: i64,
    pub paid_at: Option<String>,
    pub sent_at: Option<String>,
    pub canceled_at: Option<String>,
    pub cancel_reason: Option<String>,
    /// True, wenn dieser Beleg ein Storno für eine andere Rechnung ist.
    pub is_storno: bool,
    /// Kundenseitiger Bezahlt-/Zahlungshinweis (auf dem PDF gedruckt).
    pub payment_note: Option<String>,
    /// Eingefrorener Empfänger-Stand zur Rechnungszeit (GoBD-Snapshot).
    pub buyer_snapshot: Option<BuyerSnapshot>,
    pub items: Vec<ItemEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteEntry {
    pub quote_number: String,
    pub fiscal_year: i64,
    pub quote_date: String,
    pub valid_until: String,
    pub status: String,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub is_kleinunternehmer: bool,
    pub sent_at: Option<String>,
    pub accepted_at: Option<String>,
    pub rejected_at: Option<String>,
    pub canceled_at: Option<String>,
    pub converted_at: Option<String>,
    pub items: Vec<ItemEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseEntry {
    pub expense_number: String,
    pub fiscal_year: i64,
    pub expense_date: String,
    pub paid_date: Option<String>,
    pub vendor_name_snapshot: String,
    pub vendor_invoice_number: Option<String>,
    pub category: String,
    pub description: String,
    pub net_amount_cents: i64,
    pub tax_amount_cents: i64,
    pub gross_amount_cents: i64,
    pub currency_code: String,
    pub reverse_charge_13b: bool,
    pub status: String,
    pub canceled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEntry {
    pub kind: String,
    pub related_label: Option<String>,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub created_at: String,
    /// Ob die Original-Datei dem ZIP beigelegt wurde (von der Shell gesetzt).
    pub bundled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailEntry {
    pub created_at: String,
    pub channel: String,
    pub related_kind: String,
    pub related_number: Option<String>,
    pub from_email: String,
    pub to_email: String,
    pub subject: String,
    pub attachment_count: i64,
    pub status: String,
    pub provider_code: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventEntry {
    pub timestamp_utc: String,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
}

const DISCLAIMER: &str =
    "Diese Auskunft wurde automatisiert mit Klein.Buch nach Art. 15 DSGVO erstellt und ist \
     keine Rechtsberatung. Ein Teil der enthaltenen Daten unterliegt gesetzlichen \
     Aufbewahrungspflichten (u. a. § 147 AO, § 14b UStG, 10 Jahre) und kann vor Fristablauf nicht \
     gelöscht werden. Rein interne, subjektive Vermerke sind nicht Gegenstand dieser Auskunft. \
     Angaben Dritter in beigefügten Dokumenten sind vor einer Herausgabe zu prüfen (Art. 15 \
     Abs. 4 DSGVO). Verbindlichkeit und Vollständigkeit sind im Einzelfall rechtlich zu prüfen.";

// ============================================================================
// Assembly
// ============================================================================

fn flag(v: i64) -> bool {
    v == 1
}

fn map_invoice_items(items: &[InvoiceItemRow]) -> Vec<ItemEntry> {
    items
        .iter()
        .map(|it| ItemEntry {
            position: it.position,
            description: it.description.clone(),
            quantity: it.quantity,
            unit_code: it.unit_code.clone(),
            unit_price_cents: it.unit_price_cents,
            net_amount_cents: it.net_amount_cents,
            tax_rate_percent: it.tax_rate_percent,
            tax_category_code: it.tax_category_code.clone(),
        })
        .collect()
}

fn map_quote_items(items: &[QuoteItemRow]) -> Vec<ItemEntry> {
    items
        .iter()
        .map(|it| ItemEntry {
            position: it.position,
            description: it.description.clone(),
            quantity: it.quantity,
            unit_code: it.unit_code.clone(),
            unit_price_cents: it.unit_price_cents,
            net_amount_cents: it.net_amount_cents,
            tax_rate_percent: it.tax_rate_percent,
            tax_category_code: it.tax_category_code.clone(),
        })
        .collect()
}

/// Baut den vollständigen Report aus den gesammelten Roh-Daten. Pure.
pub fn assemble(raw: &RawData) -> DataSubjectReport {
    let c = raw.contact;

    let subject = SubjectInfo {
        id: c.id.clone(),
        contact_type: c.contact_type.clone(),
        name: c.name.clone(),
        legal_form: c.legal_form.clone(),
        vat_id: c.vat_id.clone(),
        tax_number: c.tax_number.clone(),
        street: c.street.clone(),
        postal_code: c.postal_code.clone(),
        city: c.city.clone(),
        country_code: c.country_code.clone(),
        email: c.email.clone(),
        phone: c.phone.clone(),
        iban: c.iban.clone(),
        bic: c.bic.clone(),
        accepts_einvoice: flag(c.accepts_einvoice),
        archived: flag(c.archived),
        created_at: c.created_at.clone(),
        updated_at: c.updated_at.clone(),
    };
    // Hinweis: `c.notes` (interner Vermerk) wird bewusst NICHT in `SubjectInfo`
    // übernommen — Block-18-Entscheidung, siehe INCLUDE_INTERNAL_NOTES.

    let invoices = raw
        .invoices
        .iter()
        .map(|ri| {
            let inv = ri.invoice;
            let buyer = BuyerSnapshot {
                name: inv.buyer_name.clone(),
                street: inv.buyer_street.clone(),
                postal_code: inv.buyer_postal_code.clone(),
                city: inv.buyer_city.clone(),
                country_code: inv.buyer_country_code.clone(),
                vat_id: inv.buyer_vat_id.clone(),
                email: inv.buyer_email.clone(),
            };
            InvoiceEntry {
                invoice_number: inv.invoice_number.clone(),
                direction: inv.direction.clone(),
                fiscal_year: inv.fiscal_year,
                invoice_date: inv.invoice_date.clone(),
                delivery_date: inv.delivery_date.clone(),
                due_date: inv.due_date.clone(),
                status: inv.status.clone(),
                net_amount_cents: inv.net_amount_cents,
                tax_amount_cents: inv.tax_amount_cents,
                gross_amount_cents: inv.gross_amount_cents,
                currency_code: inv.currency_code.clone(),
                is_kleinunternehmer: flag(inv.is_kleinunternehmer),
                paid_amount_cents: inv.paid_amount_cents,
                paid_at: inv.paid_at.clone(),
                sent_at: inv.sent_at.clone(),
                canceled_at: inv.canceled_at.clone(),
                cancel_reason: inv.cancel_reason.clone(),
                is_storno: inv.is_storno_for.is_some(),
                payment_note: inv.payment_note.clone(),
                buyer_snapshot: if buyer.is_empty() { None } else { Some(buyer) },
                items: map_invoice_items(ri.items),
            }
        })
        .collect();

    let quotes = raw
        .quotes
        .iter()
        .map(|rq| {
            let q = rq.quote;
            QuoteEntry {
                quote_number: q.quote_number.clone(),
                fiscal_year: q.fiscal_year,
                quote_date: q.quote_date.clone(),
                valid_until: q.valid_until.clone(),
                status: q.status.clone(),
                net_amount_cents: q.net_amount_cents,
                tax_amount_cents: q.tax_amount_cents,
                gross_amount_cents: q.gross_amount_cents,
                currency_code: q.currency_code.clone(),
                is_kleinunternehmer: flag(q.is_kleinunternehmer),
                sent_at: q.sent_at.clone(),
                accepted_at: q.accepted_at.clone(),
                rejected_at: q.rejected_at.clone(),
                canceled_at: q.canceled_at.clone(),
                converted_at: q.converted_at.clone(),
                items: map_quote_items(rq.items),
            }
        })
        .collect();

    let expenses = raw
        .expenses
        .iter()
        .map(|e| ExpenseEntry {
            expense_number: e.expense_number.clone(),
            fiscal_year: e.fiscal_year,
            expense_date: e.expense_date.clone(),
            paid_date: e.paid_date.clone(),
            vendor_name_snapshot: e.vendor_name_snapshot.clone(),
            vendor_invoice_number: e.vendor_invoice_number.clone(),
            category: e.category.clone(),
            description: e.description.clone(),
            net_amount_cents: e.net_amount_cents,
            tax_amount_cents: e.tax_amount_cents,
            gross_amount_cents: e.gross_amount_cents,
            currency_code: e.currency_code.clone(),
            reverse_charge_13b: flag(e.reverse_charge_13b),
            status: e.status.clone(),
            canceled_at: e.canceled_at.clone(),
        })
        .collect();

    let documents = raw
        .documents
        .iter()
        .map(|d| DocumentEntry {
            kind: d.kind.clone(),
            related_label: d.related_label.clone(),
            file_name: d.file_name.clone(),
            mime_type: d.mime_type.clone(),
            size_bytes: d.size_bytes,
            sha256: d.sha256.clone(),
            created_at: d.created_at.clone(),
            bundled: false,
        })
        .collect();

    let emails = raw
        .emails
        .iter()
        .map(|m| EmailEntry {
            created_at: m.created_at.clone(),
            channel: m.channel.clone(),
            related_kind: m.related_kind.clone(),
            related_number: m.related_number.clone(),
            from_email: m.from_email.clone(),
            to_email: m.to_email.clone(),
            subject: m.subject.clone(),
            attachment_count: m.attachment_count,
            status: m.status.clone(),
            provider_code: m.provider_code.clone(),
            error: m.error.clone(),
        })
        .collect();

    let audit_events = raw
        .audit
        .iter()
        .map(|a| AuditEventEntry {
            timestamp_utc: a.timestamp_utc.clone(),
            action: a.action.clone(),
            entity_type: a.entity_type.clone(),
            entity_id: a.entity_id.clone(),
        })
        .collect();

    DataSubjectReport {
        generated_at: raw.generated_at.clone(),
        subject,
        controller: raw.controller.clone(),
        processing_info: ProcessingInfo::standard(),
        invoices,
        quotes,
        expenses,
        documents,
        emails,
        audit_events,
        disclaimer: DISCLAIMER.to_string(),
        notes_included: INCLUDE_INTERNAL_NOTES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact() -> ContactRow {
        ContactRow {
            id: "c1".into(),
            contact_type: "customer".into(),
            name: "Erika Mustermann".into(),
            legal_form: None,
            vat_id: Some("DE123456789".into()),
            tax_number: None,
            street: Some("Hauptstr. 1".into()),
            postal_code: Some("84028".into()),
            city: Some("Landshut".into()),
            country_code: "DE".into(),
            email: Some("erika@example.de".into()),
            phone: Some("0871 12345".into()),
            iban: None,
            bic: None,
            accepts_einvoice: 1,
            archived: 0,
            notes: Some("INTERN: zahlt immer spät".into()),
            created_at: "2026-01-02 10:00:00".into(),
            updated_at: "2026-02-02 10:00:00".into(),
            anonymized_at: None,
        }
    }

    fn invoice() -> InvoiceRow {
        InvoiceRow {
            id: "i1".into(),
            invoice_number: "RE-2026-0001".into(),
            fiscal_year: 2026,
            direction: "issued".into(),
            invoice_date: "2026-03-01".into(),
            delivery_date: Some("2026-02-28".into()),
            due_date: Some("2026-03-15".into()),
            contact_id: "c1".into(),
            seller_name: "Wildbach".into(),
            seller_street: "Weg 2".into(),
            seller_postal_code: "84028".into(),
            seller_city: "Landshut".into(),
            seller_tax_number: None,
            seller_vat_id: None,
            net_amount_cents: 10000,
            tax_amount_cents: 0,
            gross_amount_cents: 10000,
            currency_code: "EUR".into(),
            is_kleinunternehmer: 1,
            pdf_template: "default".into(),
            status: "paid".into(),
            sent_at: Some("2026-03-02 09:00:00".into()),
            paid_amount_cents: 10000,
            paid_at: Some("2026-03-10".into()),
            payment_history_json: None,
            canceled_at: None,
            canceled_by_storno_id: None,
            is_storno_for: None,
            cancel_reason: None,
            validation_status: Some("passed".into()),
            validation_report: None,
            validated_at: None,
            pdf_archive_id: Some("ap1".into()),
            xml_archive_id: Some("ax1".into()),
            locked_at: Some("2026-03-01 12:00:00".into()),
            notes: Some("INTERN: Sonderpreis".into()),
            payment_note: Some("Betrag dankend erhalten.".into()),
            created_at: "2026-03-01 11:00:00".into(),
            updated_at: "2026-03-10 11:00:00".into(),
            buyer_name: Some("Erika Mustermann".into()),
            buyer_street: Some("Hauptstr. 1".into()),
            buyer_postal_code: Some("84028".into()),
            buyer_city: Some("Landshut".into()),
            buyer_country_code: Some("DE".into()),
            buyer_vat_id: Some("DE123456789".into()),
            buyer_email: Some("erika@example.de".into()),
            derived_from_quote_id: None,
            paid_to_account_id: None,
        }
    }

    fn invoice_item() -> InvoiceItemRow {
        InvoiceItemRow {
            id: "it1".into(),
            invoice_id: "i1".into(),
            position: 1,
            description: "Vor-Ort-Service".into(),
            quantity: 2.0,
            unit_code: "HUR".into(),
            unit_price_cents: 5000,
            net_amount_cents: 10000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        }
    }

    fn expense() -> ExpenseRow {
        ExpenseRow {
            id: "e1".into(),
            expense_number: "KO-2026-0007".into(),
            fiscal_year: 2026,
            expense_date: "2026-04-01".into(),
            paid_date: Some("2026-04-02".into()),
            paid_from_account_id: None,
            vendor_contact_id: Some("c1".into()),
            vendor_name_snapshot: "Erika Mustermann".into(),
            vendor_invoice_number: Some("L-77".into()),
            category: "services".into(),
            description: "Zulieferung".into(),
            net_amount_cents: 5000,
            tax_amount_cents: 950,
            gross_amount_cents: 5950,
            currency_code: "EUR".into(),
            reverse_charge_13b: 0,
            receipt_archive_id: Some("ar1".into()),
            einvoice_validation_status: None,
            einvoice_validation_report: None,
            recurring_subscription_id: None,
            capitalized_as_asset_id: None,
            status: "recorded".into(),
            canceled_at: None,
            canceled_reason: None,
            locked_at: Some("2026-04-01 12:00:00".into()),
            notes: Some("INTERN: prüfen".into()),
            created_at: "2026-04-01 11:00:00".into(),
        }
    }

    fn email() -> EmailLogRow {
        EmailLogRow {
            id: "m1".into(),
            created_at: "2026-03-02 09:00:00".into(),
            account_id: None,
            account_label: Some("Postfach".into()),
            channel: "graph".into(),
            related_kind: "invoice".into(),
            related_id: Some("i1".into()),
            related_number: Some("RE-2026-0001".into()),
            from_email: "info@wildbach.de".into(),
            to_email: "erika@example.de".into(),
            subject: "Ihre Rechnung RE-2026-0001".into(),
            attachment_count: 2,
            status: "success".into(),
            provider_code: Some("202".into()),
            provider_message: None,
            request_id: None,
            error: None,
        }
    }

    fn raw_doc() -> RawDocument {
        RawDocument {
            kind: "Rechnung (PDF)".into(),
            related_label: Some("RE-2026-0001".into()),
            archive_id: "ap1".into(),
            file_name: "RE-2026-0001.pdf".into(),
            mime_type: "application/pdf".into(),
            size_bytes: 12345,
            sha256: "abc".into(),
            created_at: "2026-03-01 12:00:00".into(),
        }
    }

    fn controller() -> ControllerInfo {
        ControllerInfo {
            name: "Wildbach Computerhilfe".into(),
            address: "Weg 2, 84028 Landshut".into(),
            tax_number: Some("123/456/789".into()),
            vat_id: None,
            email: Some("info@wildbach.de".into()),
            phone: Some("0871 99999".into()),
        }
    }

    #[test]
    fn assemble_includes_all_sections() {
        let c = contact();
        let inv = invoice();
        let items = vec![invoice_item()];
        let invoices = vec![RawInvoice {
            invoice: &inv,
            items: &items,
        }];
        let exps = vec![expense()];
        let docs = vec![raw_doc()];
        let mails = vec![email()];
        let audit = vec![RawAudit {
            timestamp_utc: "2026-03-01 12:00:00".into(),
            action: "invoice.lock".into(),
            entity_type: Some("invoice".into()),
            entity_id: Some("i1".into()),
        }];
        let raw = RawData {
            contact: &c,
            invoices: &invoices,
            quotes: &[],
            expenses: &exps,
            documents: &docs,
            emails: &mails,
            audit: &audit,
            controller: controller(),
            generated_at: "2026-05-24 12:00:00".into(),
        };

        let report = assemble(&raw);
        assert_eq!(report.subject.name, "Erika Mustermann");
        assert_eq!(report.subject.vat_id.as_deref(), Some("DE123456789"));
        assert_eq!(report.invoices.len(), 1);
        assert_eq!(report.invoices[0].invoice_number, "RE-2026-0001");
        assert_eq!(report.invoices[0].items.len(), 1);
        assert_eq!(report.invoices[0].items[0].description, "Vor-Ort-Service");
        // Buyer-Snapshot übernommen.
        let buyer = report.invoices[0].buyer_snapshot.as_ref().unwrap();
        assert_eq!(buyer.city.as_deref(), Some("Landshut"));
        assert_eq!(report.expenses.len(), 1);
        assert_eq!(report.expenses[0].expense_number, "KO-2026-0007");
        assert_eq!(report.documents.len(), 1);
        assert!(!report.documents[0].bundled); // Shell setzt das erst beim Bündeln.
        assert_eq!(report.emails.len(), 1);
        assert_eq!(report.audit_events.len(), 1);
        // Art.-15(1)-Pflichtangaben vorhanden.
        assert!(!report.processing_info.purposes.is_empty());
        assert!(report.processing_info.retention.contains("147"));
    }

    #[test]
    fn assemble_omits_internal_notes() {
        const {
            assert!(
                !INCLUDE_INTERNAL_NOTES,
                "Block-18-Entscheidung: interne Notizen ausgelassen"
            );
        }
        let c = contact();
        let inv = invoice();
        let items = vec![invoice_item()];
        let invoices = vec![RawInvoice {
            invoice: &inv,
            items: &items,
        }];
        let exps = vec![expense()];
        let raw = RawData {
            contact: &c,
            invoices: &invoices,
            quotes: &[],
            expenses: &exps,
            documents: &[],
            emails: &[],
            audit: &[],
            controller: controller(),
            generated_at: "2026-05-24 12:00:00".into(),
        };
        let report = assemble(&raw);
        assert!(!report.notes_included);
        // Kein internes notes-Feld darf in der serialisierten Auskunft auftauchen.
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            !json.contains("INTERN"),
            "interne Notizen dürfen nicht in der Auskunft erscheinen: {json}"
        );
        // Der kundenseitige Bezahlt-Hinweis (PDF-Text) bleibt aber enthalten.
        assert!(json.contains("dankend erhalten"));
    }

    #[test]
    fn buyer_snapshot_empty_becomes_none() {
        let c = contact();
        let mut inv = invoice();
        inv.buyer_name = None;
        inv.buyer_street = None;
        inv.buyer_postal_code = None;
        inv.buyer_city = None;
        inv.buyer_country_code = None;
        inv.buyer_vat_id = None;
        inv.buyer_email = None;
        let invoices = vec![RawInvoice {
            invoice: &inv,
            items: &[],
        }];
        let raw = RawData {
            contact: &c,
            invoices: &invoices,
            quotes: &[],
            expenses: &[],
            documents: &[],
            emails: &[],
            audit: &[],
            controller: controller(),
            generated_at: "2026-05-24 12:00:00".into(),
        };
        let report = assemble(&raw);
        assert!(report.invoices[0].buyer_snapshot.is_none());
    }
}
