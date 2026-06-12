use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use tiny_http::{Header, Method, Response, Server};

const PORT: u16 = 8765;
const HOST: &str = "127.0.0.1";

// ─── Config paths ──────────────────────────────────────────────────────────

fn mixxx_config_dir() -> PathBuf {
    // Linux embedded
    let linux = dirs_home().join(".mixxx");
    if linux.is_dir() {
        return linux;
    }
    // macOS dev
    let mac = dirs_home()
        .join("Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx");
    if mac.is_dir() {
        return mac;
    }
    linux
}

fn dirs_home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()))
}

fn mixxx_cfg_path() -> PathBuf {
    mixxx_config_dir().join("mixxx.cfg")
}

fn sound_cfg_path() -> PathBuf {
    mixxx_config_dir().join("soundconfig.xml")
}

fn controllers_dirs() -> Vec<PathBuf> {
    let system = PathBuf::from(if cfg!(target_os = "macos") {
        "/Applications/Mixxx.app/Contents/Resources/controllers"
    } else {
        "/usr/share/mixxx/controllers"
    });
    let user = mixxx_config_dir().join("controllers");
    vec![system, user]
}

// ─── Audio ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AudioCard {
    index: usize,
    name: String,
    pcms: Vec<String>,
}

fn get_audio_cards() -> Vec<AudioCard> {
    let mut cards = Vec::new();
    let Ok(content) = fs::read_to_string("/proc/asound/cards") else {
        return cards;
    };
    for line in content.lines() {
        // Format: "  0 [HD     ]: HDA-Intel - HDA Intel PCH"
        if let Some(rest) = line.trim_start().chars().next().filter(|c| c.is_ascii_digit()).map(|_| line.trim()) {
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() < 2 { continue; }
            let idx_part = parts[0].trim();
            let idx: usize = idx_part.split_whitespace().next()
                .and_then(|s| s.parse().ok()).unwrap_or(0);
            let name = parts[1].trim().to_string();
            cards.push(AudioCard {
                pcms: vec![format!("hw:{},0", idx)],
                index: idx,
                name,
            });
        }
    }
    cards
}

// ─── Sound config XML ──────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Default)]
struct SoundOutput {
    #[serde(rename = "@type")]
    r#type: String,
    #[serde(rename = "@channel")]
    channel: String,
    #[serde(rename = "@channel_count")]
    channel_count: String,
    #[serde(rename = "@index")]
    index: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct SoundDevice {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "@portAudioIndex")]
    port_audio_index: String,
    #[serde(default, rename = "output")]
    outputs: Vec<SoundOutput>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename = "SoundManagerConfig")]
struct SoundConfig {
    #[serde(rename = "@api")]
    api: String,
    #[serde(rename = "@samplerate")]
    samplerate: String,
    #[serde(rename = "@latency")]
    latency: String,
    #[serde(rename = "@deck_count")]
    deck_count: String,
    #[serde(rename = "@force_network_clock", default)]
    force_network_clock: String,
    #[serde(rename = "@sync_buffers", default)]
    sync_buffers: String,
    #[serde(default, rename = "SoundDevice")]
    devices: Vec<SoundDevice>,
}

fn get_sound_config() -> Result<SoundConfig, String> {
    let path = sound_cfg_path();
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
    // Strip DOCTYPE
    let xml = content
        .lines()
        .filter(|l| !l.trim_start().starts_with("<!DOCTYPE"))
        .collect::<Vec<_>>()
        .join("\n");
    quick_xml::de::from_str(&xml).map_err(|e| e.to_string())
}

fn write_sound_config(cfg: &SoundConfig) -> Result<(), String> {
    let mut xml = String::from("<!DOCTYPE SoundManagerConfig>\n");
    let body = quick_xml::se::to_string(cfg).map_err(|e| e.to_string())?;
    xml.push_str(&body);
    fs::write(sound_cfg_path(), xml).map_err(|e| e.to_string())
}

