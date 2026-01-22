/// Template matching data types
/// Information about a single patch
#[derive(Clone, Debug)]
pub struct PatchInfo {
    /// Patch label/name (e.g., "wave", "button")
    pub label: Option<String>,
    /// Original X coordinate in source image
    pub orig_x: u32,
    /// Original Y coordinate in source image
    pub orig_y: u32,
    /// Width of patch
    pub width: u32,
    /// Height of patch
    pub height: u32,
    /// Raw pixel data (RGB format)
    pub pixels: Vec<u8>,
}

/// A single match result
#[derive(Clone, Debug)]
pub struct Match {
    /// X coordinate in the search image
    pub x: u32,
    /// Y coordinate in the search image
    pub y: u32,
    /// Correlation score (0.0-1.0)
    pub correlation: f32,
}

impl PatchInfo {
    /// Create a new patch from filename and pixel data
    pub fn new(
        label: Option<String>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            label,
            orig_x: x,
            orig_y: y,
            width,
            height,
            pixels,
        }
    }

    /// Get the patch name for display
    pub fn display_name(&self) -> String {
        match &self.label {
            Some(label) => format!(
                "patch-{}-[{},{},{},{}]",
                label, self.orig_x, self.orig_y, self.width, self.height
            ),
            None => format!(
                "patch-[{},{},{},{}]",
                self.orig_x, self.orig_y, self.width, self.height
            ),
        }
    }
}

impl Match {
    /// Format match as string with correlation percentage
    pub fn to_string(&self, patch: &PatchInfo) -> String {
        let patch_name = &patch.label.as_deref().unwrap_or("unnamed");
        let correlation_pct = (self.correlation * 100.0) as u32;
        format!(
            "{} at ({},{}) - {}%",
            patch_name, self.x, self.y, correlation_pct
        )
    }
}
