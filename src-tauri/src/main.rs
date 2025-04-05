mod credentials;
mod states;
mod tapadapter;

use crate::credentials::CredentialsState;
use crate::states::VpnState;
use crate::tapadapter::TapAdapter;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

pub fn get_app_paths() -> Result<(PathBuf, PathBuf), String> {
    // Try to get executable path first
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?
        .parent()
        .ok_or("Failed to get executable directory")?
        .to_path_buf();

    // Common installation paths to check
    let possible_paths = vec![
        exe_dir.clone(),
        PathBuf::from("C:\\Program Files\\GekkoVPN"),
        PathBuf::from("C:\\Program Files (x86)\\GekkoVPN"),
    ];

    // Find first valid path that contains our binaries
    for base_path in possible_paths {
        let openvpn_dir = base_path.join("bin").join("openvpn_amd64");
        let config_dir = base_path.join("openvpn_config");

        if openvpn_dir.exists() && config_dir.exists() {
            return Ok((openvpn_dir, config_dir));
        }
    }

    // If no valid paths found, fall back to executable directory
    Ok((
        exe_dir.join("bin").join("openvpn_amd64"),
        exe_dir.join("openvpn_config"),
    ))
}

#[tauri::command]
async fn connect_vpn(
    vpn_state: State<'_, VpnState>,
    server_name: String,
    mut username: String,
) -> Result<String, String> {
    // Get application paths
    let (openvpn_dir, config_dir) = get_app_paths()?;

    // Initialize and ensure TAP adapter exists
    let tap = TapAdapter::new(openvpn_dir.parent().unwrap().to_path_buf());
    tap.ensure_adapter_exists()?;

    // Get stored password using the username
    let keyring = keyring::Entry::new("GekkoVPN", &username)
        .map_err(|e| format!("Failed to access keyring: {}", e))?;

    let password = keyring
        .get_password()
        .map_err(|e| format!("Failed to get password: {}", e))?;

    username.push_str("@GekkoVPN");
    println!("Auth Details:");
    println!("Username: {}", username);
    println!("Password retrieved from keyring");

    // Setup OpenVPN paths
    let openvpn_path = openvpn_dir.join("openvpn.exe");
    let config_path = config_dir
        .join(&server_name)
        .join("gekko-vpn-server_openvpn_remote_access_l3.ovpn");

    println!("OpenVPN binary path: {:?}", openvpn_path);
    println!("OpenVPN config path: {:?}", config_path);

    // Validate paths and VPN state
    if !openvpn_path.exists() {
        return Err(format!("OpenVPN binary not found at {:?}", openvpn_path));
    }
    if !config_path.exists() {
        return Err(format!("Config file not found at {:?}", config_path));
    }
    if vpn_state.child_process.lock().unwrap().is_some() {
        return Err("VPN is already running. Please disconnect first.".to_string());
    }

    // Start OpenVPN process
    let mut child = Command::new(&openvpn_path)
        .arg("--config")
        .arg(&config_path)
        .arg("--auth-nocache")
        .arg("--auth-retry")
        .arg("none")
        .arg("--connect-retry")
        .arg("1")
        .arg("--data-ciphers")
        .arg("AES-256-GCM:AES-128-GCM:AES-128-CBC")
        .arg("--cipher")
        .arg("AES-128-CBC")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start OpenVPN: {}", e))?;

    let (tx, rx) = std::sync::mpsc::channel();
    let mut stdin_writer = child.stdin.take().ok_or("Failed to get stdin")?;

    // Handle stdout with line buffering
    let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
    let tx_clone = tx.clone();
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        while let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            print!("[OpenVPN] {}", line);

            if line.contains("Enter Auth Username:") {
                println!("Username prompt detected");
                tx_clone.send("need_username").unwrap_or(());
            } else if line.contains("Enter Auth Password:") {
                println!("Password prompt detected");
                tx_clone.send("need_password").unwrap_or(());
            } else if line.contains("Initialization Sequence Completed") {
                println!("Connection successful!");
                tx_clone.send("connected").unwrap_or(());
            } else if line.contains("AUTH_FAILED") {
                println!("Authentication failed!");
                tx_clone.send("auth_failed").unwrap_or(());
            }

            line.clear();
        }
    });

    // Handle stderr
    let stderr = child.stderr.take().ok_or("Failed to get stderr")?;
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();

        while let Ok(n) = reader.read_line(&mut line) {
            if n == 0 {
                break;
            }
            eprint!("[OpenVPN] {}", line);
            line.clear();
        }
    });

    // Initial credentials
    println!("Sending initial username: {}", username);
    stdin_writer
        .write_all(username.as_bytes())
        .and_then(|_| stdin_writer.write_all(b"\n"))
        .and_then(|_| stdin_writer.flush())
        .map_err(|e| format!("Failed to write initial username: {}", e))?;

    std::thread::sleep(std::time::Duration::from_millis(500));

    println!("Sending initial password");
    stdin_writer
        .write_all(password.as_bytes())
        .and_then(|_| stdin_writer.write_all(b"\n"))
        .and_then(|_| stdin_writer.flush())
        .map_err(|e| format!("Failed to write initial password: {}", e))?;

    // Handle connection with timeout
    let timeout = std::time::Duration::from_secs(30);
    let start_time = Instant::now();
    let mut auth_attempts = 0;
    const MAX_AUTH_ATTEMPTS: i32 = 2;

    while start_time.elapsed() < timeout {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok("need_username") if auth_attempts < MAX_AUTH_ATTEMPTS => {
                println!("Received username prompt, retrying...");
                stdin_writer
                    .write_all(username.as_bytes())
                    .and_then(|_| stdin_writer.write_all(b"\n"))
                    .and_then(|_| stdin_writer.flush())
                    .map_err(|e| format!("Failed to write username: {}", e))?;
                auth_attempts += 1;
            }
            Ok("need_password") if auth_attempts < MAX_AUTH_ATTEMPTS => {
                println!("Received password prompt, retrying...");
                std::thread::sleep(std::time::Duration::from_millis(500));
                stdin_writer
                    .write_all(password.as_bytes())
                    .and_then(|_| stdin_writer.write_all(b"\n"))
                    .and_then(|_| stdin_writer.flush())
                    .map_err(|e| format!("Failed to write password: {}", e))?;
                auth_attempts += 1;
            }
            Ok("connected") => {
                println!("VPN connection established successfully");
                *vpn_state.child_process.lock().unwrap() = Some(child);
                *vpn_state.connected_server.lock().unwrap() = Some(server_name.clone());
                return Ok(format!(
                    "Connected to {} with user {}",
                    server_name, username
                ));
            }
            Ok("auth_failed") => {
                child.kill().unwrap_or(());
                return Err("Authentication failed. Please check your credentials.".to_string());
            }
            _ => continue,
        }
    }

    // Timeout reached
    child.kill().unwrap_or(());
    Err("Connection timed out waiting for authentication".to_string())
}

#[tauri::command]
async fn disconnect_vpn(state: State<'_, VpnState>) -> Result<String, String> {
    if let Some(mut child) = state.child_process.lock().unwrap().take() {
        child
            .kill()
            .map_err(|e| format!("Failed to kill OpenVPN process: {}", e))?;
        *state.connected_server.lock().unwrap() = None;
        Ok("Disconnected from VPN".to_string())
    } else {
        Ok("Not connected to VPN".to_string())
    }
}

#[tauri::command]
async fn get_vpn_status(state: State<'_, VpnState>) -> Result<bool, String> {
    Ok(state.child_process.lock().unwrap().is_some())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(VpnState {
            child_process: Mutex::new(None),
            connected_server: Mutex::new(None),
        })
        .manage(CredentialsState {
            credentials: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            connect_vpn,
            disconnect_vpn,
            get_vpn_status,
            credentials::save_vpn_password,
            credentials::get_vpn_password,
            credentials::associate_username,
            credentials::clear_credentials,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