// ─── MIDI devices ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MidiDevice {
    id: u32,
    name: String,
    device_type: String,
}

fn get_midi_devices() -> Vec<MidiDevice> {
    let mut devices = Vec::new();
    let Ok(content) = fs::read_to_string("/proc/asound/seq/clients") else {
        return devices;
    };

    let mut current_id: Option<u32> = None;
    let mut current_name = String::new();
    let mut current_type = String::new();

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Client") {
            // "Client  16 : \"Hercules\" [type=User]"
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() < 2 { continue; }
            let id: u32 = parts[0].trim().parse().unwrap_or(0);
            if id <= 1 { continue; } // skip System
            // flush previous
            if let Some(prev_id) = current_id {
                devices.push(MidiDevice { id: prev_id, name: current_name.clone(), device_type: current_type.clone() });
            }
            current_id = Some(id);
            current_name = parts[1]
                .trim()
                .trim_matches('"')
                .split('"').next().unwrap_or("").trim_matches('"').to_string();
            // Extract name between quotes
            if let (Some(a), Some(b)) = (parts[1].find('"'), parts[1].rfind('"')) {
                if a != b { current_name = parts[1][a+1..b].to_string(); }
            }
            current_type = String::new();
        } else if line.trim().starts_with("Type") {
            current_type = line.split('=').nth(1).unwrap_or("").trim().to_string();
        }
    }
    if let Some(id) = current_id {
        devices.push(MidiDevice { id, name: current_name, device_type: current_type });
    }
    devices
}

// ─── MIDI mappings ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct MidiMapping {
    file: String,
    path: String,
    system: bool,
}

fn get_midi_mappings() -> Vec<MidiMapping> {
    let mut mappings = Vec::new();
    for (i, dir) in controllers_dirs().iter().enumerate() {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".midi.xml"))
            .collect();
        files.sort_by_key(|e| e.file_name());
        for entry in files {
            mappings.push(MidiMapping {
                file: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                system: i == 0,
            });
        }
    }
    mappings
}

// ─── mixxx.cfg parser ──────────────────────────────────────────────────────

type CfgMap = HashMap<String, HashMap<String, String>>;

fn parse_mixxx_cfg() -> CfgMap {
    let mut cfg: CfgMap = HashMap::new();
    let Ok(content) = fs::read_to_string(mixxx_cfg_path()) else { return cfg };
    let mut section = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed[1..trimmed.len()-1].to_string();
            cfg.entry(section.clone()).or_default();
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') && !section.is_empty() {
            let mut parts = trimmed.splitn(2, ' ');
            let key = parts.next().unwrap_or("").to_string();
            let val = parts.next().unwrap_or("").to_string();
            cfg.entry(section.clone()).or_default().insert(key, val);
        }
    }
    cfg
}

fn write_mixxx_cfg(cfg: &CfgMap) -> Result<(), String> {
    let mut out = String::new();
    for (section, entries) in cfg {
        out.push_str(&format!("[{}]\n", section));
        for (k, v) in entries {
            if v.is_empty() {
                out.push_str(&format!("{}\n", k));
            } else {
                out.push_str(&format!("{} {}\n", k, v));
            }
        }
        out.push('\n');
    }
    fs::write(mixxx_cfg_path(), out).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct MidiConfig {
    presets: HashMap<String, String>,
    enabled: HashMap<String, String>,
}

fn get_midi_config() -> MidiConfig {
    let cfg = parse_mixxx_cfg();
    MidiConfig {
        presets: cfg.get("ControllerPreset").cloned().unwrap_or_default(),
        enabled: cfg.get("Controller").cloned().unwrap_or_default(),
    }
}

#[derive(Deserialize)]
struct MidiConfigWrite {
    presets: HashMap<String, String>,
    enabled: HashMap<String, String>,
}

fn write_midi_config(data: MidiConfigWrite) -> Result<(), String> {
    let mut cfg = parse_mixxx_cfg();
    cfg.insert("ControllerPreset".to_string(), data.presets);
    cfg.insert("Controller".to_string(), data.enabled);
    write_mixxx_cfg(&cfg)
}

// ─── Restart ───────────────────────────────────────────────────────────────

fn restart_mixxx() -> Value {
    let result = Command::new("systemctl")
        .args(["--user", "restart", "mixxx"])
        .output();
    match result {
        Ok(out) if out.status.success() => {
            serde_json::json!({"ok": true, "method": "systemctl"})
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr).to_string();
            serde_json::json!({"ok": false, "error": err})
        }
        Err(_) => {
            // macOS fallback
            let _ = Command::new("pkill").args(["-x", "mixxx"]).output();
            serde_json::json!({"ok": true, "method": "pkill", "note": "macOS fallback"})
        }
    }
}

