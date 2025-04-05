use keyring::Entry;
use std::sync::Mutex;
use tauri::State;

const SERVICE_NAME: &str = "GekkoVPN";
const TEMP_KEY: &str = "temp_credentials";

#[derive(Debug, Clone)]
pub struct VpnCredentials {
    pub password: String,
}

pub struct CredentialsState {
    pub credentials: Mutex<Option<VpnCredentials>>,
}

#[tauri::command]
pub async fn save_vpn_password(
    state: State<'_, CredentialsState>,
    password: String,
) -> Result<(), String> {
    // First save to temporary storage
    let keyring = Entry::new(SERVICE_NAME, TEMP_KEY)
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;

    keyring
        .set_password(&password)
        .map_err(|e| format!("Failed to save temporary password: {}", e))?;

    // Update in-memory state
    let credentials = VpnCredentials { password };
    *state.credentials.lock().unwrap() = Some(credentials);
    Ok(())
}

#[tauri::command]
pub async fn associate_username(username: String) -> Result<(), String> {
    // Get password from temporary storage
    let temp_keyring = Entry::new(SERVICE_NAME, TEMP_KEY)
        .map_err(|e| format!("Failed to access temp storage: {}", e))?;

    let password = temp_keyring
        .get_password()
        .map_err(|e| format!("Failed to get temporary password: {}", e))?;

    // Save with actual username
    let user_keyring = Entry::new(SERVICE_NAME, &username)
        .map_err(|e| format!("Failed to create user keyring: {}", e))?;

    user_keyring
        .set_password(&password)
        .map_err(|e| format!("Failed to save user password: {}", e))?;

    // Clean up temporary storage
    let _ = temp_keyring.delete_password();

    Ok(())
}

#[tauri::command]
pub async fn get_vpn_password(username: String) -> Result<Option<String>, String> {
    let keyring = Entry::new(SERVICE_NAME, &username)
        .map_err(|e| format!("Failed to create keyring: {}", e))?;

    match keyring.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn clear_credentials(username: String) -> Result<(), String> {
    let keyring = Entry::new(SERVICE_NAME, &username)
        .map_err(|e| format!("Failed to access keyring: {}", e))?;

    if let Err(e) = keyring.delete_password() {
        println!("Failed to delete password: {}", e);
    }
    Ok(())
}
