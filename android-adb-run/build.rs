use std::env;
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
}
