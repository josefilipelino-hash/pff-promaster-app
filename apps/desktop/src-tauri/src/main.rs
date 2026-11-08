// Evita uma janela de consola adicional no Windows em release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    momento_desktop_lib::run()
}
