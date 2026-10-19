//! `cargo run -p smriti-desktop --bin export-bindings` regenerates the UI's
//! TypeScript types from the Rust commands (also run in CI).

fn main() {
    smriti_desktop_lib::export_bindings(&smriti_desktop_lib::specta_builder());
    println!("wrote apps/desktop/src/lib/bindings.ts");
}
