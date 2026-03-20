use crate::template_matching::{PatchInfo, TemplateMatcher};
use dioxus::prelude::*;
use image::{ImageReader, RgbImage};
use std::io::Cursor;

fn add_history_message(
    history_signal: &mut Signal<Vec<(String, bool)>>,
    message: String,
    is_result: bool,
    patch_name: Option<&str>,
    is_first_result: bool,
) {
    let mut history = {
        let history_ref = history_signal.read();
        history_ref.clone()
    };

    if is_result {
        if is_first_result {
            history.retain(|(_, is_res)| *is_res);
        } else if let Some(patch) = patch_name {
            history.retain(|(msg, is_res)| {
                *is_res
                    || (!msg.contains("⏳ Still matching") && !msg.contains("🔎 Checking"))
                    || !msg.contains(patch)
            });
        }
    }

    history.push((message, is_result));

    if history.len() > 15 {
        let excess = history.len() - 15;
        history.drain(0..excess);
    }

    history_signal.set(history);
}

pub fn start_template_matching_phase(
    bytes: Vec<u8>,
    rgb_image: Option<RgbImage>,
    screenshot_counter: u32,
    mut status_signal: Signal<String>,
    mut status_history_signal: Signal<Vec<(String, bool)>>,
) {
    spawn(async move {
        log::info!("🚀 PHASE 3 STARTING - Template matching");

        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, bool)>(500);

        let init_msg = format!("[#{}] 🔍 Loading patches...", screenshot_counter);
        log::info!("📝 Setting initial status: {}", init_msg);
        status_signal.set(init_msg.clone());
        add_history_message(&mut status_history_signal, init_msg, false, None, false);

        log::info!("🧵 Spawning blocking task for match_patches_blocking_with_progress");
        let mut result_handle = tokio::task::spawn_blocking(move || {
            log::info!("🔧 Inside spawn_blocking - calling match_patches_blocking_with_progress");
            match_patches_blocking_with_progress(&bytes, rgb_image, screenshot_counter, tx)
        });

        let mut result = None;
        let mut first_result_received = false;
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    if let Some((progress_msg, is_result)) = msg {
                        status_signal.set(progress_msg.clone());

                        let patch_name = if is_result {
                            progress_msg.split(']').nth(1)
                                .and_then(|s| s.split_whitespace().nth(2))
                        } else {
                            None
                        };

                        add_history_message(
                            &mut status_history_signal,
                            progress_msg.clone(),
                            is_result,
                            patch_name,
                            is_result && !first_result_received,
                        );

                        if is_result {
                            first_result_received = true;
                        }
                    } else {
                        break;
                    }
                }
                res = &mut result_handle => {
                    result = Some(res);
                    while let Ok((progress_msg, is_result)) = rx.try_recv() {
                        status_signal.set(progress_msg.clone());

                        let patch_name = if is_result {
                            progress_msg.split(']').nth(1)
                                .and_then(|s| s.split_whitespace().nth(2))
                        } else {
                            None
                        };

                        add_history_message(
                            &mut status_history_signal,
                            progress_msg.clone(),
                            is_result,
                            patch_name,
                            is_result && !first_result_received,
                        );

                        if is_result {
                            first_result_received = true;
                        }
                    }
                    break;
                }
            }
        }

        let result = result.unwrap_or(Ok(None));
        log::info!("✅ PHASE 3 Complete - Final result: {:?}", result);
    });
}

