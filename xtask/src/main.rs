use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::io::ErrorKind;

const ROOT_CARGO_TOML: &str = "Cargo.toml";
const TAURI_CONFIG_JSON: &str = "app/src-tauri/tauri.conf.json";
const DEV_SERVER_PORT: u16 = 1420;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;

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
        "trunk-serve-dev" => {
            if args.next().is_some() {
                return Err("usage: cargo run -p xtask -- trunk-serve-dev".to_string());
            }
            trunk_serve_dev()
        }
        _ => Err(format!("unknown xtask command '{command}'\n{}", usage())),
    }
}

fn usage() -> String {
    "usage:\n  cargo run -p xtask -- bump-version <version>\n  cargo run -p xtask -- trunk-serve-dev"
        .to_string()
}

fn trunk_serve_dev() -> Result<(), String> {
    free_dev_server_port()?;

    let status = Command::new("trunk")
        .arg("serve")
        .current_dir(app_dir()?)
        .status()
        .map_err(|error| format!("failed to start `trunk serve`: {error}"))?;

    exit_status_result(status, "`trunk serve`")
}

fn app_dir() -> Result<PathBuf, String> {
    Ok(repo_root()?.join("app"))
}

fn repo_root() -> Result<PathBuf, String> {
    let mut current_dir =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;

    loop {
        if current_dir.join("app").join("Trunk.toml").is_file()
            && current_dir.join(ROOT_CARGO_TOML).is_file()
        {
            return Ok(current_dir);
        }

        if !current_dir.pop() {
            return Err("failed to find repository root containing app/Trunk.toml".to_string());
        }
    }
}

fn free_dev_server_port() -> Result<(), String> {
    let listeners = find_dev_server_listeners()?;
    if listeners.is_empty() {
        return Ok(());
    }

    let (trunk_listeners, other_listeners): (Vec<_>, Vec<_>) = listeners
        .into_iter()
        .partition(|listener| is_trunk_process(&listener.command));

    if !other_listeners.is_empty() {
        return Err(format!(
            "port {DEV_SERVER_PORT} is already in use by {}. Stop that process or change the dev port before running Tauri.",
            format_listeners(&other_listeners)
        ));
    }

    println!(
        "port {DEV_SERVER_PORT} is already used by stale Trunk listener(s): {}",
        format_listeners(&trunk_listeners)
    );
    for listener in &trunk_listeners {
        terminate_process(listener.pid)?;
    }

    wait_for_dev_server_port()
}

fn wait_for_dev_server_port() -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let listeners = find_dev_server_listeners()?;
        if listeners.is_empty() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "port {DEV_SERVER_PORT} is still in use by {} after stopping stale Trunk listener(s)",
                format_listeners(&listeners)
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PortListener {
    pid: u32,
    command: String,
}

fn format_listeners(listeners: &[PortListener]) -> String {
    listeners
        .iter()
        .map(|listener| format!("{} (pid {})", listener.command, listener.pid))
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_trunk_process(command: &str) -> bool {
    let command = command
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(command)
        .to_ascii_lowercase();
    command == "trunk" || command == "trunk.exe"
}

fn exit_status_result(status: ExitStatus, command: &str) -> Result<(), String> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command} exited with status {status}"))
    }
}

#[cfg(unix)]
fn find_dev_server_listeners() -> Result<Vec<PortListener>, String> {
    let output = match Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{DEV_SERVER_PORT}"), "-sTCP:LISTEN"])
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("failed to inspect port {DEV_SERVER_PORT}: {error}")),
    };

    if !output.status.success() && output.stdout.is_empty() {
        return Ok(Vec::new());
    }

    String::from_utf8(output.stdout)
        .map_err(|error| format!("lsof output was not valid UTF-8: {error}"))
        .map(|output| parse_lsof_listeners(&output))
}

#[cfg(windows)]
fn find_dev_server_listeners() -> Result<Vec<PortListener>, String> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .map_err(|error| format!("failed to inspect port {DEV_SERVER_PORT}: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to inspect port {DEV_SERVER_PORT}: netstat exited with status {}",
            output.status
        ));
    }

    let netstat = String::from_utf8(output.stdout)
        .map_err(|error| format!("netstat output was not valid UTF-8: {error}"))?;
    let pids = parse_netstat_listener_pids(&netstat, DEV_SERVER_PORT);
    pids.into_iter()
        .map(|pid| {
            process_name(pid).map(|command| PortListener {
                pid,
                command: command.unwrap_or_else(|| "unknown process".to_string()),
            })
        })
        .collect()
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map_err(|error| format!("failed to stop stale Trunk process {pid}: {error}"))?;

    exit_status_result(status, &format!("kill {pid}"))
}

