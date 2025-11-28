//! Template management and matching functionality

use super::region::SearchRegion;
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
    pub name: String,
    pub search_region: SearchRegion,
    pub width: u32,
    pub height: u32,
    pub category: TemplateCategory,
}

impl Template {
    pub fn new(path: String, search_region: SearchRegion) -> Result<Self, String> {
        // Load image to get dimensions
        let image =
            image::open(&path).map_err(|e| format!("Failed to load template {}: {}", path, e))?;

        let name = Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let category = Self::determine_category(&name);

        // Calculate actual template dimensions (cropped if region is specified in filename)
        let (width, height) = Self::calculate_template_dimensions(&name, &image)?;

        Ok(Self {
            path,
            name,
            search_region,
            width,
            height,
            category,
        })
    }

    /// Calculate the actual template dimensions after cropping (if applicable)
    fn calculate_template_dimensions(
        filename: &str,
        image: &image::DynamicImage,
    ) -> Result<(u32, u32), String> {
        // Check if filename contains region coordinates [x,y,width,height]
        if let Some(region_coords) = Self::extract_region_from_filename(filename) {
            let (crop_x, crop_y, crop_w, crop_h) = region_coords;

            // Validate crop region bounds
            if crop_x + crop_w > image.width() || crop_y + crop_h > image.height() {
                return Err(format!(
                    "Template crop region [{},{},{},{}] exceeds image bounds ({}x{})",
                    crop_x,
                    crop_y,
                    crop_w,
                    crop_h,
                    image.width(),
                    image.height()
                ));
            }

            // Return cropped dimensions
            Ok((crop_w, crop_h))
        } else {
            // No region specified, use full image dimensions
            Ok((image.width(), image.height()))
        }
    }

    /// Extract region coordinates from filename
    fn extract_region_from_filename(filename: &str) -> Option<(u32, u32, u32, u32)> {
        if let Some(start) = filename.find('[')
            && let Some(end) = filename.find(']')
            && end > start
        {
            let region_str = &filename[start + 1..end];
            let parts: Vec<&str> = region_str.split(',').collect();
            if parts.len() == 4
                && let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
                    parts[0].trim().parse::<u32>(),
                    parts[1].trim().parse::<u32>(),
                    parts[2].trim().parse::<u32>(),
                    parts[3].trim().parse::<u32>(),
                )
            {
                return Some((x, y, width, height));
            }
        }
        None
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

    /// Scan directory for PNG template files and load them
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
            if let Ok(entry) = entry
                && let Some(file_name) = entry.file_name().to_str()
                && file_name.ends_with(".png")
                && entry.path().is_file()
            {
                let file_path = entry.path().to_string_lossy().to_string();

                // Determine search region from filename or use full screen
                let search_region = region_manager.resolve_region(file_name);

                match Template::new(file_path, search_region) {
                    Ok(template) => {
                        if template.is_valid() {
                            self.templates.push(template);
                            loaded_count += 1;
                        } else {
                            eprintln!("⚠️ Invalid template skipped: {}", file_name);
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ Failed to load template {}: {}", file_name, e);
                    }
                }
            }
        }

        // Sort templates by category and name for consistent processing
        self.templates.sort_by(|a, b| {
            a.category
                .partial_cmp(&b.category)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name.cmp(&b.name))
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

    /// Get template by name
    pub fn get_template_by_name(&self, name: &str) -> Option<&Template> {
        self.templates.iter().find(|t| t.name == name)
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
