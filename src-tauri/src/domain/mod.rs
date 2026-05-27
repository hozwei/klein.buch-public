//! Domain-Schicht — **Functional Core**. Pure Rechnen/Validieren, keine I/O.

pub mod anonymize;
pub mod asset;
pub mod contact;
pub mod depreciation;
pub mod drop_folder;
pub mod dsgvo;
pub mod expense;
pub mod factory_reset;
pub mod fiscal_year;
pub mod invoice;
pub mod kleinbetragsrechnung;
pub mod kleinunternehmer;
pub mod numbering;
pub mod package;
pub mod private_movement;
pub mod quote;
pub mod recurring;
pub mod recurring_invoice;
pub mod storno;
pub mod travel;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
