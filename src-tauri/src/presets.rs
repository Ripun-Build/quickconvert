//! Presets (SPECS §8): shipped examples merged with the user's own `presets.json` (app config
//! dir), which holds the user's presets plus the ids of built-ins they removed.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::convert::compress::Preset;
use crate::errors::AppError;

#[derive(Default, Serialize, Deserialize)]
struct PresetFile {
    #[serde(default)]
    presets: Vec<Preset>,
    /// Built-in ids the user deleted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    removed: Vec<String>,
}

fn read(path: &Path) -> Option<PresetFile> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<PresetFile>(&text) {
        Ok(file) => Some(file),
        Err(err) => {
            eprintln!("[presets] ignoring {path:?}: {err}");
            None
        }
    }
}

fn shipped_presets(shipped: &[impl AsRef<Path>]) -> Vec<Preset> {
    shipped
        .iter()
        .find_map(|p| read(p.as_ref()))
        .map(|f| f.presets)
        .unwrap_or_default()
}

/// Shipped presets (first readable file of `shipped`) minus removed ones, then the user's: a user
/// preset with the same id replaces the shipped one, new ones are appended. Unreadable files are
/// skipped.
pub fn load(shipped: &[impl AsRef<Path>], user: Option<&Path>) -> Vec<Preset> {
    let user = user.and_then(read).unwrap_or_default();
    let removed: HashSet<&str> = user.removed.iter().map(String::as_str).collect();
    let mut presets: Vec<Preset> = shipped_presets(shipped)
        .into_iter()
        .filter(|p| !removed.contains(p.id.as_str()))
        .collect();
    for preset in user.presets {
        match presets.iter_mut().find(|p| p.id == preset.id) {
            Some(existing) => *existing = preset,
            None => presets.push(preset),
        }
    }
    presets
}

/// A preset as the editor sees it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreset {
    pub preset: Preset,
    /// Shipped with the app (an example), possibly edited.
    pub builtin: bool,
}

pub fn for_editor(shipped: &[impl AsRef<Path>], user: Option<&Path>) -> Vec<EditorPreset> {
    let ids: HashSet<String> = shipped_presets(shipped).into_iter().map(|p| p.id).collect();
    load(shipped, user)
        .into_iter()
        .map(|preset| EditorPreset {
            builtin: ids.contains(&preset.id),
            preset,
        })
        .collect()
}

/// Save the editor's full list: only what differs from the shipped presets is written.
pub fn save(shipped: &[impl AsRef<Path>], user: &Path, list: &[Preset]) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for preset in list {
        if preset.id.trim().is_empty() || preset.label.trim().is_empty() {
            return Err(AppError::invalid("Every preset needs a name."));
        }
        if !seen.insert(preset.id.as_str()) {
            return Err(AppError::invalid("Two presets have the same id."));
        }
    }
    let shipped = shipped_presets(shipped);
    let file = PresetFile {
        presets: list
            .iter()
            .filter(|p| !shipped.contains(p)) // unchanged built-ins aren't stored
            .cloned()
            .collect(),
        removed: shipped
            .iter()
            .filter(|s| !seen.contains(s.id.as_str()))
            .map(|s| s.id.clone())
            .collect(),
    };
    crate::settings::write_json(user, &file)
}

