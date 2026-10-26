use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{Manager, State};

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherPreferences {
    pub game_dir: String,
    pub player_name: String,
    pub steam_id: String,
    pub is_host: bool,
    pub host_ip: String,
}

impl Default for LauncherPreferences {
    fn default() -> Self {
        Self {
            game_dir: detect_default_game_directory(),
            player_name: std::env::var("USERNAME").unwrap_or_else(|_| "Player".to_string()),
            steam_id: generate_random_steam_id(),
            is_host: true,
            host_ip: detect_radmin_ip_internal(),
        }
    }
}

pub struct AppState {
    pub child: Mutex<Option<Child>>,
}

fn get_config_path() -> PathBuf {
    std::env::current_exe()
        .map(|p| p.parent().unwrap_or(Path::new(".")).join("launcher_config.json"))
        .unwrap_or_else(|_| PathBuf::from("launcher_config.json"))
}

fn detect_default_game_directory() -> String {
    let candidates = [
        r"C:\Games\AOE2\Game",
        r"D:\Games\AOE2\Game",
        r"E:\Games\AOE2\Game",
    ];

    for path in candidates {
        if Path::new(path).join("AoE2DE_s.exe").exists() {
            return path.to_string();
        }
    }
    String::new()
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MibIpAddrRow {
    dw_addr: u32,
    dw_index: u32,
    dw_mask: u32,
    dw_bcast_addr: u32,
    dw_reasm_size: u32,
    unused1: u16,
    w_type: u16,
}

#[link(name = "iphlpapi")]
unsafe extern "system" {
    fn GetIpAddrTable(pIpAddrTable: *mut u8, pdwSize: *mut u32, bOrder: i32) -> u32;
}

fn get_local_ipv4_addresses() -> Vec<String> {
    let mut size: u32 = 0;
    unsafe {
        GetIpAddrTable(std::ptr::null_mut(), &mut size, 0);
        if size > 0 {
            let mut buffer = vec![0u8; size as usize];
            if GetIpAddrTable(buffer.as_mut_ptr(), &mut size, 0) == 0 {
                let num_entries = *(buffer.as_ptr() as *const u32) as usize;
                let rows_ptr = buffer.as_ptr().add(4) as *const MibIpAddrRow;
                let mut ips = Vec::new();
                for i in 0..num_entries {
                    let row = *rows_ptr.add(i);
                    let bytes = row.dw_addr.to_ne_bytes();
                    let ip_str = format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3]);
                    if !ip_str.starts_with("127.")
                        && !ip_str.starts_with("169.254.")
                        && !ip_str.starts_with("0.")
                        && !ips.contains(&ip_str)
                    {
                        ips.push(ip_str);
                    }
                }
                if !ips.is_empty() {
                    ips.sort_by(|a, b| {
                        let a_radmin = a.starts_with("26.");
                        let b_radmin = b.starts_with("26.");
                        b_radmin.cmp(&a_radmin)
                    });
                    if !ips.contains(&"127.0.0.1".to_string()) {
                        ips.push("127.0.0.1".to_string());
                    }
                    return ips;
                }
            }
        }
    }
    vec!["127.0.0.1".to_string()]
}

fn detect_radmin_ip_internal() -> String {
    let ips = get_local_ipv4_addresses();
    for ip in &ips {
        if ip.starts_with("26.") {
            return ip.clone();
        }
    }
    ips.first().cloned().unwrap_or_else(|| "127.0.0.1".to_string())
}

fn generate_random_steam_id() -> String {
    let mut rng = rand::thread_rng();
    let num: u32 = rng.gen_range(100000000..999999999);
    format!("76561198{}", num)
}

const EMBEDDED_TOOLS: &[u8] = include_bytes!("../tools.zip");

fn extract_embedded_tools(target_dir: &Path) {
    if let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(EMBEDDED_TOOLS)) {
        for i in 0..archive.len() {
            let mut file = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let raw_name = file.name().replace('\\', "/");
            let path = PathBuf::from(raw_name);
            let mut clean_path = PathBuf::new();
            for comp in path.components() {
                if let std::path::Component::Normal(c) = comp {
                    clean_path.push(c);
                }
            }
            if clean_path.as_os_str().is_empty() {
                continue;
            }
            let outpath = target_dir.join(&clean_path);
            if file.is_dir() {
                let _ = fs::create_dir_all(&outpath);
            } else {
                if let Some(parent) = outpath.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(mut outfile) = fs::File::create(&outpath) {
                    let _ = std::io::copy(&mut file, &mut outfile);
                }
            }
        }
    }
}

