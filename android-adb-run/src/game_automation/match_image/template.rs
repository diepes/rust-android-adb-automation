//! Template management and matching functionality

use super::region::SearchRegion;
use super::sidecar::{MatchTargetDef, TemplateSidecar};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum TemplateCategory {
    Button,
    Icon,
    GameObject,
    UI,
    Text,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Template {
    pub path: String,
    /// Stem of the PNG file, e.g. `login` for `login.png`.
    pub name: String,
    /// Name of the specific MatchTarget within the sidecar, e.g. `login_button`.
    pub match_target_name: String,
    pub search_region: SearchRegion,
    /// Top-left x coordinate of the crop within the Template image.
    pub crop_x: u32,
    /// Top-left y coordinate of the crop within the Template image.
    pub crop_y: u32,
    /// Width of the crop region used for matching (from sidecar crop).
    pub width: u32,
    /// Height of the crop region used for matching (from sidecar crop).
    pub height: u32,
    pub category: TemplateCategory,
}

impl Template {
    /// Create a Template from a PNG path and a `MatchTargetDef` sourced from the sidecar.
    pub fn from_match_target(
        path: String,
        search_region: SearchRegion,
        target: &MatchTargetDef,
    ) -> Result<Self, String> {
        let image = image::open(&path)
            .map_err(|e| format!("Failed to load template {}: {}", path, e))?;

        let crop = &target.crop;

        if crop.x + crop.width > image.width() || crop.y + crop.height > image.height() {
            return Err(format!(
                "MatchTarget '{}' crop [{},{},{},{}] exceeds image bounds ({}x{})",
                target.name,
                crop.x,
                crop.y,
                crop.width,
                crop.height,
                image.width(),
                image.height()
            ));
        }

        let name = Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let category = Self::determine_category(&target.name);

        Ok(Self {
            path,
            name,
            match_target_name: target.name.clone(),
            search_region,
            crop_x: crop.x,
            crop_y: crop.y,
            width: crop.width,
            height: crop.height,
            category,
        })
    }

    fn determine_category(name: &str) -> TemplateCategory {
        let name_lower = name.to_lowercase();

        if name_lower.contains("button") || name_lower.contains("btn") {
            TemplateCategory::Button
        } else if name_lower.contains("icon") {
            TemplateCategory::Icon
        } else if name_lower.contains("ui") || name_lower.contains("menu") {
            TemplateCategory::UI
        } else if name_lower.contains("text") || name_lower.contains("label") {
            TemplateCategory::Text
        } else if name_lower.contains("object") || name_lower.contains("item") {
            TemplateCategory::GameObject
        } else {
            TemplateCategory::Unknown
        }
    }

    /// Check if this template is valid for matching
    pub fn is_valid(&self) -> bool {
        Path::new(&self.path).exists()
            && self.search_region.is_valid()
            && self.width > 0
            && self.height > 0
    }

    /// Get the center tap coordinates for this template at a match location
    pub fn get_tap_coordinates(&self, match_x: u32, match_y: u32) -> (u32, u32) {
        (match_x + self.width / 2, match_y + self.height / 2)
    }
}

#[derive(Debug, Clone)]
pub struct TemplateMatch {
    pub template: Template,
    pub x: u32,
    pub y: u32,
    pub confidence: f32,
    pub scale_factor: f32,
}

impl TemplateMatch {
    pub fn new(template: Template, x: u32, y: u32, confidence: f32, scale_factor: f32) -> Self {
        Self {
            template,
            x,
            y,
            confidence,
            scale_factor,
        }
    }

    /// Get tap coordinates at the center of this match
    pub fn get_tap_coordinates(&self) -> (u32, u32) {
        self.template.get_tap_coordinates(self.x, self.y)
    }

    /// Check if this match is within screen bounds
    pub fn is_within_bounds(&self, screen_width: u32, screen_height: u32) -> bool {
        let (tap_x, tap_y) = self.get_tap_coordinates();
        tap_x < screen_width && tap_y < screen_height
    }
}

/// Manager for loading and organizing templates
pub struct TemplateManager {
    templates: Vec<Template>,
    screen_width: u32,
    screen_height: u32,
}

impl TemplateManager {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            templates: Vec::new(),
            screen_width,
            screen_height,
        }
    }

    /// Scan directory for PNG files that have a companion sidecar JSON and load
    /// one `Template` per `MatchTargetDef` declared in the sidecar.
    ///
    /// PNG files without a sidecar are skipped — they are saved Screenshots,
    /// not yet Templates.
    pub fn load_templates_from_directory(&mut self, directory: &str) -> Result<usize, String> {
        use super::region::RegionManager;

        let region_manager = RegionManager::new(self.screen_width, self.screen_height);
        let dir_path = Path::new(directory);

        if !dir_path.exists() {
            return Err(format!("Template directory not found: {}", directory));
        }

        let mut loaded_count = 0;

        let entries = std::fs::read_dir(dir_path)
            .map_err(|e| format!("Failed to read directory {}: {}", directory, e))?;

        for entry in entries {
            let Ok(entry) = entry else { continue };
            let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if !file_name.ends_with(".png") || !entry.path().is_file() {
                continue;
            }

            let png_path = entry.path();

            // Skip PNGs that have no sidecar — they are saved Screenshots, not Templates.
            if !TemplateSidecar::has_sidecar(&png_path) {
                continue;
            }

            let sidecar = match TemplateSidecar::load_for(&png_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("⚠️ Failed to load sidecar for {}: {}", file_name, e);
                    continue;
                }
            };

            let file_path = png_path.to_string_lossy().to_string();
            let search_region = region_manager.resolve_region(&file_name);

            for target in &sidecar.match_targets {
                match Template::from_match_target(file_path.clone(), search_region.clone(), target)
                {
                    Ok(template) if template.is_valid() => {
                        self.templates.push(template);
                        loaded_count += 1;
                    }
                    Ok(_) => {
                        eprintln!(
                            "⚠️ Invalid template skipped: {} / {}",
                            file_name, target.name
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "⚠️ Failed to load template {} / {}: {}",
                            file_name, target.name, e
                        );
                    }
                }
            }
        }

        // Sort templates by category and match_target_name for consistent processing
        self.templates.sort_by(|a, b| {
            a.category
                .partial_cmp(&b.category)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.match_target_name.cmp(&b.match_target_name))
        });

        Ok(loaded_count)
    }

    /// Get all loaded templates
    pub fn get_templates(&self) -> &[Template] {
        &self.templates
    }

    /// Get templates by category
    pub fn get_templates_by_category(&self, category: TemplateCategory) -> Vec<&Template> {
        self.templates
            .iter()
            .filter(|t| t.category == category)
            .collect()
    }

    /// Get template by its MatchTarget name
    pub fn get_template_by_target_name(&self, name: &str) -> Option<&Template> {
        self.templates
            .iter()
            .find(|t| t.match_target_name == name)
    }

    /// Clear all loaded templates
    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// Get template count
    pub fn count(&self) -> usize {
        self.templates.len()
    }

    /// Rescan and reload templates
    pub fn reload_templates(&mut self, directory: &str) -> Result<usize, String> {
        self.clear();
        self.load_templates_from_directory(directory)
    }
}