#[cfg(windows)]
fn terminate_process(pid: u32) -> Result<(), String> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .status()
        .map_err(|error| format!("failed to stop stale Trunk process {pid}: {error}"))?;

    if status.success() {
        return Ok(());
    }

    let force_status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .map_err(|error| format!("failed to force-stop stale Trunk process {pid}: {error}"))?;

    exit_status_result(force_status, &format!("taskkill /PID {pid} /T /F"))
}

fn parse_lsof_listeners(output: &str) -> Vec<PortListener> {
    let mut listeners = BTreeMap::new();
    for line in output.lines().skip(1) {
        let mut fields = line.split_whitespace();
        let Some(command) = fields.next() else {
            continue;
        };
        let Some(pid) = fields.next().and_then(|pid| pid.parse::<u32>().ok()) else {
            continue;
        };
        listeners.entry(pid).or_insert_with(|| PortListener {
            pid,
            command: command.to_string(),
        });
    }

    listeners.into_values().collect()
}

#[cfg(windows)]
fn parse_netstat_listener_pids(output: &str, port: u16) -> Vec<u32> {
    let mut pids = BTreeMap::new();
    let suffix = format!(":{port}");

    for line in output.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 5 || !fields[0].eq_ignore_ascii_case("tcp") {
            continue;
        }
        if !fields[3].eq_ignore_ascii_case("listening") || !fields[1].ends_with(&suffix) {
            continue;
        }
        if let Ok(pid) = fields[4].parse::<u32>() {
            pids.insert(pid, ());
        }
    }

    pids.into_keys().collect()
}

#[cfg(windows)]
fn process_name(pid: u32) -> Result<Option<String>, String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .map_err(|error| format!("failed to inspect process {pid}: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "failed to inspect process {pid}: tasklist exited with status {}",
            output.status
        ));
    }

    let output = String::from_utf8(output.stdout)
        .map_err(|error| format!("tasklist output was not valid UTF-8: {error}"))?;
    Ok(parse_tasklist_process_name(&output))
}

#[cfg(windows)]
fn parse_tasklist_process_name(output: &str) -> Option<String> {
    output
        .lines()
        .find(|line| !line.trim().is_empty() && !line.contains("No tasks"))
        .and_then(|line| line.split("\",\"").next())
        .map(|name| name.trim_matches('"').to_string())
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
    use super::{
        is_trunk_process, normalize_semver, parse_lsof_listeners, replace_json_version,
        replace_workspace_package_version,
    };

    #[cfg(windows)]
    use super::{parse_netstat_listener_pids, parse_tasklist_process_name};

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

    #[test]
    fn trunk_process_detection_matches_only_trunk_executables() {
        assert!(is_trunk_process("trunk"));
        assert!(is_trunk_process("/opt/homebrew/bin/trunk"));
        assert!(is_trunk_process(r"C:\Users\dev\.cargo\bin\trunk.exe"));
        assert!(!is_trunk_process("node"));
        assert!(!is_trunk_process("trunk-helper"));
    }

    #[test]
    fn parse_lsof_listeners_deduplicates_ipv4_and_ipv6_rows() {
        let input = "\
COMMAND   PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME
trunk   28087 dev     9u  IPv4 0xabc      0t0  TCP 127.0.0.1:1420 (LISTEN)
trunk   28087 dev    10u  IPv6 0xdef      0t0  TCP [::1]:1420 (LISTEN)
node    30001 dev    12u  IPv4 0xghi      0t0  TCP 127.0.0.1:1420 (LISTEN)
";

        let listeners = parse_lsof_listeners(input);

        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].pid, 28087);
        assert_eq!(listeners[0].command, "trunk");
        assert_eq!(listeners[1].pid, 30001);
        assert_eq!(listeners[1].command, "node");
    }

    #[cfg(windows)]
    #[test]
    fn parse_netstat_listener_pids_finds_ipv4_and_ipv6_port_rows() {
        let input = "\
  Proto  Local Address          Foreign Address        State           PID
  TCP    127.0.0.1:1420         0.0.0.0:0              LISTENING       28087
  TCP    [::1]:1420             [::]:0                 LISTENING       28087
  TCP    127.0.0.1:3000         0.0.0.0:0              LISTENING       30000
";

        assert_eq!(parse_netstat_listener_pids(input, 1420), vec![28087]);
    }

    #[cfg(windows)]
    #[test]
    fn parse_tasklist_process_name_reads_csv_image_name() {
        let input = "\"trunk.exe\",\"28087\",\"Console\",\"1\",\"10,000 K\"\r\n";

        assert_eq!(
            parse_tasklist_process_name(input),
            Some("trunk.exe".to_string())
        );
    }
}
