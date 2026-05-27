//! PDF-Schicht: Typst-Render + §19-Klausel-Check.
//!
//! `klausel_check` ist Functional Core und prüft VOR jedem Render, dass
//! ein PDF-Template den `// §19-KLAUSEL-BLOCK: REQUIRED`-Marker enthält
//! und ihn nutzt, wenn `is_kleinunternehmer = true`. Sonst Render-Abort.

pub mod bundle;
pub mod klausel_check;
pub mod templates;
pub mod typst_render;

#[cfg(test)]
mod tests {
    #[test]
    fn it_compiles() {}
}