fn match_patches_blocking_with_progress(
    screenshot_bytes: &[u8],
    image_rgb: Option<RgbImage>,
    screenshot_counter: u32,
    tx: tokio::sync::mpsc::Sender<(String, bool)>,
) -> Option<String> {
    let image_rgb = match image_rgb {
        Some(img) => img,
        None => match decode_screenshot_to_rgb(screenshot_bytes) {
            Ok(img) => img,
            Err(_) => return None,
        },
    };

    let patch_dir = std::path::Path::new("assets/test_images");

    if !patch_dir.exists() {
        log::debug!("Patch directory not found: {:?}", patch_dir);
        let _ = tx.blocking_send((
            format!("[#{}] ⚠️ Patch directory not found", screenshot_counter),
            false,
        ));
        return None;
    }

    let mut matcher = TemplateMatcher::new();
    let mut patch_count = 0;

    let _ = tx.blocking_send((
        format!("[#{}] 🔍 Scanning patches...", screenshot_counter),
        false,
    ));

    match std::fs::read_dir(patch_dir) {
        Ok(entries) => {
            let entries_vec: Vec<_> = entries.flatten().collect();

            for entry in &entries_vec {
                let path = entry.path();
                let filename = match path.file_name() {
                    Some(name) => match name.to_str() {
                        Some(s) => s.to_string(),
                        None => continue,
                    },
                    None => continue,
                };

                if !filename.starts_with("patch-") || !filename.ends_with(".png") {
                    continue;
                }

                if let Some((label, x, y, width, height)) = parse_patch_filename(&filename) {
                    match std::fs::read(&path) {
                        Ok(pixel_data) => match decode_screenshot_to_rgb(&pixel_data) {
                            Ok(img) => {
                                let pixels = img.into_raw();
                                let patch = PatchInfo::new(label, x, y, width, height, pixels);
                                matcher.add_patch(patch);
                                patch_count += 1;
                            }
                            Err(_) => continue,
                        },
                        Err(_) => continue,
                    }
                }
            }

            if patch_count > 0 {
                let msg = format!(
                    "[#{}] 📦 Loaded {} patches to match...",
                    screenshot_counter, patch_count
                );
                let _ = tx.blocking_send((msg, false));
            }
        }
        Err(_) => {
            let _ = tx.blocking_send((
                format!("[#{}] ⚠️ Failed to load patches", screenshot_counter),
                false,
            ));
            return None;
        }
    }

    if patch_count == 0 {
        let _ = tx.blocking_send((
            format!("[#{}] ⚠️ No patches found", screenshot_counter),
            false,
        ));
        return None;
    }

    if let Err(e) = tx.blocking_send((
        format!(
            "[#{}] 🔎 Matching {} patches...",
            screenshot_counter, patch_count
        ),
        false,
    )) {
        log::error!("❌ Failed to send 'Matching patches' message: {}", e);
    }

    let threshold = 0.85;
    let mut best_match: Option<(String, f32)> = None;

    for (idx, patch) in matcher.patches().iter().enumerate() {
        let patch_name = patch.display_name();

        let progress_pct = ((idx + 1) as f32 / matcher.patches().len() as f32 * 100.0) as u32;
        let msg = format!(
            "[#{}] 🔎 Checking {}... ({}%)",
            screenshot_counter, patch_name, progress_pct
        );
        let _ = tx.blocking_send((msg, false));

        let start = std::time::Instant::now();
        let total_patches = matcher.patches().len();
        let patches_completed = idx;

        let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();

        let tx_clone = tx.clone();
        let patch_name_clone = patch_name.clone();
        let screenshot_counter_clone = screenshot_counter;
        let progress_handle = std::thread::spawn(move || {
            let mut counter = 0;
            loop {
                std::thread::sleep(std::time::Duration::from_secs(30));

                if stop_flag_clone.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                counter += 1;
                let elapsed_secs = counter * 30;

                let estimated_secs_per_patch = if patches_completed > 0 {
                    elapsed_secs / patches_completed as u32
                } else {
                    60
                };

                let remaining_patches = (total_patches - patches_completed - 1) as u32;
                let estimated_remaining = estimated_secs_per_patch * remaining_patches;

                let msg = if estimated_remaining > 0 {
                    format!(
                        "[#{}] ⏳ Still matching {}... ({} sec, ~{} min remaining)",
                        screenshot_counter_clone,
                        patch_name_clone,
                        elapsed_secs,
                        estimated_remaining / 60
                    )
                } else {
                    format!(
                        "[#{}] ⏳ Still matching {}... ({} sec)",
                        screenshot_counter_clone, patch_name_clone, elapsed_secs
                    )
                };

                if tx_clone.blocking_send((msg, false)).is_err() {
                    break;
                }
            }
        });

        let matches = matcher.find_matches(&image_rgb, idx, threshold, 1, 50);
        let elapsed = start.elapsed();

        stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);

        let completion_msg = if let Some(m) = matches.first() {
            let accuracy_pct = (m.correlation * 100.0) as u32;
            format!(
                "[#{}] ✓ Matched {} in {:.0}s ({}%)",
                screenshot_counter,
                patch_name,
                elapsed.as_secs_f32(),
                accuracy_pct
            )
        } else {
            format!(
                "[#{}] ✗ No match for {} ({:.0}s)",
                screenshot_counter,
                patch_name,
                elapsed.as_secs_f32()
            )
        };
        let _ = tx.blocking_send((completion_msg, true));

        let _ = progress_handle.join();

        if let Some(m) = matches.first() {
            let patch_name = patch.display_name();
            if best_match.is_none() || m.correlation > best_match.as_ref().unwrap().1 {
                best_match = Some((patch_name, m.correlation));
            }
        }
    }

    match &best_match {
        Some((name, correlation)) => {
            let accuracy_pct = (correlation * 100.0) as u32;
            if let Err(e) = tx.blocking_send((
                format!(
                    "[#{}] 🎯 BEST MATCH: {} ({}%)",
                    screenshot_counter, name, accuracy_pct
                ),
                true,
            )) {
                log::error!("❌ Failed to send final match message: {}", e);
            }
        }
        None => {
            if let Err(e) = tx.blocking_send((
                format!("[#{}] ⚠️ No matches found in any patch", screenshot_counter),
                true,
            )) {
                log::error!("❌ Failed to send no-match message: {}", e);
            }
        }
    }

    best_match.map(|(name, _)| name)
}

pub fn decode_screenshot_to_rgb(bytes: &[u8]) -> Result<RgbImage, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Failed to guess format: {}", e))?;

    let image = reader
        .decode()
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let rgb = image.to_rgb8();
    Ok(rgb)
}

fn parse_patch_filename(filename: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    let name = filename.strip_suffix(".png")?;

    if !name.starts_with("patch-") {
        return None;
    }
    let name = &name[6..];

    let bracket_pos = name.rfind('[')?;
    let label_part = &name[..bracket_pos];
    let coords_part = &name[bracket_pos..];

    if !coords_part.starts_with('[') || !coords_part.ends_with(']') {
        return None;
    }

    let coords_str = &coords_part[1..coords_part.len() - 1];
    let parts: Vec<&str> = coords_str.split(',').collect();
    if parts.len() != 4 {
        return None;
    }

    let x = parts[0].trim().parse::<u32>().ok()?;
    let y = parts[1].trim().parse::<u32>().ok()?;
    let width = parts[2].trim().parse::<u32>().ok()?;
    let height = parts[3].trim().parse::<u32>().ok()?;

    let label = if label_part.is_empty() {
        None
    } else {
        let label_str = if let Some(stripped) = label_part.strip_suffix('-') {
            stripped
        } else {
            label_part
        };
        Some(label_str.to_string())
    };

    Some((label, x, y, width, height))
}
