use std::fs;
use std::path::PathBuf;

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;

fn detect_shell() -> Option<Shell> {
    let shell_env = std::env::var("SHELL").ok()?;
    if shell_env.ends_with("/zsh") || shell_env.ends_with("\\zsh.exe") {
        Some(Shell::Zsh)
    } else if shell_env.ends_with("/bash") || shell_env.ends_with("\\bash.exe") {
        Some(Shell::Bash)
    } else if shell_env.ends_with("/fish") || shell_env.ends_with("\\fish") {
        Some(Shell::Fish)
    } else {
        None
    }
}

fn completion_script(shell: Shell) -> Vec<u8> {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(shell, &mut cmd, "bifrost", &mut buf);
    buf
}

fn is_owned_by_current_user(path: &std::path::Path) -> bool {
    fs::metadata(path)
        .map(|m| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                m.uid() == unsafe { libc::getuid() }
            }
            #[cfg(not(unix))]
            {
                let _ = m;
                true
            }
        })
        .unwrap_or(false)
}

fn zsh_completion_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    let site_functions = PathBuf::from("/usr/local/share/zsh/site-functions");
    if site_functions.is_dir() && is_owned_by_current_user(&site_functions) {
        return Some(site_functions.join("_bifrost"));
    }

    let homebrew_site = PathBuf::from("/opt/homebrew/share/zsh/site-functions");
    if homebrew_site.is_dir() {
        return Some(homebrew_site.join("_bifrost"));
    }

    let zfunc = home.join(".zfunc");
    Some(zfunc.join("_bifrost"))
}

fn bash_completion_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    for dir in &[
        PathBuf::from("/usr/local/etc/bash_completion.d"),
        PathBuf::from("/etc/bash_completion.d"),
    ] {
        if dir.is_dir() && is_owned_by_current_user(dir) {
            return Some(dir.join("bifrost"));
        }
    }

    let bash_completions_dir = home.join(".local/share/bash-completion/completions");
    Some(bash_completions_dir.join("bifrost"))
}

fn fish_completion_path() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".config/fish/completions/bifrost.fish"))
}

fn install_completion_for_shell(shell: Shell) -> Result<PathBuf, String> {
    let target = match shell {
        Shell::Zsh => zsh_completion_path(),
        Shell::Bash => bash_completion_path(),
        Shell::Fish => fish_completion_path(),
        _ => None,
    }
    .ok_or_else(|| format!("cannot determine completion path for {:?}", shell))?;

    let script = completion_script(shell);

    if target.exists() {
        if let Ok(existing) = fs::read(&target) {
            if existing == script {
                return Ok(target);
            }
        }
    }

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create dir {}: {}", parent.display(), e))?;
    }

    fs::write(&target, &script)
        .map_err(|e| format!("failed to write {}: {}", target.display(), e))?;

    Ok(target)
}

pub fn install_completions_silently() {
    let result = std::panic::catch_unwind(|| {
        let shell = match detect_shell() {
            Some(s) => s,
            None => return,
        };
        match install_completion_for_shell(shell) {
            Ok(path) => {
                tracing::debug!(
                    target: "bifrost_cli::completions",
                    shell = ?shell,
                    path = %path.display(),
                    "shell completion script installed"
                );
            }
            Err(e) => {
                tracing::warn!(
                    target: "bifrost_cli::completions",
                    shell = ?shell,
                    error = %e,
                    "failed to install shell completion script, this does not affect proxy functionality"
                );
            }
        }
    });
    if let Err(e) = result {
        let msg = e
            .downcast_ref::<String>()
            .map(|s| s.as_str())
            .or_else(|| e.downcast_ref::<&str>().copied())
            .unwrap_or("unknown panic");
        tracing::warn!(
            target: "bifrost_cli::completions",
            error = msg,
            "shell completion installation panicked, this does not affect proxy functionality"
        );
    }
}
