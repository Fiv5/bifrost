use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use colored::Colorize;

use bifrost_core::BifrostError;

const SKILL_RAW_URL: &str = "https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/SKILL.md";

#[derive(Debug, Clone, PartialEq)]
pub enum AiTool {
    ClaudeCode,
    Codex,
    Trae,
    Cursor,
}

impl AiTool {
    fn all() -> Vec<AiTool> {
        vec![
            AiTool::ClaudeCode,
            AiTool::Codex,
            AiTool::Trae,
            AiTool::Cursor,
        ]
    }

    fn default_global_dirs(&self) -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        match self {
            AiTool::ClaudeCode => {
                vec![home.join(".claude").join("skills").join("bifrost")]
            }
            AiTool::Codex => vec![home.join(".codex").join("skills").join("bifrost")],
            AiTool::Trae => vec![
                home.join(".trae").join("skills").join("bifrost"),
                home.join(".trae-cn").join("skills").join("bifrost"),
            ],
            AiTool::Cursor => vec![home.join(".cursor").join("skills").join("bifrost")],
        }
    }

    fn project_local_dir(&self, base: &Path) -> PathBuf {
        match self {
            AiTool::ClaudeCode => base.join(".claude").join("skills").join("bifrost"),
            AiTool::Codex => base.join(".codex").join("skills").join("bifrost"),
            AiTool::Trae => base.join(".trae").join("skills").join("bifrost"),
            AiTool::Cursor => base.join(".cursor").join("skills").join("bifrost"),
        }
    }

    fn target_filename(&self) -> &str {
        "SKILL.md"
    }

    fn wrap_content(&self, raw_content: &str) -> String {
        raw_content.to_string()
    }
}

impl fmt::Display for AiTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AiTool::ClaudeCode => write!(f, "Claude Code"),
            AiTool::Codex => write!(f, "Codex"),
            AiTool::Trae => write!(f, "Trae"),
            AiTool::Cursor => write!(f, "Cursor"),
        }
    }
}

fn parse_tool(s: &str) -> Result<Vec<AiTool>, BifrostError> {
    match s.to_lowercase().replace(' ', "-").as_str() {
        "all" => Ok(AiTool::all()),
        "claude-code" | "claude" => Ok(vec![AiTool::ClaudeCode]),
        "codex" | "openai-codex" => Ok(vec![AiTool::Codex]),
        "trae" => Ok(vec![AiTool::Trae]),
        "cursor" => Ok(vec![AiTool::Cursor]),
        _ => Err(BifrostError::Config(format!(
            "Unknown tool: '{}'. Available: claude-code, codex, trae, cursor, all",
            s
        ))),
    }
}

fn format_network_error(err: &ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, resp) => {
            let url = resp.get_url();
            match code {
                404 => format!(
                    "HTTP 404 Not Found: the remote file was not found at {url}. \
                     The URL may have changed or the file may have been removed."
                ),
                403 => format!(
                    "HTTP 403 Forbidden: access denied to {url}. \
                     Check if the repository is public or if a token is required."
                ),
                429 => "HTTP 429 Too Many Requests: rate limited by the server. \
                     Please wait a moment and try again."
                    .to_string(),
                500..=599 => format!(
                    "HTTP {code} Server Error: the remote server returned an error. \
                     This is likely a temporary issue — please retry later."
                ),
                _ => format!("HTTP {code}: unexpected status code from {url}."),
            }
        }
        ureq::Error::Transport(transport) => {
            let kind = transport.kind();
            let detail = transport
                .message()
                .map(|m| m.to_string())
                .unwrap_or_default();
            match kind {
                ureq::ErrorKind::Dns => format!(
                    "DNS resolution failed: could not resolve the hostname. \
                     Check your internet connection and DNS settings. ({detail})"
                ),
                ureq::ErrorKind::ConnectionFailed => format!(
                    "Connection failed: could not connect to the remote server. \
                     The server may be down or a firewall may be blocking the connection. ({detail})"
                ),
                ureq::ErrorKind::Io => {
                    let lower = detail.to_lowercase();
                    if lower.contains("timed out") || lower.contains("timeout") {
                        format!(
                            "Connection timed out: the server did not respond in time. \
                             Check your network or try again later. ({detail})"
                        )
                    } else if lower.contains("connection refused") {
                        format!(
                            "Connection refused: the server actively refused the connection. ({detail})"
                        )
                    } else if lower.contains("reset") {
                        format!(
                            "Connection reset: the connection was unexpectedly closed. ({detail})"
                        )
                    } else {
                        format!("Network I/O error: {detail}")
                    }
                }
                ureq::ErrorKind::TooManyRedirects => "Too many redirects: the server redirected too many times. \
                     The URL may be misconfigured."
                    .to_string(),
                ureq::ErrorKind::BadStatus => format!(
                    "Bad status line: received a malformed HTTP response. ({detail})"
                ),
                ureq::ErrorKind::BadHeader => format!(
                    "Bad header: received a malformed HTTP header. ({detail})"
                ),
                _ => format!("Transport error ({}): {detail}", kind),
            }
        }
    }
}

