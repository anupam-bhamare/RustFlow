// build.rs
fn main() {
    // If your file is in a 'ui' folder, use "ui/main.slint"
    // If it's in the same folder as Cargo.toml, use "main.slint"
    slint_build::compile("ui/main.slint").unwrap();
}