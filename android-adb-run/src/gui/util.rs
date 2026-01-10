// gui/util.rs
// Utility helpers for GUI

pub fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, &byte) in chunk.iter().enumerate() {
            buf[i] = byte;
        }
        let b = ((buf[0] as u32) << 16) | ((buf[1] as u32) << 8) | (buf[2] as u32);
        result.push(CHARS[((b >> 18) & 63) as usize] as char);
        result.push(CHARS[((b >> 12) & 63) as usize] as char);
        result.push(if chunk.len() > 1 {
            CHARS[((b >> 6) & 63) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            CHARS[(b & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

pub fn calculate_device_coords(
    element_rect: dioxus::html::geometry::ElementPoint,
    screen_x: u32,
    screen_y: u32,
) -> (u32, u32) {
    let max_content_width = 400.0;
    let max_content_height = 600.0;
    let border_px = 8.0;

    let image_aspect = screen_x as f32 / screen_y as f32;
    let container_aspect = max_content_width / max_content_height;
    let (content_w, content_h) = if image_aspect > container_aspect {
        (max_content_width, max_content_width / image_aspect)
    } else {
        (max_content_height * image_aspect, max_content_height)
    };
    let displayed_w = content_w.max(1.0);
    let displayed_h = content_h.max(1.0);

    let raw_x = element_rect.x as f32 - border_px;
    let raw_y = element_rect.y as f32 - border_px;

    let clamped_x_in_display = raw_x.max(0.0).min(displayed_w - 1.0);
    let clamped_y_in_display = raw_y.max(0.0).min(displayed_h - 1.0);

    let scale_x = screen_x as f32 / displayed_w;
    let scale_y = screen_y as f32 / displayed_h;
    let device_x = (clamped_x_in_display * scale_x) as u32;
    let device_y = (clamped_y_in_display * scale_y) as u32;

    (device_x.min(screen_x - 1), device_y.min(screen_y - 1))
}