fn format_io_error(err: &io::Error, path: &Path, operation: &str) -> BifrostError {
    let path_display = path.display();
    match err.kind() {
        io::ErrorKind::PermissionDenied => BifrostError::Io(io::Error::new(
            err.kind(),
            format!(
                "Permission denied: cannot {operation} '{path_display}'. \
                 Try running with sudo or choose a different directory with --dir <path>"
            ),
        )),
        io::ErrorKind::NotFound => BifrostError::Io(io::Error::new(
            err.kind(),
            format!(
                "Path not found: '{path_display}' does not exist and cannot be created. \
                 Verify the path is correct or use --dir <path> to specify a different location."
            ),
        )),
        io::ErrorKind::AlreadyExists => BifrostError::Io(io::Error::new(
            err.kind(),
            format!("Path conflict: '{path_display}' already exists as a different type. ({err})"),
        )),
        _ => {
            let raw = err.raw_os_error();
            let os_hint = raw
                .map(|code| format!(" (OS error {code})"))
                .unwrap_or_default();
            let lower = err.to_string().to_lowercase();
            let hint = if lower.contains("no space") || lower.contains("disk full") {
                " Hint: the disk may be full — free up space and retry."
            } else if lower.contains("name too long") || lower.contains("file name too long") {
                " Hint: the file path is too long — use --dir <path> with a shorter path."
            } else if lower.contains("read-only") {
                " Hint: the filesystem is read-only — choose a writable location with --dir <path>."
            } else {
                ""
            };
            BifrostError::Io(io::Error::new(
                err.kind(),
                format!("Failed to {operation} '{path_display}': {err}{os_hint}.{hint}"),
            ))
        }
    }
}

fn download_skill() -> Result<String, BifrostError> {
    println!(
        "{} {}",
        "⬇ Downloading latest SKILL.md from:".bright_cyan(),
        SKILL_RAW_URL.dimmed()
    );

    let response = ureq::get(SKILL_RAW_URL)
        .call()
        .map_err(|e| BifrostError::Network(format_network_error(&e)))?;

    let body = response.into_string().map_err(|e| {
        BifrostError::Network(format!(
            "Failed to read response body: {e}. \
             The download may have been interrupted — please retry."
        ))
    })?;

    if body.trim().is_empty() {
        return Err(BifrostError::Parse(
            "Downloaded SKILL.md is empty — the remote file may be blank or corrupted. \
             Please verify the source URL and try again."
                .to_string(),
        ));
    }

    if !body.starts_with("---\n") || body.matches("---").count() < 2 {
        println!(
            "  {} {}",
            "⚠".bright_yellow(),
            "Warning: Downloaded SKILL.md does not contain standard YAML frontmatter (---)."
                .bright_yellow()
        );
        println!(
            "    {}",
            "All major AI coding tools (Claude Code, Codex, Trae, Cursor) require frontmatter \
             with 'name' and 'description' fields for skill auto-discovery."
                .dimmed()
        );
    }

    println!(
        "{}",
        format!("✓ Downloaded {} bytes", body.len()).bright_green()
    );

    Ok(body)
}

