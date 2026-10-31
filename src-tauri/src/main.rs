// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Answer before any GTK/WebView setup so it works without a display;
    // docs/RELEASE-AND-UPDATES.md tells users to compare this with Releases.
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("archdroid {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    archdroid_lib::run();
}
