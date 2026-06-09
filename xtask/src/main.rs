use std::{env, fs, path::Path};

const ROOT_CARGO_TOML: &str = "Cargo.toml";
const TAURI_CONFIG_JSON: &str = "app/src-tauri/tauri.conf.json";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args
        .next()
        .ok_or_else(|| "usage: cargo bump-version <version>".to_string())?;

    match command.as_str() {
        "bump-version" => {
            let requested = args
                .next()
                .ok_or_else(|| "usage: cargo bump-version <version>".to_string())?;
            if args.next().is_some() {
                return Err("usage: cargo bump-version <version>".to_string());
            }
            let version = normalize_semver(&requested)?;
            update_versions(Path::new("."), &version)?;
            if version == requested {
                println!("Updated software version to {version}");
            } else {
                println!("Updated software version to {version} (normalized from {requested})");
            }
            Ok(())
        }
        _ => Err(format!(
            "unknown xtask command '{command}'\nusage: cargo bump-version <version>"
        )),
    }
}

fn update_versions(repo_root: &Path, version: &str) -> Result<(), String> {
    let cargo_path = repo_root.join(ROOT_CARGO_TOML);
    let tauri_path = repo_root.join(TAURI_CONFIG_JSON);

    let cargo_toml = fs::read_to_string(&cargo_path)
        .map_err(|error| format!("failed to read {}: {error}", cargo_path.display()))?;
    let updated_cargo_toml = replace_workspace_package_version(&cargo_toml, version)?;
    toml::from_str::<toml::Value>(&updated_cargo_toml)
        .map_err(|error| format!("updated {} is invalid TOML: {error}", cargo_path.display()))?;

    let tauri_config = fs::read_to_string(&tauri_path)
        .map_err(|error| format!("failed to read {}: {error}", tauri_path.display()))?;
    let updated_tauri_config = replace_json_version(&tauri_config, version)?;
    serde_json::from_str::<serde_json::Value>(&updated_tauri_config)
        .map_err(|error| format!("updated {} is invalid JSON: {error}", tauri_path.display()))?;

    fs::write(&cargo_path, updated_cargo_toml)
        .map_err(|error| format!("failed to write {}: {error}", cargo_path.display()))?;
    fs::write(&tauri_path, updated_tauri_config)
        .map_err(|error| format!("failed to write {}: {error}", tauri_path.display()))?;

    Ok(())
}

fn normalize_semver(requested: &str) -> Result<String, String> {
    let parts = requested
        .split('.')
        .map(|part| {
            if part.is_empty() {
                return Err(format!("invalid version '{requested}': empty version component"));
            }
            if !part.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(format!(
                    "invalid version '{requested}': only numeric major.minor.patch versions are supported"
                ));
            }
            part.parse::<u64>()
                .map_err(|error| format!("invalid version '{requested}': {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    if parts.len() != 3 {
        return Err(format!(
            "invalid version '{requested}': expected major.minor.patch"
        ));
    }

    Ok(format!("{}.{}.{}", parts[0], parts[1], parts[2]))
}

fn replace_workspace_package_version(input: &str, version: &str) -> Result<String, String> {
    let mut in_workspace_package = false;
    let mut replaced = false;
    let mut output = String::with_capacity(input.len());

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace_package = trimmed == "[workspace.package]";
        }

        if in_workspace_package && trimmed.starts_with("version") {
            let indent = line.strip_suffix(trimmed).unwrap_or_default();
            output.push_str(&format!("{indent}version = \"{version}\"\n"));
            replaced = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    if !replaced {
        return Err("failed to find [workspace.package] version in Cargo.toml".to_string());
    }

    Ok(output)
}

fn replace_json_version(input: &str, version: &str) -> Result<String, String> {
    let mut replaced = false;
    let mut output = String::with_capacity(input.len());

    for line in input.lines() {
        let trimmed = line.trim_start();
        if !replaced && trimmed.starts_with("\"version\"") {
            let indent = line.strip_suffix(trimmed).unwrap_or_default();
            let suffix = if trimmed.ends_with(',') { "," } else { "" };
            output.push_str(&format!("{indent}\"version\": \"{version}\"{suffix}\n"));
            replaced = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    if !replaced {
        return Err(format!(
            "failed to find top-level version in {TAURI_CONFIG_JSON}"
        ));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{normalize_semver, replace_json_version, replace_workspace_package_version};

    #[test]
    fn normalize_semver_accepts_date_like_input() {
        assert_eq!(normalize_semver("27.06.09").unwrap(), "27.6.9");
    }

    #[test]
    fn normalize_semver_accepts_canonical_input() {
        assert_eq!(normalize_semver("27.6.9").unwrap(), "27.6.9");
    }

    #[test]
    fn normalize_semver_rejects_non_numeric_input() {
        assert!(normalize_semver("27.6.beta").is_err());
    }

    #[test]
    fn replace_workspace_version_only_updates_workspace_package() {
        let input = "[workspace]\nresolver = \"3\"\n\n[workspace.package]\nversion = \"1.2.3\"\n\n[workspace.dependencies]\nserde = \"1\"\n";
        let output = replace_workspace_package_version(input, "2.3.4").unwrap();

        assert!(output.contains("[workspace.package]\nversion = \"2.3.4\""));
        assert!(output.contains("[workspace.dependencies]\nserde = \"1\""));
    }

    #[test]
    fn replace_json_version_preserves_comma() {
        let input = "{\n  \"productName\": \"SpeleoDB Compass Sidecar\",\n  \"version\": \"1.2.3\",\n  \"identifier\": \"com.speleodb-compass-sidecar\"\n}\n";
        let output = replace_json_version(input, "2.3.4").unwrap();

        assert!(output.contains("  \"version\": \"2.3.4\","));
        assert!(output.contains("  \"identifier\": \"com.speleodb-compass-sidecar\""));
    }
}
