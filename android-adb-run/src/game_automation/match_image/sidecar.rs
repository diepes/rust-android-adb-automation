//! Sidecar JSON metadata for Template images.
//!
//! A PNG in `assets/test_images/` becomes a **Template** only when a sidecar
//! JSON file with the same stem exists alongside it (e.g. `login.json` for
//! `login.png`).  The sidecar is the sole owner of MatchTarget definitions.
//!
//! # File layout
//! ```text
//! assets/test_images/
//!   login.png          ← raw image captured from device
//!   login.json         ← sidecar: promotes login.png to a Template
//! ```
//!
//! # Minimal schema
//! ```json
//! {
//!   "match_targets": [
//!     { "name": "login_button", "crop": { "x": 120, "y": 540, "width": 200, "height": 60 } }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A rectangle in Template-image pixel coordinates used to crop the sub-image
/// that will be matched against a Screenshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One named match unit defined inside a Template sidecar.
///
/// A single Template PNG can contain many MatchTargetDefs — for example a
/// screenshot of a login screen might define `login_button`, `username_field`,
/// and `logo` as independent targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchTargetDef {
    /// Human-readable identifier used to refer to this target in TimedEvents
    /// and Scene definitions.
    pub name: String,
    /// Sub-rectangle of the Template image to use as the match pattern.
    pub crop: CropRect,
}

/// The top-level sidecar document stored in `<image-stem>.json`.
///
/// Additional per-MatchTarget fields (SearchRegion hints, Scene links) can be
/// added in future without breaking existing sidecars because `serde` will
/// ignore unknown fields by default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateSidecar {
    pub match_targets: Vec<MatchTargetDef>,
}

impl TemplateSidecar {
    /// Derive the sidecar path for a given image path.
    ///
    /// `login.png` → `login.json`
    pub fn sidecar_path(image_path: &Path) -> PathBuf {
        image_path.with_extension("json")
    }

    /// Return `true` when a sidecar file exists alongside `image_path`.
    pub fn has_sidecar(image_path: &Path) -> bool {
        Self::sidecar_path(image_path).exists()
    }

    /// Load a sidecar from disk.  Returns an error if the file is missing or
    /// cannot be parsed.
    pub fn load_for(image_path: &Path) -> Result<Self, String> {
        let sidecar_path = Self::sidecar_path(image_path);
        let content = std::fs::read_to_string(&sidecar_path).map_err(|e| {
            format!(
                "Failed to read sidecar {}: {}",
                sidecar_path.display(),
                e
            )
        })?;
        serde_json::from_str(&content).map_err(|e| {
            format!(
                "Failed to parse sidecar {}: {}",
                sidecar_path.display(),
                e
            )
        })
    }

    /// Persist the sidecar to disk, creating or overwriting the `.json` file.
    pub fn save_for(&self, image_path: &Path) -> Result<(), String> {
        let sidecar_path = Self::sidecar_path(image_path);
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialise sidecar: {}", e))?;
        std::fs::write(&sidecar_path, content).map_err(|e| {
            format!(
                "Failed to write sidecar {}: {}",
                sidecar_path.display(),
                e
            )
        })
    }

    /// Add a new MatchTarget and immediately save the sidecar.
    ///
    /// Returns an error if a target with the same name already exists, or if
    /// the save fails.
    pub fn add_target(&mut self, image_path: &Path, target: MatchTargetDef) -> Result<(), String> {
        if self.match_targets.iter().any(|t| t.name == target.name) {
            return Err(format!("MatchTarget '{}' already exists", target.name));
        }
        self.match_targets.push(target);
        self.save_for(image_path)
    }

    /// Remove a MatchTarget by name and immediately save the sidecar.
    ///
    /// Returns an error if the target does not exist or if the save fails.
    pub fn remove_target(&mut self, image_path: &Path, name: &str) -> Result<(), String> {
        let before = self.match_targets.len();
        self.match_targets.retain(|t| t.name != name);
        if self.match_targets.len() == before {
            return Err(format!("MatchTarget '{}' not found", name));
        }
        self.save_for(image_path)
    }

