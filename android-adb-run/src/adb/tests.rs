// Tests for hardware access layer logic
// Focus: Connection flow, tap queue, touch monitoring, event triggering

#[cfg(test)]
mod hardware_access_tests {
    use super::super::types::{TouchActivityState, UsbCommand};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{RwLock, mpsc};

    // ============================================================
    // TOUCH ACTIVITY MONITORING TESTS
    // ============================================================

    #[test]
    fn test_touch_state_initial() {
        let state = TouchActivityState::new(30);

        assert!(!state.is_human_active(), "Should not be active initially");
        assert!(!state.has_activity_expired(), "Nothing to expire initially");
        assert_eq!(state.get_remaining_seconds(), None, "No timeout initially");
    }

    #[test]
    fn test_touch_activity_detection() {
        let mut state = TouchActivityState::new(30);

        // Mark touch activity
        state.mark_touch_activity();

        assert!(state.is_human_active(), "Should detect touch activity");

        let remaining = state.get_remaining_seconds();
        assert!(remaining.is_some(), "Should have remaining time");
        assert!(remaining.unwrap() <= 30, "Remaining should be <= 30s");
    }

    #[test]
    fn test_touch_activity_clear() {
        let mut state = TouchActivityState::new(30);

        state.mark_touch_activity();
        assert!(state.is_human_active(), "Should be active after touch");

        state.clear_touch_activity();
        assert!(!state.is_human_active(), "Should not be active after clear");
        assert_eq!(
            state.get_remaining_seconds(),
            None,
            "No timeout after clear"
        );
    }

    #[tokio::test]
    async fn test_touch_timeout_expiry() {
        let mut state = TouchActivityState::new_with_duration(Duration::from_millis(100));

        state.mark_touch_activity();
        assert!(state.is_human_active(), "Should be active");
        assert!(!state.has_activity_expired(), "Should not be expired yet");

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert!(
            !state.is_human_active(),
            "Should not be active after timeout"
        );
        assert!(state.has_activity_expired(), "Should be expired");
        assert_eq!(state.get_remaining_seconds(), None, "No time remaining");
    }

    #[tokio::test]
    async fn test_touch_activity_refresh() {
        let mut state = TouchActivityState::new_with_duration(Duration::from_millis(100));

        state.mark_touch_activity();

        // Wait half the timeout
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Refresh activity
        state.update_activity();

        // Wait another 75ms (total 125ms from first touch, but only 75ms from refresh)
        tokio::time::sleep(Duration::from_millis(75)).await;

        // Should still be active because we refreshed
        assert!(state.is_human_active(), "Should be active after refresh");
    }

