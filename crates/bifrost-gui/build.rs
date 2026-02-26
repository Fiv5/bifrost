fn main() {
    #[cfg(windows)]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let assets_dir = std::path::Path::new(&manifest_dir)
            .join("..")
            .join("..")
            .join("assets");
        let ico_path = assets_dir.join("bifrost.ico");

        println!("cargo:rerun-if-changed={}", ico_path.display());

        if ico_path.exists() {
            let rc_path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icon.rc");
            std::fs::write(
                &rc_path,
                format!(
                    "1 ICON \"{}\"",
                    ico_path.display().to_string().replace('\\', "/")
                ),
            )
            .expect("Failed to write icon.rc");
            let _ = embed_resource::compile(&rc_path, embed_resource::NONE);
        }
    }
}
