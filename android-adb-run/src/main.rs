use android_adb_run::adb::AdbClient;
use android_adb_run::gui::dioxus_app::run_gui;
use std::env;

fn main() {
    // Initialize the logger
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    // Defaults
    let mut mode: Option<&str> = None; // None => GUI
    let mut debug_mode: bool = false; // default no debug

    // Parse all flags (skip program name)
    for arg in args.iter().skip(1) {
        if arg == "--help" || arg == "-h" {
            print_help();
            return;
        } else if arg == "--version" || arg == "-v" {
            println!("Android ADB Run v{}", env!("CARGO_PKG_VERSION"));
            return;
        } else if arg == "--debug" {
            debug_mode = true;
        } else if arg == "--gui" {
            mode = Some("gui");
        } else if arg == "--screenshot" || arg == "-s" {
            mode = Some("screenshot");
        } else {
            eprintln!("❌ Unknown argument: {}", arg);
            print_help();
            return;
        }
    }

    match mode {
        Some("screenshot") => {
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
        Some("gui") | None => {
            println!(
                "🚀 Launching Android ADB Control GUI{}...",
                if debug_mode { " [DEBUG MODE]" } else { "" }
            );
            run_gui(debug_mode);
        }
        _ => unreachable!(),
    }
}

fn print_help() {
    println!("🤖 Android ADB Automation Tool");
    println!();
    println!("USAGE:");
    println!("    android-adb-run [FLAGS]");
    println!();
    println!("FLAGS:");
    println!("    (no flags)          Launch GUI interface");
    println!("    --gui               Launch GUI interface");
    println!("    --screenshot, -s    Take a screenshot and save to file (cli-screenshot.png)");
    println!("    --debug             Enable debug output for automation");
    println!("    --help, -h          Show this help message");
    println!("    --version, -v       Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    android-adb-run --screenshot");
    println!("    android-adb-run --gui");
    println!("    android-adb-run --debug");
}