fn resolve_tools_dir() -> PathBuf {
    let base_dir = std::env::current_exe()
        .map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
        .unwrap_or_else(|_| PathBuf::from("."));

    let candidate1 = base_dir.join("tools");
    if candidate1.join("launcher").join("launcher.exe").exists() {
        return candidate1;
    }

    // Auto-extract embedded tools into base_dir using native in-memory zip
    extract_embedded_tools(&base_dir);

    candidate1
}

#[tauri::command]
fn load_preferences() -> LauncherPreferences {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(prefs) = serde_json::from_str::<LauncherPreferences>(&content) {
                return prefs;
            }
        }
    }
    let default_prefs = LauncherPreferences::default();
    let _ = save_preferences(default_prefs.clone());
    default_prefs
}

#[tauri::command]
fn save_preferences(prefs: LauncherPreferences) -> Result<(), String> {
    let path = get_config_path();
    let json = serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn detect_radmin_ip() -> String {
    detect_radmin_ip_internal()
}

#[tauri::command]
fn get_available_ips() -> Vec<String> {
    get_local_ipv4_addresses()
}

#[tauri::command]
fn generate_steam_id() -> String {
    generate_random_steam_id()
}

#[tauri::command]
fn validate_game_directory(path: String) -> bool {
    let p = Path::new(&path);
    p.exists() && p.join("AoE2DE_s.exe").exists()
}

#[tauri::command]
fn pick_game_directory() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select Age of Empires II: DE 'Game' Directory")
        .pick_folder()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn start_game(prefs: LauncherPreferences, state: State<'_, AppState>) -> Result<(), String> {
    if prefs.game_dir.trim().is_empty() {
        return Err("Please select your game 'Game' folder containing AoE2DE_s.exe first.".to_string());
    }
    let game_path = Path::new(&prefs.game_dir);
    if !game_path.join("AoE2DE_s.exe").exists() {
        return Err(format!(
            "AoE2DE_s.exe not found in '{}'. Please select the 'Game' folder.",
            prefs.game_dir
        ));
    }

    // 1. Resolve tools directory
    let tools_dir = resolve_tools_dir();
    let launcher_dir = tools_dir.join("launcher");
    let launcher_exe = launcher_dir.join("launcher.exe");

    if !launcher_exe.exists() {
        return Err("launcher.exe is missing (quarantined by Windows Defender). Please add the launcher folder to Defender Exclusions.".to_string());
    }

    if prefs.is_host {
        let server_exe = tools_dir.join("server").join("server.exe");
        if !server_exe.exists() {
            return Err("server.exe is missing (quarantined by Windows Defender as Bearfoos.A!ml). Please open Windows Security -> Protection history -> find server.exe -> Allow on device.".to_string());
        }
        let bsm_exe = tools_dir.join("battle-server-manager").join("battle-server-manager.exe");
        if !bsm_exe.exists() {
            return Err("battle-server-manager.exe is missing (quarantined by Windows Defender). Please restore it from Windows Security.".to_string());
        }
    }

    // 2. Update Goldberg steam_settings/configs.user.ini & persona files
    let settings_dir = game_path.join("steam_settings");
    let _ = fs::create_dir_all(&settings_dir);
    let user_ini_content = format!(
        "[user::general]\naccount_name={}\npersona_name={}\naccount_steamid={}\nlanguage=english\nip_country=\n",
        prefs.player_name, prefs.player_name, prefs.steam_id
    );
    let _ = fs::write(settings_dir.join("configs.user.ini"), &user_ini_content);
    let _ = fs::write(settings_dir.join("force_persona_name.txt"), &prefs.player_name);
    let _ = fs::write(settings_dir.join("force_account_name.txt"), &prefs.player_name);
    let _ = fs::write(settings_dir.join("force_steamid.txt"), &prefs.steam_id);

    // Prevent invisible units: disable UHD DLC 1039811 if graphics-uhd folder is missing
    let app_ini = settings_dir.join("configs.app.ini");
    let uhd_exists = game_path.join("resources").join("_common").join("drs").join("graphics-uhd").exists();
    if !uhd_exists && app_ini.exists() {
        if let Ok(content) = fs::read_to_string(&app_ini) {
            if content.contains("unlock_all=1") {
                let fixed = content.replace("unlock_all=1", "unlock_all=0\n1389240=Lords of the West\n1557210=Dawn of the Dukes\n1869820=Dynasties of India\n2141580=Return of Rome\n2555420=The Mountain Royals\n2805510=Victors and Vanquished\n2805520=Chronicles: Battle for Greece");
                let _ = fs::write(&app_ini, fixed);
            }
        }
    }

    // 3. Fix ColdClientLoader.ini Exe=AoE2DE_s.exe
    let loader_ini = game_path.join("ColdClientLoader.ini");
    if loader_ini.exists() {
        if let Ok(content) = fs::read_to_string(&loader_ini) {
            let re = Regex::new(r"(?m)^Exe\s*=.*$").unwrap();
            let new_content = re.replace_all(&content, "Exe=AoE2DE_s.exe");
            let _ = fs::write(&loader_ini, new_content.as_bytes());
        }
    }

    // 4. Update tools/launcher/resources/config.age2.toml
    let loader_exe = game_path.join("steamclient_loader_x64.exe");
    let game_exe_str = if loader_exe.exists() {
        loader_exe.to_string_lossy().to_string()
    } else {
        game_path.join("AoE2DE_s.exe").to_string_lossy().to_string()
    };

    let launcher_config_toml = launcher_dir.join("resources").join("config.age2.toml");
    if launcher_config_toml.exists() {
        if let Ok(mut text) = fs::read_to_string(&launcher_config_toml) {
            let re_exe = Regex::new(r"(?m)^Executable\s*=.*$").unwrap();
            let re_path = Regex::new(r"(?m)^Path\s*=.*$").unwrap();
            text = re_exe.replace(&text, format!("Executable = '{}'", game_exe_str)).to_string();
            text = re_path.replace(&text, format!("Path = '{}'", prefs.game_dir)).to_string();
            let _ = fs::write(&launcher_config_toml, text);
        }
    }

    // 5. If Host, update battle-server-manager and server configs
    if prefs.is_host {
        let bsm_config_toml = tools_dir.join("battle-server-manager").join("resources").join("config.age2.toml");
        if bsm_config_toml.exists() {
            if let Ok(mut text) = fs::read_to_string(&bsm_config_toml) {
                let re_host = Regex::new(r"(?m)^Host\s*=.*$").unwrap();
                let re_path = Regex::new(r"(?m)^Path\s*=.*$").unwrap();
                let bserver_exe = game_path.join("BattleServer").join("BattleServer.exe").to_string_lossy().to_string();
                let escaped_bserver = bserver_exe.replace('\\', "\\\\");
                text = re_host.replace(&text, format!("Host = '{}'", prefs.host_ip)).to_string();
                text = re_path.replace(&text, format!("Path = '{}'", escaped_bserver)).to_string();
                let _ = fs::write(&bsm_config_toml, text);
            }
        }

        let server_config_toml = tools_dir.join("server").join("resources").join("config").join("config.toml");
        if server_config_toml.exists() {
            if let Ok(mut text) = fs::read_to_string(&server_config_toml) {
                let re_hosts = Regex::new(r"(\[Games\.age2\][\s\S]*?Hosts\s*=\s*)\[.*?\]").unwrap();
                text = re_hosts.replace(&text, format!("$1['{}', '127.0.0.1']", prefs.host_ip)).to_string();
                let _ = fs::write(&server_config_toml, text);
            }
        }
    }

    // 6. Stop any dangling instances first
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "launcher.exe", "/IM", "server.exe", "/IM", "BattleServer.exe", "/IM", "agent.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    std::thread::sleep(std::time::Duration::from_millis(500));

    // 7. Spawn Luskaner's launcher.exe
    let mut cmd = Command::new(&launcher_exe);
    cmd.current_dir(&launcher_dir);
    cmd.arg("-e").arg("age2");
    cmd.arg("--serverSingleAutoSelect");
    cmd.arg("--log");

    if prefs.is_host {
        cmd.arg("-a").arg("true");
    } else if !prefs.host_ip.is_empty() {
        cmd.arg("-s").arg(&prefs.host_ip);
    }

    cmd.creation_flags(CREATE_NO_WINDOW);

    let child = cmd.spawn().map_err(|e| format!("Failed to spawn launcher.exe: {}", e))?;

    let mut lock = state.child.lock().unwrap();
    *lock = Some(child);

    // Save preferences
    let _ = save_preferences(prefs);

    Ok(())
}

#[repr(C)]
#[allow(non_snake_case)]
struct PROCESSENTRY32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: usize,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; 260],
}

