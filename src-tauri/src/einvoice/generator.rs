//! XRechnung-3.0-Generator im **UN/CEFACT CII**-Format (Functional Core).
//!
//! Erzeugt `rsm:CrossIndustryInvoice`-XML, valide gegen die KoSIT-
//! Konfiguration **xrechnung_3.0** (CII). CII statt UBL, weil das XML
//! als ZUGFeRD/Factur-X in ein PDF/A-3 eingebettet wird und Mustang
//! (`CustomXMLProvider`) ausschließlich CII akzeptiert. CII-XRechnung ist
//! genauso gültig wie UBL-XRechnung; KoSIT validiert beide.
//!
//! ## §19-Kleinunternehmer (`seller.is_kleinunternehmer == true`)
//! - **BT-22** `ram:IncludedNote/ram:Content` trägt den wortgleichen
//!   [`HINWEIS_TEXT`].
//! - **BT-151** `ram:CategoryCode` = 'E' (Exempt) auf jeder Line UND im
//!   Header-`ApplicableTradeTax`.
//! - **BT-120** `ram:ExemptionReason` = Hinweis-Text im Header-Tax.
//! - **BT-117** `ram:CalculatedAmount` = 0.00, **BT-119** Rate = 0.
//!
//! ## Storno (`input.is_storno_for.is_some()`)
//! - **BT-3** `ram:TypeCode` = 384 (Corrected invoice). Standard = 380.
//! - **BG-3** `ram:InvoiceReferencedDocument/ram:IssuerAssignedID` (Pflicht
//!   bei 384 via BR-DE-26).
//!
//! ## Pflicht-Elemente, die KoSIT/BR-DE erzwingt
//! - **BG-16** Payment Instructions (BR-DE-1): mind. ein
//!   `SpecifiedTradeSettlementPaymentMeans/TypeCode`.
//! - **BT-34** Seller electronic address (`URIUniversalCommunication`).
//! - **BT-10** BuyerReference (Pflicht-Parameter).
//!
//! ## CII-Element-Reihenfolge ist schema-streng
//! Die Reihenfolge folgt der KoSIT-Schematron-Testinstanz
//! `cii-br-de-26-test.xml`. Reihenfolge nicht umstellen ohne KoSIT-Re-Test.
//!
//! Keine I/O — Caller persistiert das XML separat.

use chrono::NaiveDate;

use crate::domain::invoice::{
    compute_totals, is_supported_currency, BuyerView, InvoiceInput, InvoiceItemInput, SellerView,
};
use crate::domain::kleinunternehmer::HINWEIS_TEXT;

#[derive(Debug, thiserror::Error)]
pub enum GenerationError {
    #[error("BuyerReference (BT-10) ist Pflicht in XRechnung 3.0; übergeben Sie 'N/A' für B2C")]
    BuyerReferenceEmpty,
    #[error("Currency code (BT-5) ist leer")]
    CurrencyEmpty,
    #[error("Currency code (BT-5) '{0}' nicht unterstützt — v1.0 ist single-currency EUR")]
    CurrencyUnsupported(String),
}

/// 2-Decimal-Beträge mit Punkt-Dezimaltrenner.
fn fmt_amount(cents: i64) -> String {
    let neg = cents < 0;
    let abs: u64 = cents.unsigned_abs();
    let euros = abs / 100;
    let rest = abs % 100;
    if neg {
        format!("-{}.{:02}", euros, rest)
    } else {
        format!("{}.{:02}", euros, rest)
    }
}

/// XML-Escape für Text-Content.
fn esc(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}

/// `quantity` mit 4 Nachkommastellen (CII erlaubt bis zu 4).
fn fmt_quantity(q: f64) -> String {
    format!("{:.4}", q)
}

/// `tax_rate_percent` ohne unnötige Nachkommas: "0" / "7" / "19".
fn fmt_rate(r: f64) -> String {
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else {
        format!("{:.2}", r)
    }
}

/// CII-Datumsformat `format="102"` = `YYYYMMDD`.
fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y%m%d").to_string()
}