// ─── HTTP helpers ──────────────────────────────────────────────────────────

fn json_ok(data: impl Serialize) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::to_vec_pretty(&data).unwrap_or_default();
    Response::from_data(body)
        .with_status_code(200)
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
}

fn json_err(msg: &str, status: u16) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(&serde_json::json!({"error": msg})).unwrap_or_default();
    Response::from_data(body)
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
}

fn cors_preflight() -> Response<Cursor<Vec<u8>>> {
    Response::from_data(vec![])
        .with_status_code(204)
        .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Methods", "GET, POST, OPTIONS").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Headers", "Content-Type").unwrap())
}

fn read_body(request: &mut tiny_http::Request) -> Result<Value, String> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body).map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| e.to_string())
}

// ─── Main ──────────────────────────────────────────────────────────────────

fn main() {
    let addr = format!("{}:{}", HOST, PORT);
    let server = Server::http(&addr).expect("Failed to bind server");
    println!("[mixxx-config-server] Listening on http://{}", addr);
    println!("[mixxx-config-server] Mixxx config dir: {}", mixxx_config_dir().display());

    for mut request in server.incoming_requests() {
        let path = request.url().split('?').next().unwrap_or("").to_string();
        let method = request.method().clone();

        let response = match (&method, path.as_str()) {
            (Method::Options, _) => cors_preflight(),

            (Method::Get, "/api/status") => json_ok(serde_json::json!({
                "ok": true,
                "config_dir": mixxx_config_dir().to_string_lossy()
            })),

            (Method::Get, "/api/audio/cards") => json_ok(get_audio_cards()),

            (Method::Get, "/api/audio/config") => match get_sound_config() {
                Ok(cfg) => json_ok(cfg),
                Err(e) => json_err(&e, 500),
            },

            (Method::Post, "/api/audio/config") => match read_body(&mut request) {
                Ok(body) => match serde_json::from_value::<SoundConfig>(body) {
                    Ok(cfg) => match write_sound_config(&cfg) {
                        Ok(_) => json_ok(serde_json::json!({"ok": true})),
                        Err(e) => json_err(&e, 500),
                    },
                    Err(e) => json_err(&e.to_string(), 400),
                },
                Err(e) => json_err(&e, 400),
            },

            (Method::Get, "/api/midi/devices") => json_ok(get_midi_devices()),
            (Method::Get, "/api/midi/mappings") => json_ok(get_midi_mappings()),
            (Method::Get, "/api/midi/config") => json_ok(get_midi_config()),

            (Method::Post, "/api/midi/config") => match read_body(&mut request) {
                Ok(body) => match serde_json::from_value::<MidiConfigWrite>(body) {
                    Ok(data) => match write_midi_config(data) {
                        Ok(_) => json_ok(serde_json::json!({"ok": true})),
                        Err(e) => json_err(&e, 500),
                    },
                    Err(e) => json_err(&e.to_string(), 400),
                },
                Err(e) => json_err(&e, 400),
            },

            (Method::Post, "/api/restart") => json_ok(restart_mixxx()),

            _ => json_err(&format!("Unknown route: {}", path), 404),
        };

        let _ = request.respond(response);
    }
}