unsafe extern "system" {
    fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut std::ffi::c_void;
    fn Process32FirstW(hSnapshot: *mut std::ffi::c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn Process32NextW(hSnapshot: *mut std::ffi::c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

fn is_process_running(process_name: &str) -> bool {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(0x00000002, 0); // TH32CS_SNAPPROCESS
        if snapshot as isize == -1 || snapshot.is_null() {
            return false;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            cntUsage: 0,
            th32ProcessID: 0,
            th32DefaultHeapID: 0,
            th32ModuleID: 0,
            cntThreads: 0,
            th32ParentProcessID: 0,
            pcPriClassBase: 0,
            dwFlags: 0,
            szExeFile: [0; 260],
        };
        let mut found = false;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            let target = process_name.to_lowercase();
            loop {
                let null_pos = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..null_pos]).to_lowercase();
                if name == target {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        found
    }
}

#[tauri::command]
fn stop_game(state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut lock = state.child.lock().unwrap();
        if let Some(mut child) = lock.take() {
            let _ = child.kill();
        }
    }

    // Force terminate any lingering instances
    let _ = Command::new("taskkill")
        .args([
            "/F",
            "/IM",
            "launcher.exe",
            "/IM",
            "server.exe",
            "/IM",
            "BattleServer.exe",
            "/IM",
            "battle-server-manager.exe",
            "/IM",
            "agent.exe",
            "/IM",
            "AoE2DE_s.exe",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    // Revert hosts and certs via config.exe
    let prefs = load_preferences();
    if !prefs.game_dir.is_empty() {
        let tools_dir = resolve_tools_dir();
        let config_exe = tools_dir.join("launcher").join("bin").join("config.exe");
        if config_exe.exists() {
            let _ = Command::new(&config_exe)
                .args(["revert", "-e", "age2", "-i", "-l", "-s", "--gamePath", &prefs.game_dir])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }

    Ok(())
}

#[tauri::command]
fn is_game_running(state: State<'_, AppState>) -> bool {
    let mut lock = state.child.lock().unwrap();
    if let Some(child) = lock.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) => {
                *lock = None;
            }
            Ok(None) => return true,
            Err(_) => {
                *lock = None;
            }
        }
    }

    is_process_running("AoE2DE_s.exe") || is_process_running("launcher.exe")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        let base_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(Path::new(".")).to_path_buf())
            .unwrap_or_else(|_| PathBuf::from("."));
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", info)
        };
        let _ = fs::write(
            base_dir.join("crash.log"),
            format!("PANIC: {}\nLocation: {:?}\n", msg, info.location()),
        );
    }));

    // Pre-extract embedded tools immediately before initializing Tauri window runtime
    let _ = resolve_tools_dir();

    tauri::Builder::default()
        .setup(|_app| {
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            child: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            load_preferences,
            save_preferences,
            detect_radmin_ip,
            get_available_ips,
            generate_steam_id,
            validate_game_directory,
            pick_game_directory,
            start_game,
            stop_game,
            is_game_running
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                let _ = stop_game(state);
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract() {
        let temp_dir = std::env::temp_dir().join("test_extract_tools");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        extract_embedded_tools(&temp_dir);
        let target = temp_dir.join("tools").join("launcher").join("launcher.exe");
        let exists = target.exists();
        let _ = fs::remove_dir_all(&temp_dir);
        assert!(exists, "Expected tools/launcher/launcher.exe to exist");
    }
}
