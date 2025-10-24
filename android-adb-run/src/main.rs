use android_adb_run::adb::Adb;
use android_adb_run::gui::dioxus::run_gui; // updated path after moving dioxus.rs
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // If no command line arguments provided (only the program name), launch GUI
    if args.len() == 1 {
        println!("🚀 Launching Android ADB Control GUI...");
        run_gui();
        return;
    }
    
    // Handle command line arguments
    if args.len() > 1 {
        match args[1].as_str() {
            "--help" | "-h" => {
                print_help();
                return;
            }
            "--version" | "-v" => {
                println!("Android ADB Run v{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--gui" => {
                println!("🚀 Launching Android ADB Control GUI...");
                run_gui();
                return;
            }
            "--screenshot" | "-s" => {
                // Use tokio runtime for CLI commands only
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(take_screenshot());
                return;
            }
            _ => {
                println!("❌ Unknown argument: {}", args[1]);
                print_help();
                return;
            }
        }
    }
}

fn print_help() {
    println!("🤖 Android ADB Automation Tool");
    println!();
    println!("USAGE:");
    println!("    android-adb-run [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    (no args)        Launch GUI interface");
    println!("    --gui            Launch GUI interface");
    println!("    --screenshot, -s Take a screenshot and save to file");
    println!("    --help, -h       Show this help message");
    println!("    --version, -v    Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    android-adb-run              # Launch GUI");
    println!("    android-adb-run --gui        # Launch GUI explicitly");
    println!("    android-adb-run --screenshot # Take screenshot via CLI");
}

async fn take_screenshot() {
    match Adb::new(None).await {
        Ok(adb) => {
            println!(
                "📱 Connected to device: {} (transport_id: {}) screen size: {}x{}",
                adb.device.name,
                adb.transport_id,
                adb.screen_x,
                adb.screen_y
            );
            match adb.screen_capture("cli-screenshot.png").await {
                Ok(_) => println!("✅ Screenshot saved to cli-screenshot.png"),
                Err(e) => println!("❌ Screenshot failed: {}", e),
            }
        }
        Err(e) => {
            println!("❌ Error: {}", e);
        }
    }
}
