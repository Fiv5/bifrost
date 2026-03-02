fn main() {
    println!("cargo:rerun-if-env-changed=BIFROST_VERSION");

    if let Ok(version) = std::env::var("BIFROST_VERSION") {
        println!("cargo:rustc-env=CARGO_PKG_VERSION={}", version);
    }
}
