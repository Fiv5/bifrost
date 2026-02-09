use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = PathBuf::from(&manifest_dir);
    let web_dir = manifest_path
        .join("..")
        .join("..")
        .join("web")
        .canonicalize()
        .unwrap_or_else(|_| manifest_path.join("../../web"));
    let dist_dir = web_dir.join("dist");

    println!("cargo:rerun-if-changed={}", web_dir.join("src").display());
    println!(
        "cargo:rerun-if-changed={}",
        web_dir.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        web_dir.join("vite.config.ts").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        web_dir.join("tsconfig.json").display()
    );

    if env::var("SKIP_FRONTEND_BUILD").is_ok() {
        println!("cargo:warning=Skipping frontend build (SKIP_FRONTEND_BUILD is set)");
        ensure_dist_exists(&dist_dir);
        return;
    }

    if !web_dir.exists() {
        println!(
            "cargo:warning=Web directory not found at {}, skipping frontend build",
            web_dir.display()
        );
        ensure_dist_exists(&dist_dir);
        return;
    }

    let node_modules = web_dir.join("node_modules");
    if !node_modules.exists() {
        println!("cargo:warning=Installing frontend dependencies with pnpm...");
        let pnpm_cmd = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
        let output = Command::new(pnpm_cmd)
            .args(["install", "--frozen-lockfile"])
            .current_dir(&web_dir)
            .output()
            .or_else(|_| {
                Command::new(pnpm_cmd)
                    .arg("install")
                    .current_dir(&web_dir)
                    .output()
            })
            .expect("Failed to run pnpm install. Make sure pnpm is installed.");

        if !output.status.success() {
            eprintln!(
                "pnpm install stdout: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            eprintln!(
                "pnpm install stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            panic!(
                "pnpm install failed with exit code: {:?}",
                output.status.code()
            );
        }
    }

    println!(
        "cargo:warning=Building frontend at {}...",
        web_dir.display()
    );
    let pnpm_cmd = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
    let output = Command::new(pnpm_cmd)
        .args(["run", "build"])
        .current_dir(&web_dir)
        .output()
        .expect("Failed to run pnpm run build");

    if !output.status.success() {
        eprintln!(
            "Frontend build stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        eprintln!(
            "Frontend build stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        panic!(
            "Frontend build failed with exit code: {:?}",
            output.status.code()
        );
    }

    if !dist_dir.exists() {
        panic!(
            "Frontend build did not produce dist directory at {}",
            dist_dir.display()
        );
    }

    println!(
        "cargo:warning=Frontend build completed successfully at {}",
        dist_dir.display()
    );
}

fn ensure_dist_exists(dist_dir: &Path) {
    if !dist_dir.exists() {
        std::fs::create_dir_all(dist_dir).expect("Failed to create dist directory");
    }
    let index_html = dist_dir.join("index.html");
    if !index_html.exists() {
        std::fs::write(
            &index_html,
            r#"<!DOCTYPE html>
<html>
<head><title>Bifrost Admin</title></head>
<body>
<h1>Frontend not built</h1>
<p>Run <code>pnpm run build</code> in the web directory or rebuild with <code>cargo build</code></p>
</body>
</html>"#,
        )
        .expect("Failed to create placeholder index.html");
    }
}
