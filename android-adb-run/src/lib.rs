pub mod adb;
pub mod adb_backend;
pub mod adb_client;
pub mod adb_shell;
pub mod gui; // replaced old dioxus root module

pub use adb_backend::AdbBackend;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
