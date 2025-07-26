#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use app_lib::run_impl;

fn main() {
    run_impl().expect("error while running app_lib");
}
