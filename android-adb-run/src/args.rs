use std::env;

#[derive(Debug, Clone)]
pub enum Mode {
    Gui,
    Screenshot,
}

#[derive(Debug)]
pub struct Args {
    pub mode: Mode,
    pub debug_mode: bool,
    pub debug_mode_timeout_secs: Option<u64>,
}

impl Args {
    pub fn parse() -> Option<Self> {
        let args: Vec<String> = env::args().collect();

        let mut mode: Option<Mode> = None;
        let mut debug_mode: bool = false;
        let mut timeout_secs: Option<u64> = None;

        for arg in args.iter().skip(1) {
            if arg == "--help" || arg == "-h" {
                print_help();
                return None;
            } else if arg == "--version" || arg == "-v" {
                println!("Android ADB Run v{}", env!("CARGO_PKG_VERSION"));
                return None;
            } else if arg == "--debug" {
                debug_mode = true;
            } else if arg == "--gui" {
                mode = Some(Mode::Gui);
            } else if arg == "--screenshot" || arg == "-s" {
                mode = Some(Mode::Screenshot);
            } else if arg.starts_with("--timeout=") {
                if let Some(val) = arg.strip_prefix("--timeout=") {
                    match val.parse::<u64>() {
                        Ok(secs) => timeout_secs = Some(secs),
                        Err(_) => {
                            eprintln!("❌ Invalid timeout value: {}", val);
                            return None;
                        }
                    }
                }
            } else {
                eprintln!("❌ Unknown argument: {}", arg);
                print_help();
                return None;
            }
        }

        Some(Args {
            mode: mode.unwrap_or(Mode::Gui),
            debug_mode,
            debug_mode_timeout_secs: timeout_secs,
        })
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
    println!("    --timeout=N         Auto-exit after N seconds (for testing)");
    println!("    --help, -h          Show this help message");
    println!("    --version, -v       Show version information");
    println!();
    println!("EXAMPLES:");
    println!("    android-adb-run --screenshot");
    println!("    android-adb-run --gui");
    println!("    android-adb-run --debug");
}