/// Bring back every built-in as shipped; the user's own presets stay.
pub fn restore(shipped: &[impl AsRef<Path>], user: &Path) -> Result<(), AppError> {
    let ids: HashSet<String> = shipped_presets(shipped).into_iter().map(|p| p.id).collect();
    let current = read(user).unwrap_or_default();
    let file = PresetFile {
        presets: current
            .presets
            .into_iter()
            .filter(|p| !ids.contains(&p.id))
            .collect(),
        removed: Vec::new(),
    };
    crate::settings::write_json(user, &file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn shipped() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/presets.json")
    }

    #[test]
    fn user_presets_override_by_id_and_append() {
        let dir = std::env::temp_dir().join(format!("qc-presets-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let user = dir.join("presets.json");
        std::fs::write(
            &user,
            r#"{"presets":[
                {"id":"signature_example","label":"My signature","format":"jpg","width":150,"height":50},
                {"id":"mine","label":"Mine","format":"png","longEdge":800}
            ]}"#,
        )
        .unwrap();

        let presets = load(&[PathBuf::from("/missing.json"), shipped()], Some(&user));
        assert_eq!(presets.len(), 7);
        let sig = presets
            .iter()
            .find(|p| p.id == "signature_example")
            .unwrap();
        assert_eq!((sig.label.as_str(), sig.width), ("My signature", Some(150)));
        assert_eq!(presets.last().unwrap().id, "mine");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn presets_saved_by_v0_1_0_load_unchanged() {
        // A v0.1.0 presets.json: an edited built-in, a deleted built-in and one of the user's own.
        let dir = std::env::temp_dir().join(format!("qc-presets-v010-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let user = dir.join("presets.json");
        std::fs::write(
            &user,
            r#"{"presets":[
                {"id":"signature_example","label":"Signature for the bank","format":"jpg","width":140,"height":60,"minKB":10,"maxKB":20,"dpi":200},
                {"id":"my-photo","label":"My photo","format":"jpg","aspect":"3:4","longEdge":900,"maxKB":100,"note":"For the club form"}
            ],"removed":["whatsapp_sharp"]}"#,
        )
        .unwrap();
        let presets = load(&[shipped()], Some(&user));
        assert!(
            presets.iter().all(|p| p.id != "whatsapp_sharp"),
            "deleted built-in stays deleted"
        );
        let sig = presets
            .iter()
            .find(|p| p.id == "signature_example")
            .unwrap();
        assert_eq!(
            (sig.label.as_str(), sig.min_kb, sig.max_kb, sig.dpi),
            ("Signature for the bank", Some(10), Some(20), Some(200))
        );
        let mine = presets.last().unwrap();
        assert_eq!(
            (
                mine.id.as_str(),
                mine.aspect.as_deref(),
                mine.long_edge,
                mine.note.as_deref()
            ),
            (
                "my-photo",
                Some("3:4"),
                Some(900),
                Some("For the club form")
            )
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn broken_user_file_is_ignored() {
        let dir = std::env::temp_dir().join(format!("qc-presets-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let user = dir.join("presets.json");
        std::fs::write(&user, "{ not json").unwrap();
        assert_eq!(load(&[shipped()], Some(&user)).len(), 6);
        assert!(load(&[PathBuf::from("/missing.json")], None).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn editor_saves_only_differences_and_restore_brings_builtins_back() {
        let dir = std::env::temp_dir().join(format!("qc-presets-edit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let user = dir.join("presets.json");
        let ship = [shipped()];

        let mut list: Vec<Preset> = for_editor(&ship, Some(&user))
            .into_iter()
            .map(|e| e.preset)
            .collect();
        assert_eq!(list.len(), 6);
        // Edit one built-in, delete another, add one of our own.
        list[0].label = "My exam photo".into();
        list.retain(|p| p.id != "whatsapp_sharp");
        let mut mine = list[1].clone();
        mine.id = "mine".into();
        mine.label = "Mine".into();
        list.push(mine);
        save(&ship, &user, &list).unwrap();

        let stored: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&user).unwrap()).unwrap();
        assert_eq!(
            stored["presets"].as_array().unwrap().len(),
            2,
            "edited + new only"
        );
        assert_eq!(stored["removed"], serde_json::json!(["whatsapp_sharp"]));

        let after = for_editor(&ship, Some(&user));
        assert_eq!(after.len(), 6);
        assert_eq!(after[0].preset.label, "My exam photo");
        assert!(after[0].builtin);
        assert!(!after.last().unwrap().builtin);
        assert!(after.iter().all(|e| e.preset.id != "whatsapp_sharp"));

        restore(&ship, &user).unwrap();
        let restored = load(&ship, Some(&user));
        assert_eq!(restored.len(), 7, "6 built-ins + mine");
        assert_eq!(restored[0].label, "Exam/job photo (example)");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_rejects_blank_names_and_duplicate_ids() {
        let dir = std::env::temp_dir().join(format!("qc-presets-bad-save-{}", std::process::id()));
        let user = dir.join("presets.json");
        let mut list = load(&[shipped()], None);
        list[1].label = " ".into();
        assert_eq!(
            save(&[shipped()], &user, &list).unwrap_err().to_string(),
            "Every preset needs a name."
        );
        let mut list = load(&[shipped()], None);
        list[1].id = list[0].id.clone();
        assert!(save(&[shipped()], &user, &list).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
