//! Thin headless CLI stub for `graycart-n64`.
//!
//! Full headless options (`--frames`, `--cycles`, `--hash-out`, ISViewer capture)
//! are wired in Task M0.5.

fn main() {
    println!("graycart-n64 {}", env!("CARGO_PKG_VERSION"));
}