/// Erzeugt XRechnung-3.0-CII-XML (`rsm:CrossIndustryInvoice`).
///
/// `buyer_reference` ist Pflicht (BT-10). Caller setzt die Leitweg-ID
/// (B2G) oder den Default `"N/A"` (B2C/B2B).
///
/// `bank_accounts` ist die Liste der auf der Rechnung anzuzeigenden Bank-Konten
/// als `(IBAN, BIC?)`. Jedes wird zu einer SEPA-Überweisung (BT-81 TypeCode 58 +
/// BT-84 IBAN + optional BT-86 BIC). CII erlaubt mehrere `PaymentMeans`. Nicht-
/// Bank-Konten (PayPal etc.) erscheinen nur im PDF, nicht im XML.
pub fn to_xrechnung(
    invoice_number: &str,
    input: &InvoiceInput,
    seller: &SellerView<'_>,
    buyer: &BuyerView<'_>,
    buyer_reference: &str,
    bank_accounts: &[(&str, Option<&str>)],
) -> Result<String, GenerationError> {
    if buyer_reference.trim().is_empty() {
        return Err(GenerationError::BuyerReferenceEmpty);
    }
    if input.currency_code.trim().is_empty() {
        return Err(GenerationError::CurrencyEmpty);
    }

    let totals = compute_totals(&input.items);
    let is_klein = seller.is_kleinunternehmer;
    let is_storno = input.is_storno_for.is_some();
    // R3-002: BT-5 ist case-sensitive ISO-4217 (Großbuchstaben). Trimmen und
    // upper-casen, gegen Domain-Whitelist prüfen — der Generator ist die letzte
    // Bastion vor dem write-once-Archiv, defensive Härtung gegen Caller-Drift
    // (E-Rechnung-Empfang-Roundtrip, künftige Multi-Currency-Pfade).
    let currency = input.currency_code.trim().to_uppercase();
    if !is_supported_currency(&currency) {
        return Err(GenerationError::CurrencyUnsupported(currency));
    }
    let invoice_type_code = if is_storno { "384" } else { "380" };

    let mut x = String::with_capacity(8192);
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<rsm:CrossIndustryInvoice");
    x.push_str(" xmlns:rsm=\"urn:un:unece:uncefact:data:standard:CrossIndustryInvoice:100\"");
    x.push_str(" xmlns:ram=\"urn:un:unece:uncefact:data:standard:ReusableAggregateBusinessInformationEntity:100\"");
    x.push_str(" xmlns:udt=\"urn:un:unece:uncefact:data:standard:UnqualifiedDataType:100\">\n");

    // === ExchangedDocumentContext ===
    x.push_str("  <rsm:ExchangedDocumentContext>\n");
    x.push_str("    <ram:BusinessProcessSpecifiedDocumentContextParameter>\n");
    x.push_str("      <ram:ID>urn:fdc:peppol.eu:2017:poacc:billing:01:1.0</ram:ID>\n");
    x.push_str("    </ram:BusinessProcessSpecifiedDocumentContextParameter>\n");
    x.push_str("    <ram:GuidelineSpecifiedDocumentContextParameter>\n");
    x.push_str("      <ram:ID>urn:cen.eu:en16931:2017#compliant#urn:xeinkauf.de:kosit:xrechnung_3.0</ram:ID>\n");
    x.push_str("    </ram:GuidelineSpecifiedDocumentContextParameter>\n");
    x.push_str("  </rsm:ExchangedDocumentContext>\n");

    // === ExchangedDocument ===
    x.push_str("  <rsm:ExchangedDocument>\n");
    x.push_str(&format!("    <ram:ID>{}</ram:ID>\n", esc(invoice_number)));
    x.push_str(&format!(
        "    <ram:TypeCode>{}</ram:TypeCode>\n",
        invoice_type_code
    ));
    x.push_str("    <ram:IssueDateTime>\n");
    x.push_str(&format!(
        "      <udt:DateTimeString format=\"102\">{}</udt:DateTimeString>\n",
        fmt_date(input.invoice_date)
    ));
    x.push_str("    </ram:IssueDateTime>\n");
    // BT-22 §19-Klausel. EN-16931 verlangt SubjectCode (UNTDID 4451) für jede
    // IncludedNote — `REG` (Regulatory information) ist der EN-16931-konforme
    // Code für regulatorische Hinweise wie die Kleinunternehmer-Klausel.
    // Fix R3-001 (v2026.5-Re-Review).
    if is_klein {
        x.push_str("    <ram:IncludedNote>\n");
        x.push_str("      <ram:SubjectCode>REG</ram:SubjectCode>\n");
        x.push_str(&format!(
            "      <ram:Content>{}</ram:Content>\n",
            esc(HINWEIS_TEXT)
        ));
        x.push_str("    </ram:IncludedNote>\n");
    }
    // BT-22 Custom-Notes. Dedup R3-003 — wenn der Nutzer in `notes` den
    // §19-Klausel-Text dupliziert, geben wir die Note nur einmal aus.
    if let Some(notes) = &input.notes {
        let t = notes.trim();
        if !t.is_empty() && (!is_klein || t != HINWEIS_TEXT) {
            x.push_str("    <ram:IncludedNote>\n");
            x.push_str(&format!("      <ram:Content>{}</ram:Content>\n", esc(t)));
            x.push_str("    </ram:IncludedNote>\n");
        }
    }
    x.push_str("  </rsm:ExchangedDocument>\n");

    // === SupplyChainTradeTransaction ===
    x.push_str("  <rsm:SupplyChainTradeTransaction>\n");

    // --- Lines ---
    for (it, t) in input.items.iter().zip(totals.items.iter()) {
        x.push_str("    <ram:IncludedSupplyChainTradeLineItem>\n");
        x.push_str("      <ram:AssociatedDocumentLineDocument>\n");
        x.push_str(&format!(
            "        <ram:LineID>{}</ram:LineID>\n",
            it.position
        ));
        x.push_str("      </ram:AssociatedDocumentLineDocument>\n");
        x.push_str("      <ram:SpecifiedTradeProduct>\n");
        x.push_str(&format!(
            "        <ram:Name>{}</ram:Name>\n",
            esc(&it.description)
        ));
        x.push_str("      </ram:SpecifiedTradeProduct>\n");
        x.push_str("      <ram:SpecifiedLineTradeAgreement>\n");
        x.push_str("        <ram:NetPriceProductTradePrice>\n");
        x.push_str(&format!(
            "          <ram:ChargeAmount>{}</ram:ChargeAmount>\n",
            fmt_amount(it.unit_price_cents)
        ));
        x.push_str("        </ram:NetPriceProductTradePrice>\n");
        x.push_str("      </ram:SpecifiedLineTradeAgreement>\n");
        x.push_str("      <ram:SpecifiedLineTradeDelivery>\n");
        x.push_str(&format!(
            "        <ram:BilledQuantity unitCode=\"{}\">{}</ram:BilledQuantity>\n",
            esc(&it.unit_code),
            fmt_quantity(it.quantity)
        ));
        x.push_str("      </ram:SpecifiedLineTradeDelivery>\n");
        x.push_str("      <ram:SpecifiedLineTradeSettlement>\n");
        x.push_str("        <ram:ApplicableTradeTax>\n");
        x.push_str("          <ram:TypeCode>VAT</ram:TypeCode>\n");
        x.push_str(&format!(
            "          <ram:CategoryCode>{}</ram:CategoryCode>\n",
            esc(&it.tax_category_code)
        ));
        x.push_str(&format!(
            "          <ram:RateApplicablePercent>{}</ram:RateApplicablePercent>\n",
            fmt_rate(it.tax_rate_percent)
        ));
        x.push_str("        </ram:ApplicableTradeTax>\n");
        x.push_str("        <ram:SpecifiedTradeSettlementLineMonetarySummation>\n");
        x.push_str(&format!(
            "          <ram:LineTotalAmount>{}</ram:LineTotalAmount>\n",
            fmt_amount(t.net_amount_cents)
        ));
        x.push_str("        </ram:SpecifiedTradeSettlementLineMonetarySummation>\n");
        x.push_str("      </ram:SpecifiedLineTradeSettlement>\n");
        x.push_str("    </ram:IncludedSupplyChainTradeLineItem>\n");
    }

    // --- ApplicableHeaderTradeAgreement ---
    x.push_str("    <ram:ApplicableHeaderTradeAgreement>\n");
    x.push_str(&format!(
        "      <ram:BuyerReference>{}</ram:BuyerReference>\n",
        esc(buyer_reference.trim())
    ));
    // Seller
    x.push_str("      <ram:SellerTradeParty>\n");
    x.push_str(&format!(
        "        <ram:Name>{}</ram:Name>\n",
        esc(seller.name)
    ));
    x.push_str("        <ram:PostalTradeAddress>\n");
    x.push_str(&format!(
        "          <ram:PostcodeCode>{}</ram:PostcodeCode>\n",
        esc(seller.postal_code)
    ));
    x.push_str(&format!(
        "          <ram:LineOne>{}</ram:LineOne>\n",
        esc(seller.street)
    ));
    x.push_str(&format!(
        "          <ram:CityName>{}</ram:CityName>\n",
        esc(seller.city)
    ));
    x.push_str(&format!(
        "          <ram:CountryID>{}</ram:CountryID>\n",
        esc(seller.country_code)
    ));
    x.push_str("        </ram:PostalTradeAddress>\n");
    // BT-34 Seller electronic address (Pflicht)
    x.push_str("        <ram:URIUniversalCommunication>\n");
    x.push_str(&format!(
        "          <ram:URIID schemeID=\"EM\">{}</ram:URIID>\n",
        esc(seller.email)
    ));
    x.push_str("        </ram:URIUniversalCommunication>\n");
    // BT-31 USt-IdNr. (VA) zuerst, dann BT-32 Steuernr. (FC)
    if let Some(vat) = seller.vat_id.filter(|s| !s.trim().is_empty()) {
        x.push_str("        <ram:SpecifiedTaxRegistration>\n");
        x.push_str(&format!(
            "          <ram:ID schemeID=\"VA\">{}</ram:ID>\n",
            esc(vat)
        ));
        x.push_str("        </ram:SpecifiedTaxRegistration>\n");
    }
    if let Some(tax) = seller.tax_number.filter(|s| !s.trim().is_empty()) {
        x.push_str("        <ram:SpecifiedTaxRegistration>\n");
        x.push_str(&format!(
            "          <ram:ID schemeID=\"FC\">{}</ram:ID>\n",
            esc(tax)
        ));
        x.push_str("        </ram:SpecifiedTaxRegistration>\n");
    }
    x.push_str("      </ram:SellerTradeParty>\n");
    // Buyer
    x.push_str("      <ram:BuyerTradeParty>\n");
    x.push_str(&format!(
        "        <ram:Name>{}</ram:Name>\n",
        esc(buyer.name)
    ));
    x.push_str("        <ram:PostalTradeAddress>\n");
    if let Some(z) = buyer.postal_code.filter(|s| !s.trim().is_empty()) {
        x.push_str(&format!(
            "          <ram:PostcodeCode>{}</ram:PostcodeCode>\n",
            esc(z)
        ));
    }
    if let Some(s) = buyer.street.filter(|s| !s.trim().is_empty()) {
        x.push_str(&format!(
            "          <ram:LineOne>{}</ram:LineOne>\n",
            esc(s)
        ));
    }
    if let Some(c) = buyer.city.filter(|s| !s.trim().is_empty()) {
        x.push_str(&format!(
            "          <ram:CityName>{}</ram:CityName>\n",
            esc(c)
        ));
    }
    x.push_str(&format!(
        "          <ram:CountryID>{}</ram:CountryID>\n",
        esc(buyer.country_code)
    ));
    x.push_str("        </ram:PostalTradeAddress>\n");
    // BT-49 Buyer electronic address (falls vorhanden)
    if let Some(mail) = buyer.email.filter(|s| !s.trim().is_empty()) {
        x.push_str("        <ram:URIUniversalCommunication>\n");
        x.push_str(&format!(
            "          <ram:URIID schemeID=\"EM\">{}</ram:URIID>\n",
            esc(mail)
        ));
        x.push_str("        </ram:URIUniversalCommunication>\n");
    }
    // BT-48 Buyer USt-IdNr. (falls vorhanden)
    if let Some(vat) = buyer.vat_id.filter(|s| !s.trim().is_empty()) {
        x.push_str("        <ram:SpecifiedTaxRegistration>\n");
        x.push_str(&format!(
            "          <ram:ID schemeID=\"VA\">{}</ram:ID>\n",
            esc(vat)
        ));
        x.push_str("        </ram:SpecifiedTaxRegistration>\n");
    }
    x.push_str("      </ram:BuyerTradeParty>\n");
    x.push_str("    </ram:ApplicableHeaderTradeAgreement>\n");

    // --- ApplicableHeaderTradeDelivery ---
    x.push_str("    <ram:ApplicableHeaderTradeDelivery>\n");
    if let Some(deliv) = input.delivery_date {
        x.push_str("      <ram:ActualDeliverySupplyChainEvent>\n");
        x.push_str("        <ram:OccurrenceDateTime>\n");
        x.push_str(&format!(
            "          <udt:DateTimeString format=\"102\">{}</udt:DateTimeString>\n",
            fmt_date(deliv)
        ));
        x.push_str("        </ram:OccurrenceDateTime>\n");
        x.push_str("      </ram:ActualDeliverySupplyChainEvent>\n");
    }
    x.push_str("    </ram:ApplicableHeaderTradeDelivery>\n");

    // --- ApplicableHeaderTradeSettlement ---
    x.push_str("    <ram:ApplicableHeaderTradeSettlement>\n");
    // BT-83 Verwendungszweck = Rechnungsnummer. In der CII-Reihenfolge MUSS
    // PaymentReference VOR InvoiceCurrencyCode (BT-5) stehen.
    x.push_str(&format!(
        "      <ram:PaymentReference>{}</ram:PaymentReference>\n",
        esc(invoice_number)
    ));
    x.push_str(&format!(
        "      <ram:InvoiceCurrencyCode>{}</ram:InvoiceCurrencyCode>\n",
        esc(&currency)
    ));
    // BG-16 Payment Instructions (BR-DE-1 Pflicht). Jedes geflaggte Bank-Konto
    // wird eine SEPA-Überweisung (BT-81 TypeCode 58) + Creditor-IBAN (BT-84) +
    // optional BIC (BT-86). CII erlaubt mehrere SpecifiedTradeSettlementPaymentMeans.
    // EN16931 BR-61: bei Überweisung ist BT-84 Pflicht → TypeCode 58 nur mit IBAN.
    // Bei Storno (Gutschrift) werden keine Überweisungsdaten ausgewiesen.
    // Fallback (kein Bank-Konto oder Storno): TypeCode 1 (nicht spezifiziert),
    // damit BR-DE-1 (mind. ein PaymentMeans/TypeCode) erfüllt bleibt.
    let emitted_any = if is_storno {
        false
    } else {
        let mut any = false;
        for (iban_raw, bic_opt) in bank_accounts {
            let iban: String = iban_raw.chars().filter(|c| !c.is_whitespace()).collect();
            if iban.is_empty() {
                continue;
            }
            any = true;
            x.push_str("      <ram:SpecifiedTradeSettlementPaymentMeans>\n");
            x.push_str("        <ram:TypeCode>58</ram:TypeCode>\n");
            x.push_str("        <ram:PayeePartyCreditorFinancialAccount>\n");
            x.push_str(&format!(
                "          <ram:IBANID>{}</ram:IBANID>\n",
                esc(&iban)
            ));
            x.push_str(&format!(
                "          <ram:AccountName>{}</ram:AccountName>\n",
                esc(seller.name)
            ));
            x.push_str("        </ram:PayeePartyCreditorFinancialAccount>\n");
            if let Some(bic) = bic_opt.map(|s| s.trim()).filter(|s| !s.is_empty()) {
                x.push_str("        <ram:PayeeSpecifiedCreditorFinancialInstitution>\n");
                x.push_str(&format!("          <ram:BICID>{}</ram:BICID>\n", esc(bic)));
                x.push_str("        </ram:PayeeSpecifiedCreditorFinancialInstitution>\n");
            }
            x.push_str("      </ram:SpecifiedTradeSettlementPaymentMeans>\n");
        }
        any
    };
    if !emitted_any {
        x.push_str("      <ram:SpecifiedTradeSettlementPaymentMeans>\n");
        x.push_str("        <ram:TypeCode>1</ram:TypeCode>\n");
        x.push_str("      </ram:SpecifiedTradeSettlementPaymentMeans>\n");
    }
    // Header-Tax pro (Kategorie, Rate)-Gruppe.
    let groups = group_tax(&input.items, &totals.items);
    for g in &groups {
        x.push_str("      <ram:ApplicableTradeTax>\n");
        x.push_str(&format!(
            "        <ram:CalculatedAmount>{}</ram:CalculatedAmount>\n",
            fmt_amount(g.tax_amount_cents)
        ));
        x.push_str("        <ram:TypeCode>VAT</ram:TypeCode>\n");
        if g.category_code == "E" {
            x.push_str(&format!(
                "        <ram:ExemptionReason>{}</ram:ExemptionReason>\n",
                esc(HINWEIS_TEXT)
            ));
        }
        x.push_str(&format!(
            "        <ram:BasisAmount>{}</ram:BasisAmount>\n",
            fmt_amount(g.taxable_amount_cents)
        ));
        x.push_str(&format!(
            "        <ram:CategoryCode>{}</ram:CategoryCode>\n",
            esc(&g.category_code)
        ));
        x.push_str(&format!(
            "        <ram:RateApplicablePercent>{}</ram:RateApplicablePercent>\n",
            fmt_rate(g.rate)
        ));
        x.push_str("      </ram:ApplicableTradeTax>\n");
    }
    // Fälligkeit (BT-9) als SpecifiedTradePaymentTerms.
    if let Some(due) = input.due_date {
        x.push_str("      <ram:SpecifiedTradePaymentTerms>\n");
        x.push_str("        <ram:DueDateDateTime>\n");
        x.push_str(&format!(
            "          <udt:DateTimeString format=\"102\">{}</udt:DateTimeString>\n",
            fmt_date(due)
        ));
        x.push_str("        </ram:DueDateDateTime>\n");
        x.push_str("      </ram:SpecifiedTradePaymentTerms>\n");
    }
    // BG-22 Monetary Summation.
    x.push_str("      <ram:SpecifiedTradeSettlementHeaderMonetarySummation>\n");
    x.push_str(&format!(
        "        <ram:LineTotalAmount>{}</ram:LineTotalAmount>\n",
        fmt_amount(totals.net_amount_cents)
    ));
    x.push_str(&format!(
        "        <ram:TaxBasisTotalAmount>{}</ram:TaxBasisTotalAmount>\n",
        fmt_amount(totals.net_amount_cents)
    ));
    x.push_str(&format!(
        "        <ram:TaxTotalAmount currencyID=\"{}\">{}</ram:TaxTotalAmount>\n",
        esc(&currency),
        fmt_amount(totals.tax_amount_cents)
    ));
    x.push_str(&format!(
        "        <ram:GrandTotalAmount>{}</ram:GrandTotalAmount>\n",
        fmt_amount(totals.gross_amount_cents)
    ));
    x.push_str(&format!(
        "        <ram:DuePayableAmount>{}</ram:DuePayableAmount>\n",
        fmt_amount(totals.gross_amount_cents)
    ));
    x.push_str("      </ram:SpecifiedTradeSettlementHeaderMonetarySummation>\n");
    // BG-3 Preceding Invoice Reference (Storno, BR-DE-26 Pflicht bei 384).
    if is_storno {
        if let Some(ref_id) = &input.is_storno_for {
            x.push_str("      <ram:InvoiceReferencedDocument>\n");
            x.push_str(&format!(
                "        <ram:IssuerAssignedID>{}</ram:IssuerAssignedID>\n",
                esc(ref_id)
            ));
            x.push_str("      </ram:InvoiceReferencedDocument>\n");
        }
    }
    x.push_str("    </ram:ApplicableHeaderTradeSettlement>\n");

    x.push_str("  </rsm:SupplyChainTradeTransaction>\n");
    x.push_str("</rsm:CrossIndustryInvoice>\n");
    Ok(x)
}

