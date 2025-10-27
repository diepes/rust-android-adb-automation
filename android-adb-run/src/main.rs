use android_adb_run::gui::dioxus_app::run_gui; // updated after rename
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Defaults
    let mut mode: Option<&str> = None; // None => GUI
    let mut use_rust_adb_impl: bool = true; // default rust

    // Parse all flags (skip program name)
    for arg in args.iter().skip(1) {
        if arg == "--help" || arg == "-h" {
            print_help();
            return;
        } else if arg == "--version" || arg == "-v" {
            println!("Android ADB Run v{}", env!("CARGO_PKG_VERSION"));
            return;
        } else if arg == "--gui" {
            mode = Some("gui");
        } else if arg == "--screenshot" || arg == "-s" {
            mode = Some("screenshot");
        } else if let Some(rest) = arg.strip_prefix("--impl=") {
            use_rust_adb_impl = match rest {
                "rust" => true,
                "shell" => false,
                other => {
                    println!("❌ Unknown impl '{}', expected 'rust' or 'shell'", other);
                    return;
                }
            };
        } else {
            println!("❌ Unknown argument: {}", arg);
            print_help();
            return;
        }
    }

    match mode {
        Some("screenshot") => {
            let impl_str = if use_rust_adb_impl { "rust" } else { "shell" };
            println!("📸 CLI screenshot using impl='{}'...", impl_str);
            unsafe {
                std::env::set_var("ADB_IMPL", impl_str);
            }
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                match android_adb_run::adb_backend::AdbBackend::list_devices_env().await {
                    Ok(devs) if !devs.is_empty() => {
                        let first = &devs[0];
                        match android_adb_run::adb_backend::AdbBackend::new_with_device(&first.name).await {
                            Ok(client) => {
                                let (sx, sy) = client.screen_dimensions();
                                println!("📱 Device: {} size: {}x{} (backend={})", client.device_name(), sx, sy, impl_str);
                                match client.screen_capture().await {
                                    Ok(cap) => {
                                        if let Err(e) = tokio::fs::write("cli-screenshot.png", &cap.bytes).await { println!("❌ Write failed: {e}"); } else { println!("✅ Screenshot #{} ({}ms) saved to cli-screenshot.png", cap.index, cap.duration_ms); }
                                    }
                                    Err(e) => println!("❌ Screenshot failed: {e}"),
                                }
                            }
                            Err(e) => println!("❌ Open device error: {e}"),
                        }
                    }
                    Ok(_) => println!("❌ No devices found"),
                    Err(e) => println!("❌ List error: {e}"),
                }
            });
        }
        Some("gui") | None => {
            let impl_str = if use_rust_adb_impl { "rust" } else { "shell" };
            println!(
                "🚀 Launching Android ADB Control GUI (impl='{}')...",
                impl_str
            );
            unsafe {
                std::env::set_var("ADB_IMPL", impl_str);
            }
            run_gui();
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
    println!("    --impl=<shell|rust> Select ADB implementation for CLI actions (default: rust)");
    println!(
        "                        The shell implementation reqires the ADB tool to be installed."
    );
    println!("    --help, -h          Show this help message");
    println!("    --version, -v       Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    android-adb-run --screenshot");
    println!("    android-adb-run --screenshot --impl=rust");
    println!("    android-adb-run --impl=shell --screenshot");
    println!("    android-adb-run --gui");
}
