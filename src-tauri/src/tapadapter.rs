use std::path::PathBuf;
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const TAP_WINDOWS_COMPONENT_ID: &str = "tap0901";
const NETWORK_ADAPTERS_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4D36E972-E325-11CE-BFC1-08002BE10318}";

pub struct TapAdapter {
    tapctl_path: PathBuf,
    base_dir: PathBuf,
}

impl TapAdapter {
    pub fn new(base_dir: PathBuf) -> Self {
        let arch = std::env::consts::ARCH;
        let tapctl_path = match arch {
            "x86_64" => base_dir.join("openvpn_amd64").join("tapctl.exe"),
            "aarch64" => base_dir.join("openvpn_arm64").join("tapctl.exe"),
            _ => panic!("Unsupported architecture: {}", arch),
        };

        if !tapctl_path.exists() {
            panic!("tapctl.exe not found at {:?}", tapctl_path);
        }

        TapAdapter {
            tapctl_path,
            base_dir,
        }
    }

    pub fn ensure_adapter_exists(&self) -> Result<(), String> {
        // First check if adapter exists without requiring admin
        let existing = self.list_adapters()?;
        if !existing.is_empty() {
            println!("TAP adapter already exists");
            return Ok(());
        }

        // No adapter found, now check if we have admin rights
        if !is_elevated::is_elevated() {
            return Err("No TAP adapter found. Please run the application as administrator to set up the VPN adapter.".to_string());
        }

        // Try to create adapter
        println!("No TAP adapter found, attempting to create one...");
        match self.create_adapter() {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Failed to create adapter: {}", e);
                println!("Attempting to install OpenVPN...");
                self.install_openvpn()?;
                // Try creating adapter again after OpenVPN install
                self.create_adapter()
            }
        }
    }

    fn check_tap_driver_installed(&self) -> Result<bool, String> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        match hklm.open_subkey(NETWORK_ADAPTERS_KEY) {
            Ok(adapters) => {
                for i in 0..1000 {
                    let subkey_name = format!("{:04}", i);
                    if let Ok(subkey) = adapters.open_subkey(&subkey_name) {
                        if let Ok(component_id) = subkey.get_value::<String, _>("ComponentId") {
                            println!("Found component ID: {}", component_id);
                            if component_id.to_lowercase() == TAP_WINDOWS_COMPONENT_ID.to_lowercase() {
                                return Ok(true);
                            }
                        }
                    }
                }
                println!("No TAP driver found in registry");
                Ok(false)
            }
            Err(e) => Err(format!("Failed to check TAP driver: {}", e)),
        }
    }

    fn install_openvpn(&self) -> Result<(), String> {
        // No need to check admin here as it's checked in ensure_adapter_exists
        let arch = std::env::consts::ARCH;
        let installer_name = match arch {
            "x86_64" => "OpenVPN-2.6.12-I001-amd64.msi",
            "aarch64" => "OpenVPN-2.6.12-I001-arm64.msi",
            _ => return Err(format!("Unsupported architecture: {}", arch)),
        };

        let installer_path = self.base_dir.join(installer_name);
        if !installer_path.exists() {
            return Err(format!("OpenVPN installer not found at {:?}", installer_path));
        }

        println!("Running OpenVPN installer from: {:?}", installer_path);

        // Start the installer process
        let mut child = Command::new("msiexec")
            .args(&["/i", &installer_path.to_string_lossy(), "/quiet", "/qn", "/norestart"])
            .spawn()
            .map_err(|e| format!("Failed to start OpenVPN installer: {}", e))?;

        // Wait for up to 60 seconds
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(60);

        while start.elapsed() < timeout {
            // Check if the TAP driver is installed
            if self.check_tap_driver_installed()? {
                println!("TAP driver detected, installation successful");
                // Try to terminate the installer gracefully
                let _ = child.kill();
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        // If we get here, kill the process and return success anyway
        // since the TAP driver might still have been installed
        println!("Installation timeout reached, attempting to proceed...");
        let _ = child.kill();
        let _ = Command::new("taskkill")
            .args(&["/F", "/IM", "msiexec.exe"])
            .output();

        Ok(())
    }

    fn list_adapters(&self) -> Result<Vec<String>, String> {
        // This can run without admin privileges
        println!("Listing TAP adapters using: {:?}", self.tapctl_path);
        let output = Command::new(&self.tapctl_path)
            .arg("list")
            .output()
            .map_err(|e| format!("Failed to execute tapctl: {}", e))?;

        println!("tapctl list output: {}", String::from_utf8_lossy(&output.stdout));
        println!("tapctl list errors: {}", String::from_utf8_lossy(&output.stderr));

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        Ok(output_str
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(String::from)
            .collect())
    }

    fn create_adapter(&self) -> Result<(), String> {
        // No need to check admin here as it's checked in ensure_adapter_exists
        println!("Creating TAP adapter using: {:?}", self.tapctl_path);
        let output = Command::new(&self.tapctl_path)
            .arg("create")
            .arg("--name")
            .arg("GekkoVPN")
            .output()
            .map_err(|e| format!("Failed to create TAP adapter: {}", e))?;

        println!("Creation output: {}", String::from_utf8_lossy(&output.stdout));
        println!("Creation errors: {}", String::from_utf8_lossy(&output.stderr));

        if !output.status.success() {
            return Err(format!(
                "Failed to create TAP adapter: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Verify the adapter was created
        std::thread::sleep(std::time::Duration::from_secs(2));
        let adapters = self.list_adapters()?;
        if adapters.is_empty() {
            return Err("TAP adapter creation seemed to succeed but no adapter is present.".to_string());
        }

        println!("TAP adapter created successfully");
        Ok(())
    }

    pub fn cleanup(&self) -> Result<(), String> {
        // No need to check admin here as this is only called in tests
        let adapters = self.list_adapters()?;
        for adapter in adapters {
            println!("Removing TAP adapter: {}", adapter);
            let output = Command::new(&self.tapctl_path)
                .arg("delete")
                .arg(&adapter)
                .output()
                .map_err(|e| format!("Failed to remove TAP adapter: {}", e))?;

            if !output.status.success() {
                println!(
                    "Warning: Failed to remove TAP adapter: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_tap_adapter_management() {
        let base_dir = env::current_dir().unwrap();
        let tap = TapAdapter::new(base_dir);

        // Ensure we can create an adapter
        tap.ensure_adapter_exists().unwrap();

        // List should show at least one adapter
        let adapters = tap.list_adapters().unwrap();
        assert!(!adapters.is_empty());

        // Clean up after test
        tap.cleanup().unwrap();
    }
}