use android_adb_run::adb::{Adb, AdbClient};
use android_adb_run::adb_client::RustAdb;
use android_adb_run::gui::dioxus_app::run_gui; // updated after rename
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Defaults
    let mut mode: Option<&str> = None; // None => GUI
    let mut impl_choice: &str = "rust"; // default now rust; can override with --impl=shell

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
            impl_choice = rest;
        } else {
            println!("❌ Unknown argument: {}", arg);
            print_help();
            return;
        }
    }

    match mode {
        Some("screenshot") => {
            println!("📸 CLI screenshot using impl='{}'...", impl_choice);
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(take_screenshot(impl_choice));
        }
        Some("gui") | None => {
            println!(
                "🚀 Launching Android ADB Control GUI (impl='{}' for CLI ops)...",
                impl_choice
            );
            unsafe {
                std::env::set_var("ADB_IMPL", impl_choice);
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
    println!("    --help, -h          Show this help message");
    println!("    --version, -v       Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    android-adb-run --screenshot");
    println!("    android-adb-run --screenshot --impl=rust");
    println!("    android-adb-run --impl=shell --screenshot");
    println!("    android-adb-run --gui");
}

async fn take_screenshot(impl_choice: &str) {
    match impl_choice {
        "rust" => match RustAdb::list_devices().await {
            Ok(devs) if !devs.is_empty() => {
                let first = &devs[0];
                match RustAdb::new_with_device(&first.name).await {
                    Ok(radb) => {
                        println!(
                            "📱 (rust) Device: {} size: {}x{}",
                            radb.device_name(),
                            radb.screen_dimensions().0,
                            radb.screen_dimensions().1
                        );
                        match radb.screen_capture_bytes().await {
                            Ok(bytes) => {
                                if let Err(e) = tokio::fs::write("cli-screenshot.png", &bytes).await
                                {
                                    println!("❌ Write failed: {e}");
                                } else {
                                    println!("✅ Screenshot saved to cli-screenshot.png");
                                }
                            }
                            Err(e) => println!("❌ Screenshot failed: {e}"),
                        }
                    }
                    Err(e) => println!("❌ Rust impl device open error: {e}"),
                }
            }
            Ok(_) => println!("❌ No devices found (rust impl)"),
            Err(e) => println!("❌ Rust impl list error: {e}"),
        },
        _ => match Adb::new(None).await {
            Ok(adb) => {
                println!(
                    "📱 Device: {} (transport_id: {}) size: {}x{}",
                    adb.device.name, adb.transport_id, adb.screen_x, adb.screen_y
                );
                match adb.screen_capture("cli-screenshot.png").await {
                    Ok(_) => println!("✅ Screenshot saved to cli-screenshot.png"),
                    Err(e) => println!("❌ Screenshot failed: {e}"),
                }
            }
            Err(e) => println!("❌ Error: {e}"),
        },
    }
}
