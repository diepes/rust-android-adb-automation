# Template Matching Examples Guide

## Overview

Two examples demonstrate the complete template matching workflow for Android game automation:

### 1. `extract_patch.rs` - Patch Extraction Tool

Extracts labeled image regions from full screenshots and generates template patches.

### 2. `match-patch-regions.rs` - Template Matching Validation

Loads patches and searches for matches in images with optimized correlation matching.

---

## Example 1: Extract Patch Regions

### Purpose

Convert full screenshots with embedded region annotations into small template patches.

### File Format

**Input**: `img-<label>-[x,y,width,height].png`

- `label` - Optional identifier (e.g., "wave", "button")
- `[x,y,width,height]` - Region coordinates

**Output**: `patch-<label>-[x,y,width,height].png`

- Same label as input
- Contains only the cropped region

### Usage


```bash
# Run the extractor
cargo run --example extract_patch

# Or with release optimizations (faster)
cargo run --release --example extract_patch
```

### Output Example

```
🔍 Extract Patch Regions
======================================================================

📦 Loading patches...
  ✓ [1] patch-wave-[548,1342,520,160].png (520x160, 25.45ms)
  ✓ [2] patch-5claim-[22,1176,243,144].png (243x144, 20.44ms)
  ✓ [3] patch-retry-[79,1530,436,137].png (436x137, 33.57ms)
  ✅ Loaded 3 patches in 344.22ms total

Processing img-*.png files...

Processing: img-wave-[548,1342,520,160].png
  Extracted label: wave
  Extracted region: x=548, y=1342, width=520, height=160
  Source dimensions: 1080x2280 (loaded in 613.84ms)
  ✓ Saved patch to: patch-wave-[548,1342,520,160].png (crop: 10.79ms, save: 89.10ms, total: 714.00ms)

============================================================
📊 PROCESSING SUMMARY
============================================================
✓ Patches generated: 3
✓ Patches skipped:  0
🗑️  Old patches removed: 3
------------------------------------------------------------
⏱️  Cleanup time:     1.72ms
⏱️  Processing time:  1599.20ms
⏱️  Total time:       1600.97ms (1.601s)
============================================================
```

### Functions

#### `generate_patch_filename(label: Option<&str>, x, y, width, height) -> String`

Creates standardized patch filename with optional label.


```rust
// With label
generate_patch_filename(Some("button"), 100, 200, 50, 50)
// Returns: "patch-button-[100,200,50,50].png"

// Without label
generate_patch_filename(None, 100, 200, 50, 50)
// Returns: "patch-[100,200,50,50].png"
```

#### `generate_output_path(source_path, label, x, y, width, height) -> String`

Generates full output path in same directory as source.


```rust
generate_output_path("assets/test_images/img-wave-[548,1342,520,160].png", 
                    Some("wave"), 548, 1342, 520, 160)
// Returns: "assets/test_images/patch-wave-[548,1342,520,160].png"
```

#### `extract_region_and_label_from_filename(path) -> Option<(Option<String>, x, y, width, height)>`

Parses coordinates and label from filename.


```rust
extract_region_and_label_from_filename("img-button-[100,200,50,75].png")
// Returns: Some((Some("button"), 100, 200, 50, 75))

extract_region_and_label_from_filename("img-[100,200,50,75].png")
// Returns: Some((None, 100, 200, 50, 75))
```

### Tests

```bash
cargo test --example extract_patch
```

Covers:

- ✅ Filename generation with/without labels
- ✅ Output path construction
- ✅ Region coordinate parsing with spaces
- ✅ Full paths and relative paths

---

## Example 2: Match Patch Regions

### Purpose

Validate template matching by searching for patches in full screenshots with performance metrics.

### Features

- ✅ Optimized correlation calculation with early exit
- ✅ Localized search around expected patch location
- ✅ Progress reporting during search
- ✅ Configurable correlation threshold
- ✅ Per-operation timing details

### Usage


```bash
# Run template matching (debug mode - slower)
cargo run --example match-patch-regions

# Or release mode (5x faster)
cargo run --release --example match-patch-regions
```

### Configuration

Edit `main()` to adjust:

```rust
let threshold = 0.85;          // Correlation threshold (0-1)
let max_matches_per_patch = 5; // Max results per patch
let search_margin = 10u32;     // Search ±10 pixels from expected position
```

### Output Example

```
🔍 Template Matching Example
======================================================================

📦 Loading patches...
  ✓ [1] patch-wave-[548,1342,520,160].png (520x160, orig: (548,1342), 31.19ms)
  ✓ [2] patch-5claim-[22,1176,243,144].png (243x144, orig: (22,1176), 20.65ms)
  ✓ [3] patch-retry-[79,1530,436,137].png (436x137, orig: (79,1530), 16.19ms)
  ✅ Loaded 3 patches in 283.06ms total

🔎 Matching patches against images...

  📷 [1/] Image: img-wave-[548,1342,520,160].png (1080x2280, loaded 458.26ms)
      🔍 Patch 1/3 'wave' - searching region x:[498,1118] y:[1292,1552] ...
        ⏳ Search progress: 100%
      ✓ Patch 1/'wave': found 5 matches in 3627.48ms
        [1] Position: (548, 1342) - Correlation: 99.87%
        [2] Position: (547, 1342) - Correlation: 98.76%
        [3] Position: (549, 1342) - Correlation: 98.76%
        [4] Position: (548, 1341) - Correlation: 98.45%
        [5] Position: (548, 1343) - Correlation: 98.45%

      🔍 Patch 2/3 '5claim' - searching region x:[12,275] y:[1166,1330] ...
        ⏳ Search progress: 100%
      ✗ Patch 2/'5claim' - No matches above 85% (4005.20ms)

      🔍 Patch 3/3 'retry' - searching region x:[69,525] y:[1520,1677] ...
        ⏳ Search progress: 100%
      ✓ Patch 3/'retry': found 5 matches in 10229.01ms
        [1] Position: (97, 1676) - Correlation: 87.83%

    ⏱️  Image processing time: 17628.22ms

======================================================================
📊 MATCHING SUMMARY
======================================================================
✓ Patches loaded:        3
✓ Images loaded:         3
✓ Total comparisons:     9
✓ Matches found:         35
  Threshold:             85%
------------------------------------------------------------
⏱️  Load time:             4.41ms
⏱️  Matching time:         52608.04ms (52.6s)
⏱️  Total time:            52611.18ms (52.611s)
⏱️  Avg time per comparison: 5845.34ms
======================================================================
```