fn prompt_confirm(message: &str) -> bool {
    print!("{} [y/N]: ", message);
    io::stdout().flush().ok();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn resolve_target_dirs(tool: &AiTool, custom_dir: &Option<PathBuf>, cwd: bool) -> Vec<PathBuf> {
    if let Some(d) = custom_dir {
        return vec![d.clone()];
    }
    if cwd {
        let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        return vec![tool.project_local_dir(&base)];
    }
    tool.default_global_dirs()
}

fn install_to_dir(tool: &AiTool, content: &str, target_dir: &Path) -> Result<(), BifrostError> {
    let target_file = target_dir.join(tool.target_filename());

    println!();
    println!(
        "{} {}",
        format!("📦 Installing to {}:", tool).bright_cyan().bold(),
        target_file.display()
    );

    if target_file.exists() {
        println!(
            "  {} {}",
            "⚠ Overwriting existing file with latest version from remote:".bright_yellow(),
            target_file.display()
        );
    }

    fs::create_dir_all(target_dir)
        .map_err(|e| format_io_error(&e, target_dir, "create directory"))?;

    let final_content = tool.wrap_content(content);

    fs::write(&target_file, &final_content)
        .map_err(|e| format_io_error(&e, &target_file, "write file"))?;

    println!(
        "  {} {} ({})",
        "✓".bright_green().bold(),
        target_file.display(),
        format!("{} bytes", final_content.len()).dimmed()
    );

    Ok(())
}

fn install_to_tool(
    tool: &AiTool,
    content: &str,
    custom_dir: &Option<PathBuf>,
    cwd: bool,
) -> Result<(), BifrostError> {
    let dirs = resolve_target_dirs(tool, custom_dir, cwd);
    for d in &dirs {
        install_to_dir(tool, content, d)?;
    }
    Ok(())
}

pub fn handle_install_skill(
    tool: Option<String>,
    dir: Option<PathBuf>,
    cwd: bool,
    yes: bool,
) -> Result<(), BifrostError> {
    if dir.is_some() && cwd {
        return Err(BifrostError::Config(
            "--dir and --cwd are mutually exclusive. Use --dir for a custom path, or --cwd for the current project directory.".to_string(),
        ));
    }

    let tools = match &tool {
        Some(t) => parse_tool(t)?,
        None => AiTool::all(),
    };

    let separator = "─".repeat(64);
    println!();
    println!("{}", separator.bright_cyan());
    println!("{}", "  🔧 Bifrost SKILL.md Installer".bright_cyan().bold());
    println!("{}", separator.bright_cyan());
    println!();

    println!(
        "  {} {}",
        "Source:".dimmed(),
        "GitHub main branch (latest)".bright_white()
    );
    println!(
        "  {} {}",
        "Target tools:".dimmed(),
        tools
            .iter()
            .map(|t| format!("{}", t))
            .collect::<Vec<_>>()
            .join(", ")
            .bright_white()
    );

    let mode_label = if dir.is_some() {
        "custom directory"
    } else if cwd {
        "project-local (current directory)"
    } else {
        "global"
    };
    println!(
        "  {} {}",
        "Install mode:".dimmed(),
        mode_label.bright_white()
    );

    if let Some(ref d) = dir {
        println!(
            "  {} {}",
            "Custom directory:".dimmed(),
            d.display().to_string().bright_white()
        );
    }

    println!();

    println!("  Target paths:");
    for t in &tools {
        let target_dirs = resolve_target_dirs(t, &dir, cwd);
        for target_dir in &target_dirs {
            let target_file = target_dir.join(t.target_filename());
            let exists = if target_file.exists() {
                " (exists → overwrite)".bright_yellow().to_string()
            } else {
                " (new)".bright_green().to_string()
            };
            println!(
                "    {} {} → {}{}",
                "•".bright_cyan(),
                t,
                target_file.display(),
                exists
            );
        }
    }

    println!();

    if !yes && !prompt_confirm("Proceed with installation?") {
        println!("{}", "Installation cancelled.".dimmed());
        return Ok(());
    }

    let content = download_skill()?;

    let mut success_count = 0;
    let mut errors: Vec<(AiTool, String)> = Vec::new();

    for t in &tools {
        match install_to_tool(t, &content, &dir, cwd) {
            Ok(()) => success_count += 1,
            Err(e) => {
                println!(
                    "  {} {} — {}",
                    "✗".bright_red().bold(),
                    t,
                    e.to_string().bright_red()
                );
                errors.push((t.clone(), e.to_string()));
            }
        }
    }

    println!();
    println!("{}", separator.bright_cyan());

    if errors.is_empty() {
        println!(
            "{}",
            format!(
                "  ✓ Successfully installed to {} tool{}!",
                success_count,
                if success_count > 1 { "s" } else { "" }
            )
            .bright_green()
            .bold()
        );
    } else {
        println!(
            "{}",
            format!(
                "  ⚠ Installed to {}/{} tools ({} failed)",
                success_count,
                tools.len(),
                errors.len()
            )
            .bright_yellow()
            .bold()
        );
        for (tool, err) in &errors {
            println!("    {} {}: {}", "✗".bright_red(), tool, err);
        }
    }

    println!("{}", separator.bright_cyan());
    println!();

    Ok(())
}
