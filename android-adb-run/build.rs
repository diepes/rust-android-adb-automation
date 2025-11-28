use std::env;
use std::process::Command;
use time::OffsetDateTime;

fn main() {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let build_year = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|raw| raw.parse::<i64>().ok())
        .and_then(|epoch| OffsetDateTime::from_unix_timestamp(epoch).ok())
        .map(|dt| dt.year())
        .unwrap_or_else(|| OffsetDateTime::now_utc().year());

    println!("cargo:rustc-env=APP_BUILD_YEAR={build_year}");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    let package_version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let expected_tag = format!("v{package_version}");

    let git_tag = Command::new("git")
        .args(["describe", "--tags", "--exact-match"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    let display_version = if matches!(git_tag.as_deref(), Some(tag) if tag == expected_tag) {
        package_version.clone()
    } else {
        format!("{package_version}-dev")
    };

    println!("cargo:rustc-env=APP_VERSION_DISPLAY={display_version}");
    println!("cargo:rustc-env=APP_VERSION_SEMVER={package_version}");
}
