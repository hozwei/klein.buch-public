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
//!
//! **Aktive Inhalte (W7-BUNDLE-ACTIVE-PDF, Lauf F5c):** Die Rechtsdokumente sind
//! hochgeladene PDFs. Vor dem Zusammenführen entfernt [`strip_active_content`]
//! Auslöser und Aktionen, die beim Öffnen Code ausführen, Programme starten,
//! Formulardaten senden oder eingebettete Dateien mitbringen (`/OpenAction`,
//! `/AA`, JavaScript-/Launch-/SubmitForm-/ImportData-/Rendition-/GoToE-Aktionen,
//! `/JavaScript`- und `/EmbeddedFiles`-Namensbäume). Links (`/URI`, `/GoTo`)
//! bleiben. Der Catalog des Bundles ist der des ersten Teils (Angebots-PDF aus
//! Klein.Buch), nicht der eines Uploads. Der Mail-Versand hängt die archivierten
//! Originale unverändert an (Nachweis der versandten Fassung, ADR 0018).

use std::collections::{BTreeMap, BTreeSet};

use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::error::{Error, Result};

/// Obergrenze je Stream beim Laden fremder PDFs (Plan W14).
const MAX_DECOMPRESSED_STREAM: usize = 64 * 1024 * 1024;

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
        // W14: Rechtsdokumente sind hochgeladene PDFs — Objekt-/Xref-Streams
        // beim Laden höchstens 64 MiB entpacken (Dekompressionsbombe).
        let options = lopdf::LoadOptions::with_max_decompressed_size(MAX_DECOMPRESSED_STREAM);
        let mut doc = Document::load_mem_with_options(bytes, options)
            .map_err(|e| Error::Domain(format!("merge_pdfs: PDF nicht ladbar: {e}")))?;
        strip_active_content(&mut doc);

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
                // Catalog des ersten Teils (eigenes Angebots-PDF); die Objekt-
                // IDs sind aufsteigend je Teil vergeben.
                if catalog_object.is_none() {
                    catalog_object = Some((object_id, object));
                }
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

/// Aktionsarten, die beim Öffnen oder Klicken Code ausführen, Programme,
/// externe bzw. eingebettete Dateien oder Medien öffnen, Daten senden oder die
/// Sichtbarkeit von Inhalten umschalten (ISO 32000-1, 12.6.4; ISO 32000-2
/// RichMediaExecute; F5-fix: GoToR, Thread, Movie, Sound, RichMediaExecute,
/// SetOCGState, GoTo3DView).
const ACTIVE_ACTIONS: &[&[u8]] = &[
    b"JavaScript",
    b"Launch",
    b"SubmitForm",
    b"ImportData",
    b"Rendition",
    b"GoToE",
    b"GoToR",
    b"Thread",
    b"Movie",
    b"Sound",
    b"RichMediaExecute",
    b"SetOCGState",
    b"GoTo3DView",
];

/// Annotations-Typen mit Medien- bzw. 3D-Inhalten (ISO 32000-1, 12.5.6), die
/// beim Zusammenführen ganz entfallen.
const ACTIVE_ANNOTATIONS: &[&[u8]] = &[b"RichMedia", b"Screen", b"Movie", b"Sound", b"3D"];

/// URI-Schemata, die als Link erhalten bleiben (alles andere, z. B.
/// `javascript:` oder `file:`, gilt als aktive Aktion).
const SAFE_URI_SCHEMES: &[&str] = &["http://", "https://", "mailto:"];

fn is_safe_uri(uri: &[u8]) -> bool {
    let text = String::from_utf8_lossy(uri);
    let lower = text.trim_start().to_ascii_lowercase();
    SAFE_URI_SCHEMES.iter().any(|s| lower.starts_with(s))
}

fn is_active_action(dict: &Dictionary) -> bool {
    match dict.get(b"S") {
        Ok(Object::Name(n)) if ACTIVE_ACTIONS.contains(&n.as_slice()) => true,
        Ok(Object::Name(n)) if n.as_slice() == b"URI" => {
            !matches!(dict.get(b"URI"), Ok(Object::String(uri, _)) if is_safe_uri(uri))
        }
        _ => false,
    }
}

fn is_active_annotation(dict: &Dictionary) -> bool {
    matches!(dict.get(b"Subtype"), Ok(Object::Name(n)) if ACTIVE_ANNOTATIONS.contains(&n.as_slice()))
}

