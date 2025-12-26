pub mod adb;
pub mod args;
pub mod game_automation;
pub mod gui; // replaced old dioxus root module

pub use adb::AdbBackend;

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
