// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// TODO: add back in, temporary while I debug windows auto updater
//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    pepo_lib::run()
}
