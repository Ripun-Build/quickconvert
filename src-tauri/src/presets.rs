//! Presets (SPECS §8): shipped examples merged with the user's own (app config dir).

use std::path::Path;

use serde::Deserialize;

use crate::convert::compress::Preset;

#[derive(Deserialize)]
struct PresetFile {
    presets: Vec<Preset>,
}

fn read(path: &Path) -> Option<Vec<Preset>> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<PresetFile>(&text) {
        Ok(file) => Some(file.presets),
        Err(err) => {
            eprintln!("[presets] ignoring {path:?}: {err}");
            None
        }
    }
}

/// Shipped presets (first readable file of `shipped`), then the user's: a user preset with the
/// same id replaces the shipped one, new ones are appended. Unreadable files are skipped.
pub fn load(shipped: &[impl AsRef<Path>], user: Option<&Path>) -> Vec<Preset> {
    let mut presets = shipped
        .iter()
        .find_map(|p| read(p.as_ref()))
        .unwrap_or_default();
    for preset in user.and_then(read).unwrap_or_default() {
        match presets.iter_mut().find(|p| p.id == preset.id) {
            Some(existing) => *existing = preset,
            None => presets.push(preset),
        }
    }
    presets
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
    fn broken_user_file_is_ignored() {
        let dir = std::env::temp_dir().join(format!("qc-presets-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let user = dir.join("presets.json");
        std::fs::write(&user, "{ not json").unwrap();
        assert_eq!(load(&[shipped()], Some(&user)).len(), 6);
        assert!(load(&[PathBuf::from("/missing.json")], None).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