    /// Rename a MatchTarget and immediately save the sidecar.
    pub fn rename_target(
        &mut self,
        image_path: &Path,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), String> {
        if self.match_targets.iter().any(|t| t.name == new_name) {
            return Err(format!("MatchTarget '{}' already exists", new_name));
        }
        match self.match_targets.iter_mut().find(|t| t.name == old_name) {
            Some(t) => t.name = new_name.to_string(),
            None => return Err(format!("MatchTarget '{}' not found", old_name)),
        }
        self.save_for(image_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_png() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("screen.png");
        fs::write(&png, b"fake png content").unwrap();
        (dir, png)
    }

    #[test]
    fn test_sidecar_path_derives_from_png() {
        let (_dir, png) = tmp_png();
        let sidecar = TemplateSidecar::sidecar_path(&png);
        assert_eq!(sidecar.extension().unwrap(), "json");
        assert_eq!(sidecar.file_stem().unwrap(), png.file_stem().unwrap());
    }

    #[test]
    fn test_has_sidecar_false_when_missing() {
        let (_dir, png) = tmp_png();
        assert!(!TemplateSidecar::has_sidecar(&png));
    }

    #[test]
    fn test_roundtrip_save_and_load() {
        let (_dir, png) = tmp_png();
        let mut sidecar = TemplateSidecar::default();
        sidecar
            .match_targets
            .push(MatchTargetDef {
                name: "login_button".to_string(),
                crop: CropRect {
                    x: 10,
                    y: 20,
                    width: 100,
                    height: 50,
                },
            });

        sidecar.save_for(&png).unwrap();
        assert!(TemplateSidecar::has_sidecar(&png));

        let loaded = TemplateSidecar::load_for(&png).unwrap();
        assert_eq!(loaded.match_targets.len(), 1);
        assert_eq!(loaded.match_targets[0].name, "login_button");
        assert_eq!(loaded.match_targets[0].crop.x, 10);
    }

    #[test]
    fn test_add_target_saves_immediately() {
        let (_dir, png) = tmp_png();
        let mut sidecar = TemplateSidecar::default();
        sidecar
            .add_target(
                &png,
                MatchTargetDef {
                    name: "btn".to_string(),
                    crop: CropRect { x: 0, y: 0, width: 50, height: 30 },
                },
            )
            .unwrap();

        let loaded = TemplateSidecar::load_for(&png).unwrap();
        assert_eq!(loaded.match_targets[0].name, "btn");
    }

    #[test]
    fn test_add_duplicate_name_is_error() {
        let (_dir, png) = tmp_png();
        let mut sidecar = TemplateSidecar::default();
        let target = MatchTargetDef {
            name: "btn".to_string(),
            crop: CropRect { x: 0, y: 0, width: 50, height: 30 },
        };
        sidecar.add_target(&png, target.clone()).unwrap();
        assert!(sidecar.add_target(&png, target).is_err());
    }

    #[test]
    fn test_remove_target() {
        let (_dir, png) = tmp_png();
        let mut sidecar = TemplateSidecar::default();
        sidecar
            .add_target(
                &png,
                MatchTargetDef {
                    name: "btn".to_string(),
                    crop: CropRect { x: 0, y: 0, width: 50, height: 30 },
                },
            )
            .unwrap();
        sidecar.remove_target(&png, "btn").unwrap();

        let loaded = TemplateSidecar::load_for(&png).unwrap();
        assert!(loaded.match_targets.is_empty());
    }

    #[test]
    fn test_rename_target() {
        let (_dir, png) = tmp_png();
        let mut sidecar = TemplateSidecar::default();
        sidecar
            .add_target(
                &png,
                MatchTargetDef {
                    name: "old".to_string(),
                    crop: CropRect { x: 0, y: 0, width: 10, height: 10 },
                },
            )
            .unwrap();
        sidecar.rename_target(&png, "old", "new").unwrap();

        let loaded = TemplateSidecar::load_for(&png).unwrap();
        assert_eq!(loaded.match_targets[0].name, "new");
    }

    #[test]
    fn test_remove_nonexistent_is_error() {
        let (_dir, png) = tmp_png();
        let mut sidecar = TemplateSidecar::default();
        sidecar.save_for(&png).unwrap();
        assert!(sidecar.remove_target(&png, "ghost").is_err());
    }
}