    #[tokio::test]
    async fn test_concurrent_touch_monitoring() {
        let monitor = Arc::new(RwLock::new(TouchActivityState::new(30)));

        let monitor_clone = monitor.clone();
        let reader_task = tokio::spawn(async move {
            for _ in 0..5 {
                let _is_active = monitor_clone.read().await.is_human_active();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        let monitor_clone2 = monitor.clone();
        let writer_task = tokio::spawn(async move {
            for _ in 0..5 {
                monitor_clone2.write().await.mark_touch_activity();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        // Should not deadlock
        let result = tokio::time::timeout(Duration::from_secs(1), async {
            reader_task.await.unwrap();
            writer_task.await.unwrap();
        })
        .await;

        assert!(result.is_ok(), "Should not deadlock with concurrent access");
    }

    // ============================================================
    // TAP QUEUE PROCESSOR TESTS
    // ============================================================

    #[tokio::test]
    async fn test_tap_queue_basic() {
        let (tx, mut rx) = mpsc::channel(10);

        // Send some tap commands
        let (tx1, _rx1) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 100,
            y: 200,
            response_tx: tx1,
        })
        .await
        .unwrap();
        let (tx2, _rx2) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 300,
            y: 400,
            response_tx: tx2,
        })
        .await
        .unwrap();

        // Receive them
        let tap1 = rx.recv().await.unwrap();
        let tap2 = rx.recv().await.unwrap();

        match tap1 {
            UsbCommand::Tap { x, y, .. } => {
                assert_eq!(x, 100);
                assert_eq!(y, 200);
            }
            _ => panic!("Expected Tap command"),
        }

        match tap2 {
            UsbCommand::Tap { x, y, .. } => {
                assert_eq!(x, 300);
                assert_eq!(y, 400);
            }
            _ => panic!("Expected Tap command"),
        }
    }

    #[tokio::test]
    async fn test_tap_queue_ordering() {
        let (tx, mut rx) = mpsc::channel(10);

        // Send commands in order
        for i in 0..5 {
            let (tx_dummy, _rx_dummy) = tokio::sync::oneshot::channel();
            tx.send(UsbCommand::Tap {
                x: i * 10,
                y: i * 20,
                response_tx: tx_dummy,
            })
            .await
            .unwrap();
        }

        // Verify ordering
        for i in 0..5 {
            let cmd = rx.recv().await.unwrap();
            match cmd {
                UsbCommand::Tap { x, y, .. } => {
                    assert_eq!(x, i * 10);
                    assert_eq!(y, i * 20);
                }
                _ => panic!("Expected Tap command"),
            }
        }
    }

    #[tokio::test]
    async fn test_tap_queue_backpressure() {
        let (tx, mut rx) = mpsc::channel(2); // Small buffer

        // Fill the buffer
        let (tx1, _rx1) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 1,
            y: 1,
            response_tx: tx1,
        })
        .await
        .unwrap();
        let (tx2, _rx2) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 2,
            y: 2,
            response_tx: tx2,
        })
        .await
        .unwrap();

        // Try to send one more (should apply backpressure)
        let (tx3, _rx3) = tokio::sync::oneshot::channel();
        let send_future = tx.send(UsbCommand::Tap {
            x: 3,
            y: 3,
            response_tx: tx3,
        });

        // Start receiver task
        let receiver = tokio::spawn(async move {
            let mut count = 0;
            while let Some(_cmd) = rx.recv().await {
                count += 1;
                tokio::time::sleep(Duration::from_millis(10)).await;
                if count >= 3 {
                    break;
                }
            }
            count
        });

        // Send should complete once receiver starts consuming
        let timeout_result = tokio::time::timeout(Duration::from_secs(1), send_future).await;

        assert!(
            timeout_result.is_ok(),
            "Send should complete with backpressure"
        );

        let count = receiver.await.unwrap();
        assert_eq!(count, 3, "Should receive all 3 commands");
    }

    #[tokio::test]
    async fn test_tap_and_swipe_mixed_queue() {
        let (tx, mut rx) = mpsc::channel(10);

        // Send mixed commands
        let (tx1, _rx1) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 100,
            y: 100,
            response_tx: tx1,
        })
        .await
        .unwrap();
        let (tx2, _rx2) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Swipe {
            x1: 100,
            y1: 100,
            x2: 200,
            y2: 200,
            duration: Some(300),
            response_tx: tx2,
        })
        .await
        .unwrap();
        let (tx3, _rx3) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 300,
            y: 300,
            response_tx: tx3,
        })
        .await
        .unwrap();

        // Verify order and types
        match rx.recv().await.unwrap() {
            UsbCommand::Tap { x, y, .. } => {
                assert_eq!(x, 100);
                assert_eq!(y, 100);
            }
            _ => panic!("Expected Tap"),
        }

        match rx.recv().await.unwrap() {
            UsbCommand::Swipe { x1, y1, x2, y2, .. } => {
                assert_eq!(x1, 100);
                assert_eq!(y1, 100);
                assert_eq!(x2, 200);
                assert_eq!(y2, 200);
            }
            _ => panic!("Expected Swipe"),
        }

        match rx.recv().await.unwrap() {
            UsbCommand::Tap { x, y, .. } => {
                assert_eq!(x, 300);
                assert_eq!(y, 300);
            }
            _ => panic!("Expected Tap"),
        }
    }

    #[tokio::test]
    async fn test_tap_queue_processor_shutdown() {
        let (tx, mut rx) = mpsc::channel(10);

        let processor = tokio::spawn(async move {
            let mut processed = 0;
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    UsbCommand::Tap { .. } => processed += 1,
                    UsbCommand::Swipe { .. } => processed += 1,
                    UsbCommand::Screenshot { .. } => {}
                    UsbCommand::CheckTouchEvent { .. } => {}
                }
            }
            processed
        });

        // Send some commands
        let (tx1, _rx1) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 1,
            y: 1,
            response_tx: tx1,
        })
        .await
        .unwrap();
        let (tx2, _rx2) = tokio::sync::oneshot::channel();
        tx.send(UsbCommand::Tap {
            x: 2,
            y: 2,
            response_tx: tx2,
        })
        .await
        .unwrap();

        // Close the sender
        drop(tx);

        // Processor should exit cleanly
        let result = tokio::time::timeout(Duration::from_secs(1), processor).await;
        assert!(result.is_ok(), "Processor should exit when channel closed");

        let processed = result.unwrap().unwrap();
        assert_eq!(processed, 2, "Should have processed 2 commands");
    }

    // ============================================================
    // BOUNDS CHECKING TESTS
    // ============================================================

    #[test]
    fn test_tap_bounds_validation() {
        let screen_x = 1080;
        let screen_y = 2400;

        // Valid taps
        assert!(validate_tap_bounds(0, 0, screen_x, screen_y));
        assert!(validate_tap_bounds(500, 1000, screen_x, screen_y));
        assert!(validate_tap_bounds(screen_x, screen_y, screen_x, screen_y));

        // Invalid taps
        assert!(!validate_tap_bounds(screen_x + 1, 0, screen_x, screen_y));
        assert!(!validate_tap_bounds(0, screen_y + 1, screen_x, screen_y));
        assert!(!validate_tap_bounds(2000, 3000, screen_x, screen_y));
    }

    fn validate_tap_bounds(x: u32, y: u32, screen_x: u32, screen_y: u32) -> bool {
        x <= screen_x && y <= screen_y
    }

    // ============================================================
    // SCREEN SIZE PARSING TESTS
    // ============================================================

    #[test]
    fn test_parse_screen_size() {
        let output = "Physical size: 1080x2400\n";
        let result = parse_wm_size_output(output);
        assert_eq!(result, Some((1080, 2400)));
    }

    #[test]
    fn test_parse_screen_size_with_noise() {
        let output = "Override size: 1080x1920\nPhysical size: 1080x2400\n";
        let result = parse_wm_size_output(output);
        assert_eq!(result, Some((1080, 2400)));
    }

    #[test]
    fn test_parse_screen_size_invalid() {
        assert_eq!(parse_wm_size_output(""), None);
        assert_eq!(parse_wm_size_output("No size info"), None);
        assert_eq!(parse_wm_size_output("Physical size: invalid"), None);
        assert_eq!(parse_wm_size_output("Physical size: 1080"), None);
    }

    fn parse_wm_size_output(output: &str) -> Option<(u32, u32)> {
        for line in output.lines() {
            if let Some(size_str) = line.strip_prefix("Physical size: ") {
                let parts: Vec<&str> = size_str.trim().split('x').collect();
                if parts.len() == 2 {
                    if let (Ok(x), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    // ============================================================
    // TOUCH EVENT LINE DETECTION TESTS
    // ============================================================

    #[test]
    fn test_touch_event_detection() {
        // Valid touch event lines
        assert!(is_touch_event_line(
            "0003 0035 00000123    ABS_MT_POSITION_X"
        ));
        assert!(is_touch_event_line(
            "0003 0036 00000456    ABS_MT_POSITION_Y"
        ));
        assert!(is_touch_event_line(
            "[  12345.678] /dev/input/event2: 0001 014a 00000001    BTN_TOUCH         DOWN"
        ));
        assert!(is_touch_event_line("BTN_TOOL_FINGER DOWN"));
        assert!(is_touch_event_line("ABS_X 100"));
        assert!(is_touch_event_line("ABS_Y 200"));

        // Invalid lines (not touch events)
        assert!(!is_touch_event_line("0001 0072 00000001    KEY_VOLUMEDOWN"));
        assert!(!is_touch_event_line("Random log line"));
        assert!(!is_touch_event_line(""));
    }

    fn is_touch_event_line(line: &str) -> bool {
        line.contains("ABS_MT")
            || line.contains("BTN_TOUCH")
            || line.contains("BTN_TOOL_FINGER")
            || line.contains("ABS_X")
            || line.contains("ABS_Y")
            || (line.contains("0003") && (line.contains("0035") || line.contains("0036")))
    }

    // ============================================================
    // CONNECTION RETRY LOGIC TESTS
    // ============================================================

    #[tokio::test]
    async fn test_connection_retry_success_first_attempt() {
        let mut attempts = 0;
        let max_attempts = 5;

        let result: Result<(), &str> = retry_connection(max_attempts, || {
            attempts += 1;
            Ok(())
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempts, 1, "Should succeed on first attempt");
    }

    #[tokio::test]
    async fn test_connection_retry_success_after_failures() {
        let mut attempts = 0;
        let max_attempts = 5;

        let result: Result<(), &str> = retry_connection(max_attempts, || {
            attempts += 1;
            if attempts < 3 {
                Err("Connection failed")
            } else {
                Ok(())
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempts, 3, "Should succeed on third attempt");
    }

    #[tokio::test]
    async fn test_connection_retry_max_attempts_exceeded() {
        let mut attempts = 0;
        let max_attempts = 3;

        let result: Result<(), &str> = retry_connection(max_attempts, || {
            attempts += 1;
            Err("Always fails")
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts, 3, "Should try exactly max_attempts times");
    }

    async fn retry_connection<F, T, E>(max_attempts: u32, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Result<T, E>,
    {
        for attempt in 1..=max_attempts {
            match operation() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempt == max_attempts {
                        return Err(e);
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        unreachable!()
    }

    // ============================================================
    // INTEGRATION: TOUCH MONITORING WITH TAP QUEUE
    // ============================================================

    #[tokio::test]
    async fn test_touch_blocks_tap_execution() {
        let touch_monitor = Arc::new(RwLock::new(TouchActivityState::new(30)));
        let (tap_tx, mut tap_rx) = mpsc::channel(10);

        // Simulate automation wanting to tap
        let (tx, _rx) = tokio::sync::oneshot::channel();
        tap_tx
            .send(UsbCommand::Tap {
                x: 100,
                y: 100,
                response_tx: tx,
            })
            .await
            .unwrap();

        // Mark human touch activity
        touch_monitor.write().await.mark_touch_activity();

        // Processor should check touch status before executing
        let should_process = !touch_monitor.read().await.is_human_active();

        assert!(
            !should_process,
            "Should NOT process taps when human is touching"
        );

        // Clear touch activity
        touch_monitor.write().await.clear_touch_activity();
        let should_process = !touch_monitor.read().await.is_human_active();

        assert!(
            should_process,
            "Should process taps when human not touching"
        );

        // Verify tap is still in queue
        let cmd = tap_rx.recv().await.unwrap();
        match cmd {
            UsbCommand::Tap { x, y, .. } => {
                assert_eq!(x, 100);
                assert_eq!(y, 100);
            }
            _ => panic!("Expected Tap"),
        }
    }

    #[tokio::test]
    async fn test_tap_queue_concurrent_with_screenshot() {
        // Simulate the pattern where screenshot and tap share a mutex
        let device_lock = Arc::new(tokio::sync::Mutex::new(0u32));

        let lock_clone = device_lock.clone();
        let screenshot_task = tokio::spawn(async move {
            for _ in 0..10 {
                let _guard = lock_clone.lock().await;
                // Simulate screenshot work
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        let lock_clone2 = device_lock.clone();
        let tap_task = tokio::spawn(async move {
            for _ in 0..10 {
                let _guard = lock_clone2.lock().await;
                // Simulate tap work
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        // Should complete without deadlock
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            screenshot_task.await.unwrap();
            tap_task.await.unwrap();
        })
        .await;

        assert!(
            result.is_ok(),
            "Should not deadlock between screenshot and tap operations"
        );
    }

    // ============================================================
    // FRAMEBUFFER FORMAT DETECTION TESTS
    // ============================================================

    #[test]
    fn test_detect_framebuffer_format() {
        // Test RGBA (4 bytes per pixel)
        let rgba_data = vec![0u8; 1080 * 2400 * 4];
        assert_eq!(detect_bytes_per_pixel(1080, 2400, rgba_data.len()), Some(4));

        // Test RGB (3 bytes per pixel)
        let rgb_data = vec![0u8; 1080 * 2400 * 3];
        assert_eq!(detect_bytes_per_pixel(1080, 2400, rgb_data.len()), Some(3));

        // Test RGB565 (2 bytes per pixel)
        let rgb565_data = vec![0u8; 1080 * 2400 * 2];
        assert_eq!(
            detect_bytes_per_pixel(1080, 2400, rgb565_data.len()),
            Some(2)
        );

        // Test invalid size
        let invalid_data = vec![0u8; 1000];
        assert_eq!(detect_bytes_per_pixel(1080, 2400, invalid_data.len()), None);
    }

    fn detect_bytes_per_pixel(width: u32, height: u32, data_len: usize) -> Option<u32> {
        let pixel_count = (width * height) as usize;

        if data_len < pixel_count {
            return None;
        }

        for bpp in &[4, 3, 2] {
            let expected_size = pixel_count * (*bpp as usize);
            if data_len >= expected_size && data_len < expected_size + 1024 {
                return Some(*bpp);
            }
        }

        None
    }
}

// ============================================================
// CROSS-PLATFORM COMPATIBILITY TESTS
// ============================================================

#[cfg(test)]
mod cross_platform_tests {
    use super::super::types::UsbCommand;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{RwLock, mpsc};

    #[test]
    fn test_usb_command_channel_send_receive() {
        // Test that UsbCommand variants can be sent through channels on all platforms
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (tx, mut rx) = mpsc::channel::<UsbCommand>(10);

            // Test Tap command with response channel
            let (tx_resp, _rx_resp) = tokio::sync::oneshot::channel();
            tx.send(UsbCommand::Tap {
                x: 100,
                y: 200,
                response_tx: tx_resp,
            })
            .await
            .unwrap();

            // Verify it's received correctly
            let cmd = rx.recv().await.unwrap();
            match cmd {
                UsbCommand::Tap { x, y, .. } => {
                    assert_eq!(x, 100);
                    assert_eq!(y, 200);
                }
                _ => panic!("Expected Tap command"),
            }
        });
    }

    #[test]
    fn test_oneshot_channel_send_receive() {
        // Test oneshot channels used in tap/swipe response - critical for cross-platform
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (tx, rx) = tokio::sync::oneshot::channel::<String>();

            tokio::spawn(async move {
                let _ = tx.send("test message".to_string());
            });

            let msg = tokio::time::timeout(Duration::from_secs(1), rx)
                .await
                .expect("timeout")
                .expect("channel closed");

            assert_eq!(msg, "test message");
        });
    }

    #[tokio::test]
    async fn test_concurrent_arc_mutex_operations() {
        // Test Arc<Mutex> pattern used in USB device locking on all platforms
        let shared = Arc::new(tokio::sync::Mutex::new(vec![1, 2, 3]));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let shared_clone = Arc::clone(&shared);
                tokio::spawn(async move {
                    let mut vec = shared_clone.lock().await;
                    vec.push(i + 4);
                })
            })
            .collect();

        for handle in handles {
            handle.await.unwrap();
        }

        let final_vec = shared.lock().await;
        assert_eq!(final_vec.len(), 7, "All concurrent operations completed");
    }

    #[tokio::test]
    async fn test_rwlock_read_write_consistency() {
        // Test RwLock pattern used for screenshot data updates on all platforms
        let data = Arc::new(RwLock::new(String::from("initial")));

        // Spawn multiple readers
        let read_handles: Vec<_> = (0..3)
            .map(|_| {
                let data_clone = Arc::clone(&data);
                tokio::spawn(async move {
                    let val = data_clone.read().await.clone();
                    val.len() > 0
                })
            })
            .collect();

        // Spawn a writer
        let data_clone = Arc::clone(&data);
        let write_handle = tokio::spawn(async move {
            let mut val = data_clone.write().await;
            val.push_str(" updated");
        });

        // Await all tasks
        for handle in read_handles {
            assert!(handle.await.unwrap());
        }
        write_handle.await.unwrap();

        let final_val = data.read().await.clone();
        assert_eq!(final_val, "initial updated");
    }

    #[tokio::test]
    async fn test_error_type_serialization() {
        // Test that AdbError can be converted to string across platforms
        use super::super::error::AdbError;

        let err = AdbError::TapOutOfBounds { x: 100, y: 200 };
        let err_str = err.to_string();

        // Verify error message is meaningful on all platforms
        assert!(!err_str.is_empty());
        assert!(err_str.contains("100") || err_str.contains("200"));
    }

    #[test]
    fn test_platform_specific_path_handling() {
        // Verify that string paths work consistently across platforms
        // (no hardcoded path separators)
        let device_name = "emulator-5554";
        let normalized = device_name.replace('\\', "/");

        // Should be unchanged on all platforms when using logical names
        assert_eq!(normalized, device_name);
    }

    #[tokio::test]
    async fn test_channel_capacity_behavior() {
        // Test that channel capacity works consistently on all platforms
        let (tx, mut rx) = mpsc::channel::<u32>(2);

        // Send two messages (should succeed)
        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();

        // Receive them
        assert_eq!(rx.recv().await.unwrap(), 1);
        assert_eq!(rx.recv().await.unwrap(), 2);

        // Channel should be empty now
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_timeout_duration_behavior() {
        // Verify Duration and timeout work consistently across platforms
        let start = std::time::Instant::now();

        let result = tokio::time::timeout(
            Duration::from_millis(100),
            tokio::time::sleep(Duration::from_millis(50)),
        )
        .await;

        let elapsed = start.elapsed();
        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_millis(50));
        assert!(elapsed < Duration::from_millis(200));
    }
}
