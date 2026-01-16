mod args;

use android_adb_run::adb::AdbClient;
use android_adb_run::gui::dioxus_app::run_gui;
use args::{Args, Mode};

fn main() {
    // Initialize the logger with filter for harmless cleanup errors
    env_logger::Builder::from_default_env()
        .filter(Some("adb_client::transports::usb_transport"), log::LevelFilter::Off)
        .init();

    let args = match Args::parse() {
        Some(args) => args,
        None => return,
    };

    match args.mode {
        Mode::Screenshot => {
            println!("📸 CLI screenshot mode...");
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                match android_adb_run::adb::AdbBackend::list_devices().await {
                    Ok(devs) if !devs.is_empty() => {
                        let first = &devs[0];
                        match android_adb_run::adb::AdbBackend::new_with_device(&first.name).await {
                            Ok(client) => {
                                let (sx, sy) = client.screen_dimensions();
                                println!("📱 Device: {} size: {}x{}", client.device_name(), sx, sy);
                                match client.screen_capture().await {
                                    Ok(cap) => {
                                        if let Err(e) =
                                            tokio::fs::write("cli-screenshot.png", &cap.bytes)
                                                .await
                                        {
                                            eprintln!("❌ Write failed: {e}");
                                        } else {
                                            println!(
                                                "✅ Screenshot #{} ({}ms) saved to cli-screenshot.png",
                                                cap.index, cap.duration_ms
                                            );
                                        }
                                    }
                                    Err(e) => eprintln!("❌ Screenshot failed: {e}"),
                                }
                            }
                            Err(e) => eprintln!("❌ Open device error: {e}"),
                        }
                    }
                    Ok(_) => eprintln!("❌ No devices found"),
                    Err(e) => eprintln!("❌ List error: {e}"),
                }
            });
        }
        Mode::Gui => {
            println!(
                "🚀 Launching Android ADB Control GUI{}...",
                if args.debug_mode { " [DEBUG MODE]" } else { "" }
            );
            //# timeout set spawn a thread to exit after timeout
            if let Some(secs) = args.debug_mode_timeout_secs {
                println!("⏱️  Auto-exit after {} seconds", secs);
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(secs));
                    println!("⏱️  Timeout reached, exiting...");
                    std::process::exit(0);
                });
            }
            // Run GUI, it will create async runtime and start backend
            run_gui(args.debug_mode);
        }
    }
}
