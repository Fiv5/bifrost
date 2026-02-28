use bifrost_core::BifrostError;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpType {
    Patch,
    Minor,
    Major,
}

impl std::fmt::Display for BumpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BumpType::Patch => write!(f, "patch"),
            BumpType::Minor => write!(f, "minor"),
            BumpType::Major => write!(f, "major"),
        }
    }
}

impl std::str::FromStr for BumpType {
    type Err = BifrostError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "patch" | "1" => Ok(BumpType::Patch),
            "minor" | "2" => Ok(BumpType::Minor),
            "major" | "3" => Ok(BumpType::Major),
            _ => Err(BifrostError::Parse(format!(
                "Invalid bump type '{}'. Expected: patch, minor, major",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone)]
struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    prerelease: Option<String>,
}

impl Version {
    fn parse(version_str: &str) -> Result<Self, BifrostError> {
        let version_str = version_str.trim().trim_matches('"');

        let (version_part, prerelease) = if let Some(idx) = version_str.find('-') {
            (
                &version_str[..idx],
                Some(version_str[idx + 1..].to_string()),
            )
        } else {
            (version_str, None)
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() != 3 {
            return Err(BifrostError::Parse(format!(
                "Invalid version format '{}'. Expected: MAJOR.MINOR.PATCH",
                version_str
            )));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| BifrostError::Parse(format!("Invalid major version: {}", parts[0])))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| BifrostError::Parse(format!("Invalid minor version: {}", parts[1])))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| BifrostError::Parse(format!("Invalid patch version: {}", parts[2])))?;

        Ok(Version {
            major,
            minor,
            patch,
            prerelease,
        })
    }

    fn bump(&self, bump_type: BumpType) -> Self {
        match bump_type {
            BumpType::Patch => Version {
                major: self.major,
                minor: self.minor,
                patch: self.patch + 1,
                prerelease: self.prerelease.clone(),
            },
            BumpType::Minor => Version {
                major: self.major,
                minor: self.minor + 1,
                patch: 0,
                prerelease: self.prerelease.clone(),
            },
            BumpType::Major => Version {
                major: self.major + 1,
                minor: 0,
                patch: 0,
                prerelease: self.prerelease.clone(),
            },
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref pre) = self.prerelease {
            write!(f, "{}.{}.{}-{}", self.major, self.minor, self.patch, pre)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

fn find_workspace_root() -> Result<PathBuf, BifrostError> {
    let mut current_dir = std::env::current_dir()?;

    loop {
        let cargo_toml = current_dir.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(current_dir);
            }
        }
        if !current_dir.pop() {
            return Err(BifrostError::NotFound(
                "Could not find workspace root (Cargo.toml with [workspace])".to_string(),
            ));
        }
    }
}

fn read_current_version(cargo_toml_path: &Path) -> Result<String, BifrostError> {
    let content = fs::read_to_string(cargo_toml_path)?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version") && trimmed.contains('=') {
            if let Some(value) = trimmed.split('=').nth(1) {
                return Ok(value.trim().trim_matches('"').to_string());
            }
        }
    }

    Err(BifrostError::NotFound(
        "Could not find version in [workspace.package]".to_string(),
    ))
}

fn update_version_in_file(
    file_path: &Path,
    old_version: &str,
    new_version: &str,
) -> Result<bool, BifrostError> {
    let content = fs::read_to_string(file_path)?;

    let old_quoted = format!("\"{}\"", old_version);
    let new_quoted = format!("\"{}\"", new_version);

    if content.contains(&old_quoted) {
        let new_content = content.replace(&old_quoted, &new_quoted);
        fs::write(file_path, new_content)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn prompt_bump_type(current_version: &Version) -> Result<BumpType, BifrostError> {
    let patch_version = current_version.bump(BumpType::Patch);
    let minor_version = current_version.bump(BumpType::Minor);
    let major_version = current_version.bump(BumpType::Major);

    println!("Current version: {}\n", current_version);
    println!("Select upgrade strategy:");
    println!("  [1] patch → {}", patch_version);
    println!("  [2] minor → {}", minor_version);
    println!("  [3] major → {}", major_version);
    print!("\nEnter choice (1/2/3): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(BifrostError::Io)?;

    input.trim().parse()
}

pub fn handle_upgrade(bump: Option<String>, dry_run: bool) -> Result<(), BifrostError> {
    let workspace_root = find_workspace_root()?;
    let cargo_toml_path = workspace_root.join("Cargo.toml");

    tracing::info!(
        workspace = %workspace_root.display(),
        "Found workspace root"
    );

    let current_version_str = read_current_version(&cargo_toml_path)?;
    let current_version = Version::parse(&current_version_str)?;

    let bump_type = match bump {
        Some(b) => b.parse()?,
        None => prompt_bump_type(&current_version)?,
    };

    let new_version = current_version.bump(bump_type);

    println!("\nUpgrading: {} → {}", current_version, new_version);

    if dry_run {
        println!("\n[Dry run] No files will be modified");
        return Ok(());
    }

    let mut updated_files = Vec::new();

    if update_version_in_file(
        &cargo_toml_path,
        &current_version_str,
        &new_version.to_string(),
    )? {
        updated_files.push(cargo_toml_path.clone());
    }

    let crates_dir = workspace_root.join("crates");
    if crates_dir.exists() {
        for entry in fs::read_dir(&crates_dir)? {
            let entry = entry?;
            let crate_cargo_toml = entry.path().join("Cargo.toml");
            if crate_cargo_toml.exists()
                && update_version_in_file(
                    &crate_cargo_toml,
                    &current_version_str,
                    &new_version.to_string(),
                )?
            {
                updated_files.push(crate_cargo_toml);
            }
        }
    }

    println!("\nUpdated {} file(s):", updated_files.len());
    for file in &updated_files {
        let relative_path = file.strip_prefix(&workspace_root).unwrap_or(file);
        println!("  - {}", relative_path.display());
    }

    println!(
        "\n✓ Version upgraded from {} to {}",
        current_version, new_version
    );

    Ok(())
}
