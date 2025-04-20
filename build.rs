fn main() {
    // We're now using the manual type generation via `cargo run -- config generate-types`
    // rather than doing it at build time, to avoid requiring cargo-typify during builds.
    
    // Check if src/types.rs exists - it's required for compilation
    if !std::path::Path::new("src/types.rs").exists() {
        println!("⚠️ Warning: src/types.rs does not exist.");
        println!("⚠️ To generate it, use: cargo run -- config generate-types");
        println!("⚠️ This requires cargo-typify to be installed: cargo install cargo-typify");
    }
    
    // Set rerun-if-changed for build.rs itself
    println!("cargo:rerun-if-changed=build.rs");
}