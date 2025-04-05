use std::process::Child;
use std::sync::Mutex;

pub struct VpnState {
    pub child_process: Mutex<Option<Child>>,
    pub connected_server: Mutex<Option<String>>,
}