/// Entfernt aktive Inhalte aus einem geladenen PDF (W7-BUNDLE-ACTIVE-PDF):
/// Auslöser `/OpenAction` und `/AA` überall, `/A`-Einträge mit aktiver Aktion
/// (direkt oder als Verweis; das Aktions-Objekt selbst wird `null`), die
/// Namensbäume `/JavaScript` und `/EmbeddedFiles` sowie Medien- und
/// 3D-Annotationen (aus `/Annots` entfernt, Objekt `null`). Links mit
/// anderem Schema als http(s)/mailto verlieren ihre Aktion. Seiteninhalt,
/// Schriften und Web-Links bleiben unverändert.
fn strip_active_content(doc: &mut Document) {
    let active: BTreeSet<ObjectId> = doc
        .objects
        .iter()
        .filter(|(_, o)| {
            o.as_dict()
                .is_ok_and(|d| is_active_action(d) || is_active_annotation(d))
        })
        .map(|(id, _)| *id)
        .collect();
    for (id, object) in doc.objects.iter_mut() {
        // Aktive Aktions-Objekte selbst werden `null` (Objektnummer bleibt,
        // damit keine Referenz ins Leere oder auf ein anderes Objekt zeigt).
        if active.contains(id) {
            *object = Object::Null;
            continue;
        }
        let dict = match object {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &mut s.dict,
            // Indirekte Arrays (z. B. ein per Verweis gespeichertes `/Annots`)
            // verlieren ihre Verweise auf entfernte Objekte.
            Object::Array(items) => {
                items.retain(|a| !matches!(a, Object::Reference(r) if active.contains(r)));
                continue;
            }
            _ => continue,
        };
        dict.remove(b"OpenAction");
        dict.remove(b"AA");
        dict.remove(b"JavaScript");
        dict.remove(b"EmbeddedFiles");
        if let Ok(Object::Array(annots)) = dict.get_mut(b"Annots") {
            annots.retain(|a| match a {
                Object::Reference(id) => !active.contains(id),
                Object::Dictionary(d) => !is_active_annotation(d),
                _ => true,
            });
        }
        let drop_action = match dict.get(b"A") {
            Ok(Object::Dictionary(a)) => is_active_action(a),
            Ok(Object::Reference(id)) => active.contains(id),
            _ => false,
        };
        if drop_action {
            dict.remove(b"A");
        }
        // Folgeaktionen (`/Next`) einer verbleibenden Aktion ebenso.
        let drop_next = match dict.get(b"Next") {
            Ok(Object::Dictionary(a)) => is_active_action(a),
            Ok(Object::Reference(id)) => active.contains(id),
            Ok(Object::Array(_)) => true,
            _ => false,
        };
        if drop_next {
            dict.remove(b"Next");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// 1-seitiges PDF mit aktiven Inhalten: JavaScript beim Öffnen (Catalog),
    /// Seiten-Auslöser, Launch-Link, eingebettete Datei — und ein harmloser
    /// URI-Link, der bleiben muss.
    fn active_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let js_id = doc.add_object(dictionary! {
            "S" => "JavaScript",
            "JS" => Object::string_literal("app.alert('aktiv')"),
        });
        let launch_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "A" => dictionary! { "S" => "Launch", "F" => Object::string_literal("calc.exe") },
        });
        let uri_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 20.into(), 10.into(), 30.into()],
            "A" => dictionary! { "S" => "URI", "URI" => Object::string_literal("https://example.org/agb") },
        });
        let js_uri_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 40.into(), 10.into(), 50.into()],
            "A" => dictionary! { "S" => "URI", "URI" => Object::string_literal(" JavaScript:alert(1)") },
        });
        let gotor_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 60.into(), 10.into(), 70.into()],
            "A" => dictionary! { "S" => "GoToR", "F" => Object::string_literal("remote-target.pdf") },
        });
        let sound_action = doc.add_object(dictionary! {
            "S" => "Sound",
            "Sound" => Object::string_literal("sound-target"),
        });
        let sound_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 80.into(), 10.into(), 90.into()],
            "A" => sound_action,
        });
        let movie_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 100.into(), 10.into(), 110.into()],
            "A" => dictionary! { "S" => "Movie", "T" => Object::string_literal("movie-target") },
        });
        let rich_media_exec = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 120.into(), 10.into(), 130.into()],
            "A" => dictionary! { "S" => "RichMediaExecute", "CMD" => Object::string_literal("richmedia-cmd") },
        });
        let thread_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 140.into(), 10.into(), 150.into()],
            "A" => dictionary! { "S" => "Thread", "F" => Object::string_literal("thread-target.pdf") },
        });
        let rich_media = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "RichMedia",
            "Rect" => vec![0.into(), 160.into(), 10.into(), 170.into()],
            "NM" => Object::string_literal("richmedia-annot"),
        });
        let screen = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Screen",
            "Rect" => vec![0.into(), 180.into(), 10.into(), 190.into()],
            "NM" => Object::string_literal("screen-annot"),
        });
        let three_d = dictionary! {
            "Type" => "Annot",
            "Subtype" => "3D",
            "Rect" => vec![0.into(), 200.into(), 10.into(), 210.into()],
            "NM" => Object::string_literal("threed-annot"),
        };
        let mailto_link = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![0.into(), 220.into(), 10.into(), 230.into()],
            "A" => dictionary! { "S" => "URI", "URI" => Object::string_literal("mailto:info@example.org") },
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Annots" => vec![
                launch_link.into(),
                uri_link.into(),
                js_uri_link.into(),
                gotor_link.into(),
                sound_link.into(),
                movie_link.into(),
                rich_media_exec.into(),
                thread_link.into(),
                rich_media.into(),
                screen.into(),
                Object::Dictionary(three_d),
                mailto_link.into(),
            ],
            "AA" => dictionary! { "O" => js_id },
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let names = doc.add_object(dictionary! {
            "JavaScript" => dictionary! { "Names" => vec![Object::string_literal("a"), js_id.into()] },
            "EmbeddedFiles" => dictionary! { "Names" => vec![Object::string_literal("x.exe"), js_id.into()] },
        });
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "OpenAction" => js_id,
            "Names" => names,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// W7-BUNDLE-ACTIVE-PDF: Das Bundle enthält keine Auslöser und keine aktiven
    /// Aktionen aus einem hochgeladenen Rechtsdokument; Seiten und Links bleiben.
    #[test]
    fn merge_strips_active_content_of_uploaded_parts() {
        let merged = merge_pdfs(&[one_page_pdf(), active_pdf()]).unwrap();
        let doc = Document::load_mem(&merged).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let catalog = doc.catalog().unwrap();
        assert!(catalog.get(b"OpenAction").is_err(), "kein OpenAction");
        for object in doc.objects.values() {
            let Ok(dict) = object.as_dict() else { continue };
            for key in [&b"AA"[..], b"OpenAction", b"JavaScript", b"EmbeddedFiles"] {
                assert!(
                    dict.get(key).is_err(),
                    "{} bleibt",
                    String::from_utf8_lossy(key)
                );
            }
            if let Ok(Object::Dictionary(a)) = dict.get(b"A") {
                assert!(!is_active_action(a), "aktive Aktion bleibt: {a:?}");
            }
        }
        let text = String::from_utf8_lossy(&merged);
        assert!(text.contains("https://example.org/agb"), "URI-Link bleibt");
        assert!(
            text.contains("mailto:info@example.org"),
            "mailto-Link bleibt"
        );
        assert!(!text.contains("calc.exe"), "Launch-Ziel entfernt");
        // F5-fix: weitere aktive Aktionen und Medien-/3D-Annotationen.
        for gone in [
            "alert(1)",
            "remote-target.pdf",
            "sound-target",
            "movie-target",
            "richmedia-cmd",
            "thread-target.pdf",
            "richmedia-annot",
            "screen-annot",
            "threed-annot",
        ] {
            assert!(!text.contains(gone), "{gone} bleibt im Bundle");
        }
        for object in doc.objects.values() {
            let Ok(dict) = object.as_dict() else { continue };
            assert!(!is_active_annotation(dict), "Medien-Annotation bleibt");
            if let Ok(Object::Array(annots)) = dict.get(b"Annots") {
                for a in annots {
                    if let Object::Reference(id) = a {
                        let target = doc.get_object(*id).unwrap();
                        assert!(
                            !matches!(target, Object::Null),
                            "Annots verweist auf entferntes Objekt"
                        );
                    }
                }
            }
        }
        assert!(
            !text.contains("app.alert"),
            "JavaScript-Aktion neutralisiert"
        );
    }

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
