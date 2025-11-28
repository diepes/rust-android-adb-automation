//! Search region management for targeted image matching

#[derive(Debug, Clone, PartialEq)]
pub struct SearchRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub name: String,
}

impl SearchRegion {
    pub fn new(x: u32, y: u32, width: u32, height: u32, name: String) -> Self {
        Self {
            x,
            y,
            width,
            height,
            name,
        }
    }

    /// Create a full-screen region
    pub fn full_screen(screen_width: u32, screen_height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width: screen_width,
            height: screen_height,
            name: "full_screen".to_string(),
        }
    }

    /// Parse region from filename format: template-[x,y,width,height].png
    pub fn parse_from_filename(filename: &str, screen_width: u32, screen_height: u32) -> Self {
        if let Some(region_str) = Self::extract_region_string(filename)
            && let Some(region) = Self::parse_region_coordinates(&region_str)
        {
            // Validate and clip to screen bounds
            let clipped = Self::clip_to_screen(region, screen_width, screen_height);
            return clipped;
        }

        // Default to full screen if parsing fails
        Self::full_screen(screen_width, screen_height)
    }

    /// Extract region string from filename (e.g., "[300,1682,50,50]")
    fn extract_region_string(filename: &str) -> Option<String> {
        if let Some(start) = filename.find('[')
            && let Some(end) = filename.find(']')
            && end > start
        {
            return Some(filename[start + 1..end].to_string());
        }
        None
    }

    /// Parse coordinates from region string (e.g., "300,1682,50,50")
    fn parse_region_coordinates(region_str: &str) -> Option<SearchRegion> {
        let parts: Vec<&str> = region_str.split(',').collect();
        if parts.len() == 4
            && let (Ok(x), Ok(y), Ok(width), Ok(height)) = (
                parts[0].trim().parse::<u32>(),
                parts[1].trim().parse::<u32>(),
                parts[2].trim().parse::<u32>(),
                parts[3].trim().parse::<u32>(),
            )
        {
            return Some(SearchRegion::new(
                x,
                y,
                width,
                height,
                format!("parsed_{}_{}_{}_{}", x, y, width, height),
            ));
        }
        None
    }

    /// Clip region to screen boundaries
    fn clip_to_screen(
        mut region: SearchRegion,
        screen_width: u32,
        screen_height: u32,
    ) -> SearchRegion {
        // Ensure region doesn't exceed screen bounds
        region.x = region.x.min(screen_width.saturating_sub(1));
        region.y = region.y.min(screen_height.saturating_sub(1));

        // Adjust width and height to fit within screen
        region.width = region.width.min(screen_width.saturating_sub(region.x));
        region.height = region.height.min(screen_height.saturating_sub(region.y));

        region
    }

    /// Check if this region contains a point
    pub fn contains_point(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < (self.x + self.width) && y >= self.y && y < (self.y + self.height)
    }

    /// Get the center point of this region
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Check if this region is valid (non-zero dimensions)
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Manager for predefined search regions in Android games
pub struct RegionManager {
    regions: std::collections::HashMap<String, SearchRegion>,
    screen_width: u32,
    screen_height: u32,
}

impl RegionManager {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let mut manager = Self {
            regions: std::collections::HashMap::new(),
            screen_width,
            screen_height,
        };

        // Add common Android game regions
        manager.add_common_regions();
        manager
    }

    /// Add common Android game UI regions
    fn add_common_regions(&mut self) {
        let w = self.screen_width;
        let h = self.screen_height;

        // Status bar and navigation areas
        self.add_region(
            "status_bar",
            SearchRegion::new(0, 0, w, h / 20, "status_bar".to_string()),
        );
        self.add_region(
            "navigation_bar",
            SearchRegion::new(0, h * 19 / 20, w, h / 20, "navigation_bar".to_string()),
        );

        // Common UI areas
        self.add_region(
            "top_left",
            SearchRegion::new(0, 0, w / 4, h / 4, "top_left".to_string()),
        );
        self.add_region(
            "top_right",
            SearchRegion::new(w * 3 / 4, 0, w / 4, h / 4, "top_right".to_string()),
        );
        self.add_region(
            "bottom_left",
            SearchRegion::new(0, h * 3 / 4, w / 4, h / 4, "bottom_left".to_string()),
        );
        self.add_region(
            "bottom_right",
            SearchRegion::new(
                w * 3 / 4,
                h * 3 / 4,
                w / 4,
                h / 4,
                "bottom_right".to_string(),
            ),
        );
        self.add_region(
            "center",
            SearchRegion::new(w / 4, h / 4, w / 2, h / 2, "center".to_string()),
        );

        // Game-specific areas
        self.add_region(
            "hud_area",
            SearchRegion::new(0, 0, w, h / 8, "hud_area".to_string()),
        );
        self.add_region(
            "action_area",
            SearchRegion::new(0, h * 7 / 8, w, h / 8, "action_area".to_string()),
        );
        self.add_region(
            "inventory_area",
            SearchRegion::new(w * 7 / 8, h / 4, w / 8, h / 2, "inventory_area".to_string()),
        );
    }

    pub fn add_region(&mut self, name: &str, region: SearchRegion) {
        self.regions.insert(name.to_string(), region);
    }

    pub fn get_region(&self, name: &str) -> Option<&SearchRegion> {
        self.regions.get(name)
    }

    pub fn get_region_names(&self) -> Vec<String> {
        self.regions.keys().cloned().collect()
    }

    /// Create region from filename or return predefined region
    pub fn resolve_region(&self, identifier: &str) -> SearchRegion {
        // First try to get predefined region
        if let Some(region) = self.get_region(identifier) {
            return region.clone();
        }

        // Try to parse from filename format
        SearchRegion::parse_from_filename(identifier, self.screen_width, self.screen_height)
    }
}
