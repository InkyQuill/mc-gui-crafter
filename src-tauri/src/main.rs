#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mc_gui_crafter::configure_platform_environment();
    mc_gui_crafter::run();
}
