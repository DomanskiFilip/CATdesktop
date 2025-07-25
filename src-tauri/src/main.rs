// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "android")]
use jni::JavaVM;
#[cfg(target_os = "android")]
use std::sync::Arc;
#[cfg(target_os = "android")]
mod encryption_utils;

use app_lib::run_impl;

fn main() {
    #[cfg(target_os = "android")]
    {
        tauri::Builder::default()
            .setup(|app| {
                app.handle().runtime().run_on_android_context(|env, _activity, _webview| {
                    let vm = env.get_java_vm().unwrap();
                    let _ = encryption_utils::platform::initialize_android_jni_context(vm);
                });
                Ok(())
            })
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    }

    #[cfg(not(target_os = "android"))]
    {
        run_impl().expect("error while running app_lib");
    }
}