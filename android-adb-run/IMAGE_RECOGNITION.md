# Image Recognition Feature

## Overview

The game automation system now includes image recognition capabilities that can detect patterns in Android device screenshots and automatically perform tap actions when matches are found.

## How It Works

### 1. Template Matching

- Uses **normalized cross-correlation** algorithm from the `imageproc` crate
- Converts both screenshot and template to grayscale for processing
- Returns confidence scores from 0.0 to 1.0 for match quality

### 2. Configuration

The system uses an `ImageRecognitionConfig` with these parameters:

- **Template Path**: Path to the PNG template image (e.g., "img-[300,1682,50,50].png")
- **Confidence Threshold**: Minimum match confidence (0.0-1.0, default: 0.8)
- **Template Dimensions**: Width and height of the template in pixels

### 3. Automated Actions

When a template match is found above the threshold:

1. Calculates tap coordinates at the center of the matched region
2. Validates coordinates are within screen bounds
3. Executes an ADB tap command at those coordinates

## Usage

### Starting Image Recognition

1. Launch the application: `cargo run -- --debug`
2. Connect to an Android device
3. Switch to "Auto" mode in the GUI
4. Click "Start" to begin automated screenshot analysis

### Debug Output

With `--debug` flag, you'll see detailed logging:

```BASH
🔍 Analyzing screenshot for template matches...
🎯 Template found at (305, 1687) with confidence 0.923
✅ Tap executed at (330, 1712)
```

### Customizing Recognition

You can update the configuration programmatically:

```rust
game_automation.update_image_config(
    "my_template.png".to_string(),
    0.85,  // 85% confidence threshold
    60,    // 60px width
    40     // 40px height
);
```

## Template Image Requirements

### Format

- **File Format**: PNG (recommended) or JPEG
- **Color**: Any (converted to grayscale internally)
- **Size**: Should be reasonably sized (20x20 to 200x200 pixels typically work best)

### Quality Guidelines


- Use clear, distinctive patterns
- Avoid templates that are too generic (might match multiple areas)
- Ensure template is representative of what appears on screen
- Consider different device resolutions if targeting multiple devices

### Current Template

The included `img-[300,1682,50,50].png` is a 50x50 pixel template extracted from coordinates (300, 1682).

## Performance Considerations

### Speed Optimizations

- Screenshots use framebuffer API (3-5x faster than `screencap -p`)
- Base64 encoding runs on background threads
- Template matching is single-threaded but efficient

### Timing Behavior

- **Match Found**: Wait 1000ms before next screenshot (allows UI to update)
- **No Match**: Wait 500ms before next screenshot (faster scanning)
- **Error**: Wait 500ms before retry

### Memory Usage

- Screenshots stored temporarily as raw PNG bytes
- Template images loaded once at startup
- Grayscale conversion creates temporary buffers

## Error Handling

### Common Issues

1. **Template Not Found**: Check file path and permissions
2. **No Screenshot Available**: Ensure ADB connection is working
3. **Coordinates Out of Bounds**: Template found near screen edges
4. **Low Confidence**: Template doesn't match well, consider adjusting threshold

### Troubleshooting

- Use `--debug` flag for detailed logging
- Verify template image exists and is readable
- Check ADB connection status
- Monitor confidence scores to tune threshold

## Future Enhancements

### Potential Improvements

- Multiple template support (detect different UI elements)
- Region-of-interest scanning (only check specific screen areas)
- Template rotation/scaling tolerance
- Machine learning-based pattern recognition
- Configuration file support for multiple game profiles

### Integration Ideas

- Save/load template configurations
- GUI controls for threshold adjustment
- Real-time confidence visualization
- Template recording from live screenshots