### Performance Analysis

#### Patch Size Impact

| Patch Size | Time per Comparison | Status |
|------------|-------------------|--------|
| 94×94 | ~0.3-0.5s | ⚡ Fast |
| 243×144 | ~3-5s | ✅ Acceptable |
| 436×137 | ~3-5s | ✅ Acceptable |
| 520×160 | ~3-10s | ✅ Acceptable |

#### Optimization Techniques


1. **Early Exit on Low Correlation**
   - Exits pixel comparison early if threshold cannot be met
   - Saves ~70-80% computation for non-matching patches

2. **Localized Search**
   - Searches only ±10-50 pixels from expected position
   - Avoids scanning entire image (1080×2280)

3. **Progress Reporting**
   - Shows search progress every 5% completion
   - Allows user to monitor long-running searches

### Functions

#### `calculate_correlation(patch, region, min_match) -> f32`

Computes normalized correlation with early exit.


```rust
// Perfect match
let corr = calculate_correlation(&patch, &exact_region, 0.9);
// Returns: 0.99+

// Non-matching region
let corr = calculate_correlation(&patch, &different_region, 0.9);
// Returns: 0.0 (early exit triggered)
```

**Algorithm**:

- Computes sum of squared pixel differences
- Periodically checks if threshold can still be met
- Returns 0.0 immediately if threshold is impossible

#### `find_matches(image, patch, threshold, max_matches, expected_x, expected_y, search_margin) -> Vec<(x, y, correlation)>`

Finds best matches in image around expected location.


```rust
let matches = find_matches(
    &screenshot,
    &patch_img,
    0.85,  // Threshold
    5,     // Max matches
    Some(548), Some(1342),  // Expected position
    10  // Search ±10 pixels
);
// Returns: Vec with top 5 matches sorted by correlation
```

**Search Strategy**:

- If expected_x/y provided: searches localized region
- If None: searches entire image
- Returns matches sorted by correlation (highest first)

### Tests

```bash
cargo test --example match-patch-regions
```

Covers:

- ✅ Filename parsing from patches
- ✅ Perfect match correlation (100%)
- ✅ Different image detection (<10%)
- ✅ Size mismatch handling (0%)

---

## Integration with FSM Automation

### Typical Workflow


1. **Capture Screenshots**
   ```
   Screenshot saved → img-gamestate-[100,200,300,400].png
   ```

2. **Extract Patches**
   ```bash
   cargo run --example extract_patch
   ```
   Generates: `patch-gamestate-[100,200,300,400].png`

3. **Load in FSM**
   - Store patch coordinates from filename
   - Use during automation for fast matching

4. **Match in Runtime**
   ```rust
   // During game automation
   let matches = find_matches(current_screenshot, &patch, 0.90,
       1, Some(expected_x), Some(expected_y), 5);
   
   if !matches.is_empty() && matches[0].2 >= 0.90 {
       // Action triggered - correlation is high
       perform_action();
   }
   ```

### Performance Tips

| Optimization | Impact | Notes |
|--------------|--------|-------|
| Cache patches | 500ms/image | Load once, reuse |
| Strict threshold | 70% faster | Use 85-95% |
| Known location | 5x faster | Store previous position |
| Small search margin | 2x faster | Use ±5-10 pixels |
| Release build | 3x faster | Always use for automation |

---

## Common Issues & Solutions

### Issue: Slow Matching (>10s per image)

**Solutions**:

- ✅ Increase correlation threshold to 90%+
- ✅ Reduce search margin to ±5 pixels
- ✅ Use release build: `cargo run --release`
- ✅ Use smaller patches if possible

### Issue: False Negatives (no matches found)

**Solutions**:

- ✅ Decrease correlation threshold to 75-80%
- ✅ Increase search margin to ±20-50 pixels
- ✅ Verify expected position is accurate
- ✅ Check patch file exists and is readable

### Issue: False Positives (unwanted matches)

**Solutions**:

- ✅ Increase correlation threshold to 95%+
- ✅ Use unique/distinctive patches
- ✅ Verify patches don't contain duplicate UI elements

---

## Files Reference

```
examples/
├── extract_patch.rs           # Patch extraction tool
└── match-patch-regions.rs     # Template matching validator

assets/test_images/
├── img-wave-[548,1342,520,160].png         # Input screenshot
├── patch-wave-[548,1342,520,160].png       # Generated patch
├── img-5claim-[22,1176,243,144].png
├── patch-5claim-[22,1176,243,144].png
└── ... (more examples)
```

---

## Related Documentation

- [Template Matching Performance Comparison](./TEMPLATE_MATCHING_COMPARISON.md)
- [FSM State Machine Guide](./FSM_GUIDE.md) (when available)
- [Image Recognition Setup](./IMAGE_RECOGNITION.md)
