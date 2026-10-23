//! Page-level PDF edits that never touch page content: keep some pages (Split PDF, pick pages)
//! and turn pages (Rotate pages). Pictures, text and fonts are copied exactly as they are.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use lopdf::{dictionary, Dictionary, Document, Object, ObjectId};

use super::crop::Turn;
use super::pdf::{inherit_page_attributes, load_pdf, pdf_error};
use crate::errors::AppError;

/// Load a PDF for editing. One that opens without a password is decrypted (the copy is saved
/// without the encryption); pages get their inherited size, rotation and resources written onto
/// themselves, so they can move to a new page tree.
pub fn open(path: &Path) -> Result<Document, AppError> {
    let mut doc = load_pdf(path)?;
    if doc.is_encrypted() {
        return Err(AppError::PasswordProtected);
    }
    inherit_page_attributes(&mut doc)?;
    Ok(doc)
}

pub fn page_count(doc: &Document) -> u32 {
    doc.get_pages().len() as u32
}

/// A new document with only `pages` (1-based, in this order) of `source`. Bookmarks and form
/// structure are left behind, and links to pages that aren't kept are removed so they don't drag
/// those pages (and their pictures) along.
pub fn keep_pages(source: &Document, pages: &[u32]) -> Result<Document, AppError> {
    if pages.is_empty() {
        return Err(AppError::invalid("Pick at least one page."));
    }
    let all = source.get_pages();
    let count = all.len();
    let ids = pages
        .iter()
        .map(|n| {
            all.get(n).copied().ok_or_else(|| {
                AppError::invalid(format!(
                    "This PDF has only {count} {}.",
                    if count == 1 { "page" } else { "pages" }
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let kept: HashSet<ObjectId> = ids.iter().copied().collect();

    let mut doc = source.clone();
    let pages_id = doc.new_object_id();
    for &id in &ids {
        let mut page = doc.get_dictionary(id).map_err(pdf_error)?.clone();
        page.set("Parent", pages_id);
        drop_links_to_other_pages(&doc, &mut page, &kept);
        doc.objects.insert(id, Object::Dictionary(page));
    }
    let kids: Vec<Object> = ids.iter().map(|&id| Object::Reference(id)).collect();
    let page_count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => page_count },
        ),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    // A fresh trailer: only what the new catalog (and the document info) reaches survives.
    let info = source.trailer.get(b"Info").ok().cloned();
    doc.trailer = dictionary! { "Root" => catalog_id };
    if let Some(info) = info {
        doc.trailer.set("Info", info);
    }
    doc.prune_objects();
    doc.renumber_objects();
    Ok(doc)
}

/// Remove link annotations whose target is a page outside `kept`.
fn drop_links_to_other_pages(doc: &Document, page: &mut Dictionary, kept: &HashSet<ObjectId>) {
    let annots = match page.get(b"Annots") {
        Ok(Object::Array(items)) => items.clone(),
        Ok(Object::Reference(id)) => match doc.get_object(*id) {
            Ok(Object::Array(items)) => items.clone(),
            _ => return,
        },
        _ => return,
    };
    let target_page = |annot: &Dictionary| -> Option<ObjectId> {
        let dest = annot.get(b"Dest").ok().or_else(|| {
            let action = match annot.get(b"A").ok()? {
                Object::Reference(id) => doc.get_dictionary(*id).ok()?,
                Object::Dictionary(d) => d,
                _ => return None,
            };
            action.get(b"D").ok()
        })?;
        match dest {
            Object::Array(parts) => parts.first()?.as_reference().ok(),
            _ => None, // named destinations live in the catalog, which isn't copied
        }
    };
    let keep: Vec<Object> = annots
        .into_iter()
        .filter(|item| {
            let annot = match item {
                Object::Reference(id) => doc.get_dictionary(*id).ok(),
                Object::Dictionary(d) => Some(d),
                _ => None,
            };
            annot
                .and_then(target_page)
                .is_none_or(|target| kept.contains(&target))
        })
        .collect();
    page.set("Annots", keep);
}

/// Each part as its own document (Split PDF). Parts are lists of 1-based pages.
pub fn split(source: &Document, parts: &[Vec<u32>]) -> Result<Vec<Document>, AppError> {
    parts
        .iter()
        .map(|pages| keep_pages(source, pages))
        .collect()
}

/// Turn pages: `turns` maps 1-based page numbers to quarter turns added to their rotation.
/// Only the page's /Rotate entry changes; its content is untouched.
pub fn rotate_pages(doc: &mut Document, turns: &HashMap<u32, Turn>) -> Result<(), AppError> {
    for (number, id) in doc.get_pages() {
        let Some(turn) = turns.get(&number).filter(|t| **t != Turn::None) else {
            continue;
        };
        let page = doc.get_dictionary_mut(id).map_err(pdf_error)?;
        let current = page.get(b"Rotate").and_then(Object::as_i64).unwrap_or(0);
        page.set(
            "Rotate",
            (current + i64::from(turn.degrees())).rem_euclid(360),
        );
    }
    Ok(())
}

pub fn save(doc: &mut Document) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::convert::pdf::{images_to_pdf, Margin, PageSize};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use std::io::Cursor;
    use std::path::PathBuf;

    pub(crate) fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("qc-pdfedit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A PDF whose page n holds a (90 + 10n) × 50 picture, so pages can be told apart.
    pub(crate) fn numbered_pdf(dir: &Path, pages: u32) -> PathBuf {
        let images: Vec<PathBuf> = (1..=pages)
            .map(|n| {
                let p = dir.join(format!("img{n}.jpg"));
                let mut buf = Vec::new();
                DynamicImage::ImageRgb8(RgbImage::from_pixel(
                    90 + 10 * n,
                    50,
                    Rgb([n as u8, 0, 0]),
                ))
                .write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
                .unwrap();
                std::fs::write(&p, buf).unwrap();
                p
            })
            .collect();
        let ok = |_: usize| Ok(());
        let mut doc = images_to_pdf(&images, PageSize::A4, Margin::None, 2, &ok).unwrap();
        let path = dir.join("doc.pdf");
        doc.save(&path).unwrap();
        path
    }

    /// Picture width on each page, in order.
    pub(crate) fn widths(doc: &Document) -> Vec<i64> {
        doc.get_pages()
            .into_values()
            .map(|page| {
                let (resources, _) = doc.get_page_resources(page).unwrap();
                let xobjects = resources
                    .unwrap()
                    .get(b"XObject")
                    .unwrap()
                    .as_dict()
                    .unwrap();
                let (_, image) = xobjects.iter().next().unwrap();
                let stream = doc
                    .get_object(image.as_reference().unwrap())
                    .unwrap()
                    .as_stream()
                    .unwrap();
                stream.dict.get(b"Width").unwrap().as_i64().unwrap()
            })
            .collect()
    }

    fn reload(mut doc: Document) -> Document {
        Document::load_mem(&save(&mut doc).unwrap()).unwrap()
    }

    #[test]
    fn keeps_the_chosen_pages_in_the_chosen_order_and_drops_the_rest() {
        let dir = temp_dir("keep");
        let source = open(&numbered_pdf(&dir, 5)).unwrap();
        let full = save(&mut source.clone()).unwrap().len();
        let kept = reload(keep_pages(&source, &[4, 2]).unwrap());
        assert_eq!(widths(&kept), [130, 110]);
        assert!(
            save(&mut kept.clone()).unwrap().len() < full / 2,
            "other pages' pictures are gone"
        );
        let err = keep_pages(&source, &[9]).unwrap_err();
        assert_eq!(err.to_string(), "This PDF has only 5 pages.");
        assert_eq!(
            keep_pages(&source, &[]).unwrap_err().code(),
            "invalid_input"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn links_to_pages_left_behind_are_removed() {
        let dir = temp_dir("links");
        let mut source = open(&numbered_pdf(&dir, 3)).unwrap();
        let pages = source.get_pages();
        let (first, third) = (pages[&1], pages[&3]);
        let link = source.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => vec![0.into(), 0.into(), 10.into(), 10.into()],
            "Dest" => vec![Object::Reference(third), Object::Name(b"Fit".to_vec())],
        });
        source
            .get_dictionary_mut(first)
            .unwrap()
            .set("Annots", vec![Object::Reference(link)]);

        let kept = reload(keep_pages(&source, &[1, 2]).unwrap());
        let page = kept.get_dictionary(kept.get_pages()[&1]).unwrap();
        assert!(page.get(b"Annots").unwrap().as_array().unwrap().is_empty());
        assert_eq!(widths(&kept), [100, 110], "page 3 didn't come along");
        let both = reload(keep_pages(&source, &[1, 3]).unwrap());
        let page = both.get_dictionary(both.get_pages()[&1]).unwrap();
        assert_eq!(
            page.get(b"Annots").unwrap().as_array().unwrap().len(),
            1,
            "kept: target kept"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn split_makes_one_document_per_part() {
        let dir = temp_dir("split");
        let source = open(&numbered_pdf(&dir, 5)).unwrap();
        let parts = split(&source, &[vec![1, 2], vec![3], vec![4, 5]]).unwrap();
        let widths: Vec<Vec<i64>> = parts.into_iter().map(|d| widths(&reload(d))).collect();
        assert_eq!(widths, [vec![100, 110], vec![120], vec![130, 140]]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rotation_adds_to_what_the_page_already_had() {
        let dir = temp_dir("rotate");
        let mut doc = open(&numbered_pdf(&dir, 3)).unwrap();
        let second = doc.get_pages()[&2];
        doc.get_dictionary_mut(second).unwrap().set("Rotate", 90);
        let turns = HashMap::from([(1, Turn::Left), (2, Turn::Right), (3, Turn::None)]);
        rotate_pages(&mut doc, &turns).unwrap();
        let doc = reload(doc);
        let rotation = |n: u32| {
            let page = doc.get_dictionary(doc.get_pages()[&n]).unwrap();
            page.get(b"Rotate").and_then(Object::as_i64).unwrap_or(0)
        };
        assert_eq!((rotation(1), rotation(2), rotation(3)), (270, 180, 0));
        assert_eq!(widths(&doc), [100, 110, 120], "content untouched");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn inherited_rotation_counts() {
        let dir = temp_dir("inherited");
        let mut raw = load_pdf(&numbered_pdf(&dir, 1)).unwrap();
        // Rotation set on the page tree, not the page.
        let root = raw.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let pages = raw
            .get_dictionary(root)
            .unwrap()
            .get(b"Pages")
            .unwrap()
            .as_reference()
            .unwrap();
        raw.get_dictionary_mut(pages).unwrap().set("Rotate", 180);
        let path = dir.join("tree.pdf");
        raw.save(&path).unwrap();

        let mut doc = open(&path).unwrap();
        rotate_pages(&mut doc, &HashMap::from([(1, Turn::Right)])).unwrap();
        let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
        assert_eq!(page.get(b"Rotate").unwrap().as_i64().unwrap(), 270);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
