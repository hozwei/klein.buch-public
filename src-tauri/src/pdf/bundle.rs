//! Angebots-Bundle: mehrere PDFs zu EINEM Druck-PDF zusammenführen (Block 8).
//!
//! Schicht: **Functional Core** (in-memory, kein I/O). Führt das Angebots-PDF
//! und die archivierten AGB-/Datenschutz-PDFs zu einer Datei zusammen — für
//! „Drucken/Ansehen" als ein Dokument. Die Mail-Variante hängt die drei PDFs
//! dagegen als separate Anhänge an (Block 5 Multi-Attachment).
//!
//! Der zusammengeführte Bundle ist ein **abgeleitetes Convenience-Artefakt**
//! und wird NICHT archiviert: kanonisch + write-once sind das Angebots-PDF und
//! jede Legal-Version einzeln. Der rechtliche „welche Version ging raus"-Nachweis
//! liegt in `quote_legal_documents` (append-only), nicht im Merge.
//!
//! Implementierung mit `lopdf` (reines Rust). Das Merge-Verfahren (Objekte
//! renummerieren, Page-Tree zusammenführen, einen Catalog/Pages-Root bauen) ist
//! gegen das offizielle `lopdf`-Beispiel `examples/merge.rs` (v0.40)
//! verifiziert; Bookmarks/Outlines lassen wir weg (für ein Angebots-Bundle
//! nicht nötig).

use std::collections::BTreeMap;

use lopdf::{Document, Object, ObjectId};

use crate::error::{Error, Result};

/// Führt mehrere PDF-Dokumente (als Bytes) zu einem zusammen. Leere Teile
/// werden übersprungen. Bei genau einem (nicht-leeren) Teil werden dessen
/// Bytes unverändert durchgereicht (kein unnötiges Re-Encoding).
pub fn merge_pdfs(parts: &[Vec<u8>]) -> Result<Vec<u8>> {
    let inputs: Vec<&Vec<u8>> = parts.iter().filter(|p| !p.is_empty()).collect();
    if inputs.is_empty() {
        return Err(Error::Domain(
            "merge_pdfs: keine PDF-Teile übergeben".into(),
        ));
    }
    if inputs.len() == 1 {
        return Ok(inputs[0].clone());
    }

    let mut max_id = 1u32;
    let mut documents_pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut documents_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut document = Document::with_version("1.7");

    for bytes in inputs {
        let mut doc = Document::load_mem(bytes)
            .map_err(|e| Error::Domain(format!("merge_pdfs: PDF nicht ladbar: {e}")))?;

        // Objekt-IDs versatzfrei hinter die bisherigen schieben.
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        for object_id in doc.get_pages().into_values() {
            let object = doc
                .get_object(object_id)
                .map_err(|e| Error::Domain(format!("merge_pdfs: Page-Objekt fehlt: {e}")))?
                .to_owned();
            documents_pages.insert(object_id, object);
        }
        documents_objects.extend(doc.objects);
    }

    // Catalog + Pages sammeln (ersten je Typ verwenden, Pages-Dictionaries mergen).
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects.into_iter() {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                catalog_object = Some((
                    catalog_object.map(|(id, _)| id).unwrap_or(object_id),
                    object,
                ));
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object {
                        if let Ok(old) = existing.as_dict() {
                            dictionary.extend(old);
                        }
                    }
                    pages_object = Some((
                        pages_object.map(|(id, _)| id).unwrap_or(object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" => {}     // separat verarbeitet
            b"Outlines" => {} // nicht unterstützt
            b"Outline" => {}  // nicht unterstützt
            _ => {
                document.objects.insert(object_id, object);
            }
        }
    }

    let pages_id = match pages_object.as_ref() {
        Some((id, _)) => *id,
        None => {
            return Err(Error::Domain(
                "merge_pdfs: keine Pages-Root gefunden".into(),
            ))
        }
    };

    // Alle Pages auf die neue gemeinsame Pages-Root umbiegen.
    for (object_id, object) in documents_pages.iter() {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    let (catalog_id, catalog_object) = match catalog_object {
        Some(c) => c,
        None => return Err(Error::Domain("merge_pdfs: kein Catalog gefunden".into())),
    };
    let (pages_id, pages_object) = pages_object.unwrap();

    // Neues Pages-Objekt: Count + Kids über alle gesammelten Pages.
    if let Ok(dictionary) = pages_object.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", documents_pages.len() as u32);
        dictionary.set(
            "Kids",
            documents_pages
                .into_keys()
                .map(Object::Reference)
                .collect::<Vec<_>>(),
        );
        document
            .objects
            .insert(pages_id, Object::Dictionary(dictionary));
    }

    // Neues Catalog-Objekt.
    if let Ok(dictionary) = catalog_object.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        document
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.len() as u32;
    document.renumber_objects();

    let mut buf: Vec<u8> = Vec::new();
    document
        .save_to(&mut buf)
        .map_err(|e| Error::Domain(format!("merge_pdfs: Speichern fehlgeschlagen: {e}")))?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// Minimales 1-seitiges PDF als Test-Fixture.
    fn one_page_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn merge_two_pdfs_yields_two_pages() {
        let merged = merge_pdfs(&[one_page_pdf(), one_page_pdf()]).unwrap();
        assert!(merged.starts_with(b"%PDF-"), "kein PDF-Header");
        let doc = Document::load_mem(&merged).unwrap();
        assert_eq!(doc.get_pages().len(), 2, "Bundle muss 2 Seiten haben");
    }

    #[test]
    fn merge_three_pdfs_yields_three_pages() {
        let merged = merge_pdfs(&[one_page_pdf(), one_page_pdf(), one_page_pdf()]).unwrap();
        let doc = Document::load_mem(&merged).unwrap();
        assert_eq!(doc.get_pages().len(), 3);
    }

    #[test]
    fn merge_single_pdf_is_passthrough() {
        let a = one_page_pdf();
        let merged = merge_pdfs(std::slice::from_ref(&a)).unwrap();
        assert_eq!(merged, a);
    }

    #[test]
    fn merge_skips_empty_parts() {
        let a = one_page_pdf();
        // Ein leerer Teil + ein echtes PDF → Passthrough des echten.
        let merged = merge_pdfs(&[Vec::new(), a.clone()]).unwrap();
        assert_eq!(merged, a);
    }

    #[test]
    fn merge_empty_errors() {
        let err = merge_pdfs(&[]).unwrap_err();
        assert!(format!("{err}").contains("keine PDF"));
    }
}