/// Pro (Tax-Category, Rate) gruppiert.
#[derive(Debug, Clone)]
struct TaxGroup {
    category_code: String,
    rate: f64,
    taxable_amount_cents: i64,
    tax_amount_cents: i64,
}

fn group_tax(
    inputs: &[InvoiceItemInput],
    totals: &[crate::domain::invoice::ItemTotals],
) -> Vec<TaxGroup> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, TaxGroup> = BTreeMap::new();
    for (it, t) in inputs.iter().zip(totals.iter()) {
        let key = format!("{}|{:.4}", it.tax_category_code, it.tax_rate_percent);
        groups
            .entry(key)
            .and_modify(|g| {
                g.taxable_amount_cents += t.net_amount_cents;
                g.tax_amount_cents += t.tax_amount_cents;
            })
            .or_insert(TaxGroup {
                category_code: it.tax_category_code.clone(),
                rate: it.tax_rate_percent,
                taxable_amount_cents: t.net_amount_cents,
                tax_amount_cents: t.tax_amount_cents,
            });
    }
    groups.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::invoice::{InvoiceDirection, InvoiceInput, InvoiceItemInput};
    use chrono::NaiveDate;

    fn seller_klein() -> SellerView<'static> {
        SellerView {
            name: "Wildbach Computerhilfe",
            street: "Beispielweg 1",
            postal_code: "84028",
            city: "Landshut",
            country_code: "DE",
            tax_number: Some("123/456/78901"),
            vat_id: None,
            email: "schmidm@wildbach-computerhilfe.de",
            iban: None,
            bic: None,
            is_kleinunternehmer: true,
            waived_since: None,
        }
    }

    fn buyer() -> BuyerView<'static> {
        BuyerView {
            name: "Kunde GmbH",
            street: Some("Hauptstr. 7"),
            postal_code: Some("80331"),
            city: Some("München"),
            country_code: "DE",
            vat_id: Some("DE111111111"),
            email: Some("info@kunde.de"),
        }
    }

    fn invoice_simple() -> InvoiceInput {
        InvoiceInput {
            direction: InvoiceDirection::Issued,
            invoice_date: NaiveDate::from_ymd_opt(2026, 5, 19).unwrap(),
            delivery_date: Some(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap()),
            due_date: Some(NaiveDate::from_ymd_opt(2026, 6, 18).unwrap()),
            currency_code: "EUR".into(),
            items: vec![InvoiceItemInput {
                position: 1,
                description: "Beratung & Support".into(),
                quantity: 2.0,
                unit_code: "HUR".into(),
                unit_price_cents: 10_000,
                tax_rate_percent: 0.0,
                tax_category_code: "E".into(),
                description_title: None,
                description_markup: None,
                source_package_id: None,
                source_package_revision: None,
            }],
            notes: None,
            payment_note: None,
            pdf_template: "default".into(),
            is_storno_for: None,
            cancel_reason: None,
        }
    }

    #[test]
    fn fmt_amount_basics() {
        assert_eq!(fmt_amount(0), "0.00");
        assert_eq!(fmt_amount(1), "0.01");
        assert_eq!(fmt_amount(100), "1.00");
        assert_eq!(fmt_amount(12345), "123.45");
        assert_eq!(fmt_amount(-12345), "-123.45");
    }

    #[test]
    fn fmt_rate_drops_trailing_zeros() {
        assert_eq!(fmt_rate(0.0), "0");
        assert_eq!(fmt_rate(19.0), "19");
        assert_eq!(fmt_rate(7.0), "7");
        assert_eq!(fmt_rate(7.5), "7.50");
    }

    #[test]
    fn fmt_date_is_yyyymmdd() {
        assert_eq!(
            fmt_date(NaiveDate::from_ymd_opt(2026, 5, 19).unwrap()),
            "20260519"
        );
        assert_eq!(
            fmt_date(NaiveDate::from_ymd_opt(2026, 1, 3).unwrap()),
            "20260103"
        );
    }

    #[test]
    fn buyer_reference_empty_errors() {
        let err = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "  ",
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, GenerationError::BuyerReferenceEmpty));
    }

    #[test]
    fn produces_cii_root_not_ubl() {
        let xml = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        assert!(xml.contains("<rsm:CrossIndustryInvoice"));
        assert!(!xml.contains("<ubl:Invoice"));
    }

    #[test]
    fn delivery_date_controls_bt72_event() {
        // Kontrakt, auf den der Leistungsdatum-Fallback (commands::invoices) baut:
        // Mit Leistungsdatum → ActualDeliverySupplyChainEvent + Datum (format 102).
        let with = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        assert!(with.contains("<ram:ActualDeliverySupplyChainEvent>"));
        assert!(with.contains("<udt:DateTimeString format=\"102\">20260519</udt:DateTimeString>"));
        // Ohne Leistungsdatum → kein Event. (Bei aktivem Fallback setzt der Command
        // delivery_date = Rechnungsdatum, sodass BT-72 dann doch gefüllt wird.)
        let mut no_deliv = invoice_simple();
        no_deliv.delivery_date = None;
        let without = to_xrechnung(
            "RE-2026-0001",
            &no_deliv,
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        assert!(!without.contains("<ram:ActualDeliverySupplyChainEvent>"));
    }

    #[test]
    fn klein_xml_contains_19_clause_and_exempt_category() {
        let xml = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        // BT-22 Note (CII)
        assert!(xml.contains(
            "<ram:Content>Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.</ram:Content>"
        ));
        // BT-151 Category E (Line + Header)
        assert!(xml.contains("<ram:CategoryCode>E</ram:CategoryCode>"));
        // BT-117 CalculatedAmount = 0.00
        assert!(xml.contains("<ram:CalculatedAmount>0.00</ram:CalculatedAmount>"));
        // BT-120 ExemptionReason mit §19-Text
        assert!(xml.contains(
            "<ram:ExemptionReason>Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.</ram:ExemptionReason>"
        ));
        // Customization XRechnung 3.0
        assert!(xml.contains("xrechnung_3.0"));
        // BT-3 Type 380
        assert!(xml.contains("<ram:TypeCode>380</ram:TypeCode>"));
        // BT-1 Nummer
        assert!(xml.contains("<ram:ID>RE-2026-0001</ram:ID>"));
        // BT-34 Seller electronic address
        assert!(xml
            .contains("<ram:URIID schemeID=\"EM\">schmidm@wildbach-computerhilfe.de</ram:URIID>"));
        // BG-16 Payment Instructions
        assert!(xml.contains("<ram:SpecifiedTradeSettlementPaymentMeans>"));
    }

    #[test]
    fn storno_uses_type_code_384_and_invoice_referenced_document() {
        let mut inv = invoice_simple();
        inv.is_storno_for = Some("RE-2026-0001".into());
        inv.items[0].unit_price_cents = -10_000;
        let xml =
            to_xrechnung("ST-2026-0001", &inv, &seller_klein(), &buyer(), "N/A", &[]).unwrap();
        assert!(xml.contains("<ram:TypeCode>384</ram:TypeCode>"));
        assert!(xml.contains("<ram:InvoiceReferencedDocument>"));
        assert!(xml.contains("<ram:IssuerAssignedID>RE-2026-0001</ram:IssuerAssignedID>"));
    }

    #[test]
    fn monetary_summation_matches_totals() {
        let mut inv = invoice_simple();
        inv.items.push(InvoiceItemInput {
            position: 2,
            description: "Reisekosten".into(),
            quantity: 1.0,
            unit_code: "C62".into(),
            unit_price_cents: 5_000,
            tax_rate_percent: 0.0,
            tax_category_code: "E".into(),
            description_title: None,
            description_markup: None,
            source_package_id: None,
            source_package_revision: None,
        });
        let xml =
            to_xrechnung("RE-2026-0001", &inv, &seller_klein(), &buyer(), "N/A", &[]).unwrap();
        // 200,00 + 50,00 = 250,00 €
        assert!(xml.contains("<ram:GrandTotalAmount>250.00</ram:GrandTotalAmount>"));
        assert!(xml.contains("<ram:DuePayableAmount>250.00</ram:DuePayableAmount>"));
        assert!(xml.contains("<ram:LineTotalAmount>250.00</ram:LineTotalAmount>"));
        // genau eine Header-ApplicableTradeTax-Gruppe (E/0)
        assert_eq!(
            xml.matches("<ram:BasisAmount>250.00</ram:BasisAmount>")
                .count(),
            1
        );
    }

    #[test]
    fn xml_is_well_formed_parses_back() {
        let xml = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        let mut reader = quick_xml::Reader::from_str(&xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut depth: i32 = 0;
        let mut events: u32 = 0;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(_)) => {
                    depth += 1;
                    events += 1;
                }
                Ok(quick_xml::events::Event::End(_)) => {
                    depth -= 1;
                    events += 1;
                }
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => events += 1,
                Err(e) => panic!("xml malformed: {e}"),
            }
            buf.clear();
        }
        assert_eq!(depth, 0, "tags should balance");
        assert!(events > 40, "should have many events");
    }

    #[test]
    fn escaping_handles_ampersand_and_lt_in_text() {
        let mut inv = invoice_simple();
        inv.items[0].description = "Tom & Jerry <test>".into();
        let xml =
            to_xrechnung("RE-2026-0001", &inv, &seller_klein(), &buyer(), "N/A", &[]).unwrap();
        assert!(xml.contains("Tom &amp; Jerry &lt;test&gt;"));
        assert!(!xml.contains("<test>"));
    }

    fn seller_with_iban() -> SellerView<'static> {
        SellerView {
            iban: Some("DE02 1203 0000 0000 2020 51"),
            bic: Some("BYLADEM1001"),
            ..seller_klein()
        }
    }

    #[test]
    fn iban_emits_sepa_credit_transfer_and_payment_reference() {
        let xml = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_with_iban(),
            &buyer(),
            "N/A",
            &[("DE02 1203 0000 0000 2020 51", Some("BYLADEM1001"))],
        )
        .unwrap();
        // BT-83 Verwendungszweck = Rechnungsnummer
        assert!(xml.contains("<ram:PaymentReference>RE-2026-0001</ram:PaymentReference>"));
        // BT-81 SEPA-Überweisung
        assert!(xml.contains("<ram:TypeCode>58</ram:TypeCode>"));
        // BT-84 IBAN — Leerzeichen entfernt
        assert!(xml.contains("<ram:IBANID>DE02120300000000202051</ram:IBANID>"));
        // BT-85 Kontoinhaber + BT-86 BIC
        assert!(xml.contains("<ram:AccountName>Wildbach Computerhilfe</ram:AccountName>"));
        assert!(xml.contains("<ram:BICID>BYLADEM1001</ram:BICID>"));
        // Reihenfolge: PaymentReference vor InvoiceCurrencyCode (CII-Schema)
        let pr = xml.find("PaymentReference").expect("PaymentReference");
        let cc = xml
            .find("InvoiceCurrencyCode")
            .expect("InvoiceCurrencyCode");
        assert!(
            pr < cc,
            "PaymentReference muss vor InvoiceCurrencyCode stehen"
        );
    }

    #[test]
    fn multiple_bank_accounts_emit_multiple_payment_means() {
        let xml = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[
                ("DE02 1203 0000 0000 2020 51", Some("BYLADEM1001")),
                ("DE89 3704 0044 0532 0130 00", None),
            ],
        )
        .unwrap();
        // Zwei SEPA-Überweisungen (BT-81 TypeCode 58)
        assert_eq!(xml.matches("<ram:TypeCode>58</ram:TypeCode>").count(), 2);
        // Beide IBANs ohne Leerzeichen
        assert!(xml.contains("<ram:IBANID>DE02120300000000202051</ram:IBANID>"));
        assert!(xml.contains("<ram:IBANID>DE89370400440532013000</ram:IBANID>"));
        // Erstes Konto mit BIC, zweites ohne
        assert_eq!(xml.matches("<ram:BICID>").count(), 1);
        // Kein Fallback-TypeCode 1, weil mind. ein Bank-Konto vorhanden
        assert!(!xml.contains("<ram:TypeCode>1</ram:TypeCode>"));
    }

    #[test]
    fn empty_bank_accounts_fall_back_to_type_code_1() {
        let xml = to_xrechnung(
            "RE-2026-0001",
            &invoice_simple(),
            &seller_klein(),
            &buyer(),
            "N/A",
            &[],
        )
        .unwrap();
        // BR-DE-1: ohne Bank-Konto Fallback auf TypeCode 1
        assert!(xml.contains("<ram:TypeCode>1</ram:TypeCode>"));
        assert!(!xml.contains("<ram:IBANID>"));
    }

    #[test]
    fn storno_keeps_typecode_1_without_iban() {
        let mut inv = invoice_simple();
        inv.is_storno_for = Some("RE-2026-0001".into());
        inv.items[0].unit_price_cents = -10_000;
        let xml = to_xrechnung(
            "ST-2026-0001",
            &inv,
            &seller_with_iban(),
            &buyer(),
            "N/A",
            &[("DE02 1203 0000 0000 2020 51", Some("BYLADEM1001"))],
        )
        .unwrap();
        // Gutschrift: keine Überweisungsdaten, auch wenn ein Bank-Konto geflaggt ist
        assert!(xml.contains("<ram:TypeCode>1</ram:TypeCode>"));
        assert!(!xml.contains("<ram:IBANID>"));
    }
}
