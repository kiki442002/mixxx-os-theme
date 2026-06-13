use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiOutput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

const MIDI_PORT_NAME: &str = "mixxx-os Config";

// ─── Config paths ──────────────────────────────────────────────────────────

fn mixxx_config_dir() -> PathBuf {
    // macOS sandbox path takes priority on macOS
    if cfg!(target_os = "macos") {
        let mac = dirs_home()
            .join("Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx");
        if mac.is_dir() {
            return mac;
        }
    }
    // Linux (or macOS fallback without sandbox)
    let dot = dirs_home().join(".mixxx");
    if dot.is_dir() {
        return dot;
    }
    dirs_home().join(".mixxx")
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
    if cfg!(target_os = "macos") {
        let Ok(out) = Command::new("system_profiler")
            .args(["SPAudioDataType", "-json"])
            .output() else { return vec![] };
        let text = String::from_utf8_lossy(&out.stdout);
        let Ok(json) = serde_json::from_str::<Value>(&text) else { return vec![] };
        let mut cards = Vec::new();
        if let Some(items) = json.get("SPAudioDataType").and_then(|v| v.as_array()) {
            for (i, item) in items.iter().enumerate() {
                if let Some(name) = item.get("_name").and_then(|v| v.as_str()) {
                    cards.push(AudioCard {
                        index: i,
                        // On macOS, Mixxx usually sees devices by their name directly (Core Audio)
                        name: name.to_string(),
                        pcms: vec![format!("{}", i)],
                    });
                }
            }
        }
        cards
    } else {
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
                    // Format name for Mixxx (PortAudio ALSA)
                    name: format!("ALSA: {}: hw:{},0", name, idx),
                });
            }
        }
        cards
    }
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

#[derive(Serialize, Clone)]
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
            // For controllers, the name might contain spaces, so the value is after the LAST space
            if section == "Controller" || section == "ControllerPreset" {
                if let Some(last_space_idx) = trimmed.rfind(' ') {
                    let key = trimmed[..last_space_idx].trim().to_string();
                    let val = trimmed[last_space_idx..].trim().to_string();
                    cfg.entry(section.clone()).or_default().insert(key, val);
                } else {
                    cfg.entry(section.clone()).or_default().insert(trimmed.to_string(), "".to_string());
                }
            } else {
                let mut parts = trimmed.splitn(2, ' ');
                let key = parts.next().unwrap_or("").to_string();
                let val = parts.next().unwrap_or("").to_string();
                cfg.entry(section.clone()).or_default().insert(key, val);
            }
        }
    }
    cfg
}

fn write_mixxx_cfg(cfg: &CfgMap) -> Result<(), String> {
    let mut out = String::new();
    // Use a fixed order for sections we care about to avoid massive reordering
    let mut sections: Vec<_> = cfg.keys().collect();
    sections.sort();

    for section in sections {
        let entries = cfg.get(section).unwrap();
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
    let path = mixxx_cfg_path();
    println!("[CONFIG] Writing mixxx.cfg to {}", path.display());
    fs::write(&path, out).map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    println!("[CONFIG] mixxx.cfg updated successfully");
    Ok(())
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
    println!("[MIDI] Updating MIDI config in mixxx.cfg...");
    let mut cfg = parse_mixxx_cfg();
    cfg.insert("ControllerPreset".to_string(), data.presets);
    cfg.insert("Controller".to_string(), data.enabled);
    write_mixxx_cfg(&cfg)
}

// ─── Restart ───────────────────────────────────────────────────────────────

fn restart_mixxx() -> Value {
    // Try SysVinit /etc/init.d/weston restart via sudo
    let result = Command::new("sudo")
        .args(["service", "weston", "restart"])
        .output();
    match result {
        Ok(out) if out.status.success() => {
            serde_json::json!({"ok": true, "method": "sysvinit service"})
        }
        Ok(out) => {
            // Try direct path fallback
            let res2 = Command::new("sudo")
                .args(["/etc/init.d/weston", "restart"])
                .output();
            match res2 {
                Ok(out2) if out2.status.success() => {
                    serde_json::json!({"ok": true, "method": "sysvinit direct"})
                }
                _ => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    serde_json::json!({"ok": false, "error": err})
                }
            }
        }
        Err(_) => {
            // macOS fallback
            let _ = Command::new("pkill").args(["-x", "mixxx"]).output();
            serde_json::json!({"ok": true, "method": "pkill", "note": "macOS fallback"})
        }
    }
}

// ─── Physical MIDI inputs (midir port enumeration) ────────────────────────

fn get_physical_midi_inputs() -> Vec<String> {
    let Ok(midi_in) = MidiInput::new("mixxx-os-enum") else { return vec![] };
    let ports = midi_in.ports();
    ports.iter().filter_map(|p| {
        let name = midi_in.port_name(p).ok()?;
        if name.contains(MIDI_PORT_NAME) { return None; }
        Some(name)
    }).collect()
}

// ─── Audio output devices ─────────────────────────────────────────────────

fn get_audio_output_devices() -> Vec<AudioCard> {
    if cfg!(target_os = "macos") {
        let Ok(out) = Command::new("system_profiler")
            .args(["SPAudioDataType", "-json"])
            .output() else { return get_audio_cards() };
        let text = String::from_utf8_lossy(&out.stdout);
        let Ok(json) = serde_json::from_str::<Value>(&text) else { return get_audio_cards() };
        let mut cards = Vec::new();
        if let Some(items) = json.get("SPAudioDataType").and_then(|v| v.as_array()) {
            for (i, item) in items.iter().enumerate() {
                if let Some(name) = item.get("_name").and_then(|v| v.as_str()) {
                    cards.push(AudioCard {
                        index: i,
                        name: name.to_string(),
                        pcms: vec![format!("{}", i)],
                    });
                }
            }
        }
        if cards.is_empty() { get_audio_cards() } else { cards }
    } else {
        get_audio_cards()
    }
}

#[derive(Deserialize)]
struct SetOutputRequest {
    output_type: String,   // "master" or "headphone"
    device_name: String,
    port_audio_index: String,
}

fn set_audio_output(req: SetOutputRequest) -> Result<(), String> {
    let mut cfg = get_sound_config().unwrap_or(SoundConfig {
        api: "ALSA".to_string(),
        samplerate: "44100".to_string(),
        latency: "5".to_string(),
        deck_count: "2".to_string(),
        force_network_clock: "0".to_string(),
        sync_buffers: "2".to_string(),
        devices: vec![],
    });
    // Remove this output type from all existing devices
    for dev in &mut cfg.devices {
        dev.outputs.retain(|o| o.r#type != req.output_type);
    }
    cfg.devices.retain(|d| !d.outputs.is_empty());

    let new_output = SoundOutput {
        r#type: req.output_type,
        channel: "0".to_string(),
        channel_count: "2".to_string(),
        index: "0".to_string(),
    };
    if let Some(dev) = cfg.devices.iter_mut().find(|d| d.name == req.device_name) {
        dev.outputs.push(new_output);
    } else {
        cfg.devices.push(SoundDevice {
            name: req.device_name,
            port_audio_index: req.port_audio_index,
            outputs: vec![new_output],
        });
    }
    write_sound_config(&cfg)
}

// ─── MIDI protocol constants ─────────────────────────────────────────────────
//
// CCs:  server → Mixxx (channel 1, status 0xB0)
//   0x03  latency_idx      (0-5)
//   0x04  decks_idx        (0-1)
//   0x05  server_online    (0 or 1)
//   0x10  master_out_idx   (0-6, 127 = none)
//   0x11  headphone_out_idx(0-6, 127 = none)
//   0x12  num_output_devices (0-127)
//   0x20  num_midi_devices (0-4)
//   0x21..0x24  slot 0..3 state  (0=empty 1=inactive 2=active)
//   0x25..0x28  slot 0..3 script_idx (0-126, 127=none)
//
// Notes: Mixxx → server  (channel 1, status 0x90, velocity > 0)
//   0x01  RELOAD   (re-read config, push all CCs)
//   0x03  RESTART  (restart Mixxx)
//   0x04  SET_LATENCY   (velocity = latency_idx)
//   0x05  SET_DECKS     (velocity = decks_idx: 0=2decks 1=4decks)
//   0x10  MASTER_NEXT   (cycle to next output device)
//   0x11  HEADPHONE_NEXT
//   0x20  SLOT0_TOGGLE  (activate/deactivate device slot 0)
//   0x21  SLOT0_SCRIPT_NEXT (cycle script for slot 0)
//   0x22  SLOT1_TOGGLE
//   0x23  SLOT1_SCRIPT_NEXT
//   0x24  SLOT2_TOGGLE
//   0x25  SLOT2_SCRIPT_NEXT
//   0x26  SLOT3_TOGGLE
//   0x27  SLOT3_SCRIPT_NEXT

#[allow(dead_code)]
const MIDI_CH: u8 = 0x00; // channel 1

// ─── MIDI profiles ──────────────────────────────────────────────────────────

struct ControllerProfile {
    keyword: &'static str,
    preset_name: &'static str,
}

const KNOWN_CONTROLLERS: &[ControllerProfile] = &[
    ControllerProfile { keyword: "FLX4",       preset_name: "Pioneer-DDJ-FLX4.midi.xml" },
    ControllerProfile { keyword: "DDJ-400",     preset_name: "Pioneer-DDJ-400.midi.xml" },
    ControllerProfile { keyword: "Inpulse 500", preset_name: "Hercules-DJControl-Inpulse-500.midi.xml" },
    ControllerProfile { keyword: "Inpulse 300", preset_name: "Hercules-DJControl-Inpulse-300.midi.xml" },
    ControllerProfile { keyword: "Inpulse 200", preset_name: "Hercules-DJControl-Inpulse-200.midi.xml" },
    ControllerProfile { keyword: "S2 MK3",      preset_name: "Traktor-Kontrol-S2-MK3.midi.xml" },
    ControllerProfile { keyword: "Mixtrack Pro",preset_name: "Numark-Mixtrack-Pro-3.midi.xml" },
];

fn find_mapping_path(preset_file: &str) -> Option<String> {
    for dir in controllers_dirs() {
        let path = dir.join(preset_file);
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
struct AppState {
    output_devices: Vec<String>,
    master_idx: Option<usize>,
    headphone_idx: Option<usize>,

    midi_devices: Vec<String>,
    midi_mappings: Vec<MidiMapping>,
    device_scripts: [Option<usize>; 4],
    device_active: [bool; 4],

    latency_idx: usize,
    decks_idx: usize,
}

type MidiOut     = Arc<Mutex<Option<midir::MidiOutputConnection>>>;
type SharedState = Arc<Mutex<AppState>>;

fn update_skin_settings(audio_devices: &[String], midi_mappings: &[MidiMapping], current_midi: &[String]) {
    let Ok(template) = fs::read_to_string("skins/mixxx-os-2deck/settings.template.xml") else {
        eprintln!("[SKIN] settings.template.xml not found");
        return;
    };

    let mut audio_states = String::new();
    for (i, name) in audio_devices.iter().enumerate() {
        audio_states.push_str(&format!("                <State><Number>{}</Number><Text>{}</Text></State>\n", i, name));
    }
    audio_states.push_str(&format!("                <State><Number>{}</Number><Text>—</Text></State>\n", audio_devices.len()));

    let mut mapping_states = String::new();
    // For MIDI slots, we display the detected device name or VIDE
    for i in 0..2 { // Let's simplify to 2 slots for the UI
        let label = current_midi.get(i).cloned().unwrap_or_else(|| "VIDE".to_string());
        mapping_states.push_str(&format!("                <State><Number>{}</Number><Text>{}</Text></State>\n", i, label));
    }

    let content = template
        .replace("<!-- AUDIO_COUNT -->", &(audio_devices.len() + 1).to_string())
        .replace("<!-- AUDIO_STATES -->", &audio_states)
        .replace("<!-- SCRIPT_COUNT -->", "2")
        .replace("<!-- SCRIPT_STATES -->", &mapping_states);

    if let Err(e) = fs::write("skins/mixxx-os-2deck/settings.xml", content) {
        eprintln!("[SKIN] failed to write settings.xml: {}", e);
    }
}

fn refresh_state(s: &mut AppState) {
    let old_midi_count = s.midi_devices.len();
    
    s.output_devices = get_audio_output_devices().into_iter().map(|c| c.name).collect();

    if let Ok(mi) = MidiInput::new("mixxx-os-enum") {
        s.midi_devices = mi.ports().iter()
            .filter_map(|p| mi.port_name(p).ok())
            .filter(|n| !n.contains(MIDI_PORT_NAME))
            .take(4)
            .collect();
    }

    s.midi_mappings = get_midi_mappings();
    
    // Update the skin XML
    update_skin_settings(&s.output_devices, &s.midi_mappings, &s.midi_devices);

    if let Ok(cfg) = get_sound_config() {
        s.latency_idx = match cfg.latency.as_str() {
            "1" => 0, "2" => 1, "5" => 2, "10" => 3, "20" => 4, "46" => 5, _ => 2,
        };
        s.decks_idx = if cfg.deck_count == "4" { 1 } else { 0 };
        s.master_idx    = None;
        s.headphone_idx = None;
        for dev in &cfg.devices {
            let idx = s.output_devices.iter().position(|n| {
                n == &dev.name || n.contains(&dev.name) || dev.name.contains(n.as_str())
            });
            if let Some(i) = idx {
                for out in &dev.outputs {
                    match out.r#type.as_str() {
                        "master"    => s.master_idx    = Some(i),
                        "headphone" => s.headphone_idx = Some(i),
                        _ => {}
                    }
                }
            }
        }
    }

    // AUTO-CONFIG LOGIC
    let mut midi_cfg = get_midi_config();
    let mut changed = false;

    for i in 0..4 {
        if let Some(dev_name) = s.midi_devices.get(i) {
            // Check if this is a known controller and not yet enabled
            let is_enabled = midi_cfg.enabled.get(dev_name).map(|v| v == "1").unwrap_or(false);
            
            if !is_enabled {
                if let Some(profile) = KNOWN_CONTROLLERS.iter().find(|p| dev_name.contains(p.keyword)) {
                    if let Some(full_path) = find_mapping_path(profile.preset_name) {
                        println!("[AUTO-MIDI] Detected {} -> Auto-enabling with mapping {}", dev_name, profile.preset_name);
                        midi_cfg.enabled.insert(dev_name.clone(), "1".to_string());
                        midi_cfg.presets.insert(dev_name.clone(), full_path);
                        changed = true;
                    }
                }
            }
            
            s.device_active[i]  = midi_cfg.enabled.get(dev_name).map(|v| v == "1").unwrap_or(false);
            s.device_scripts[i] = midi_cfg.presets.get(dev_name)
                .and_then(|path| s.midi_mappings.iter().position(|m| &m.path == path));
        } else {
            s.device_active[i]  = false;
            s.device_scripts[i] = None;
        }
    }

    if changed {
        let _ = write_midi_config(MidiConfigWrite {
            presets: midi_cfg.presets,
            enabled: midi_cfg.enabled,
        });
    }
}


fn push_state_to_midi(s: &AppState, conn: &mut midir::MidiOutputConnection) {
    macro_rules! cc {
        ($c:expr, $v:expr) => { let _ = conn.send(&[0xB0, $c, $v]); };
    }
    println!("[MIDI] Pushing state to Mixxx...");
    cc!(0x05, 1_u8);
    cc!(0x03, s.latency_idx.min(5) as u8);
    cc!(0x04, s.decks_idx.min(1) as u8);
    cc!(0x12, s.output_devices.len().min(127) as u8);
    cc!(0x10, s.master_idx.map(|i| i as u8).unwrap_or(127));
    cc!(0x11, s.headphone_idx.map(|i| i as u8).unwrap_or(127));
    cc!(0x20, s.midi_devices.len().min(4) as u8);
    for i in 0..4usize {
        let present   = i < s.midi_devices.len();
        let dev_state = if !present { 0 } else if s.device_active[i] { 2 } else { 1 };
        cc!(0x21 + i as u8, dev_state);
        let script = s.device_scripts[i].map(|x| x as u8).unwrap_or(127);
        cc!(0x25 + i as u8, script);
    }
    println!("[MIDI] State pushed: latency_idx={}, master_idx={:?}, headphone_idx={:?}, outputs_count={}, midi_count={}", 
             s.latency_idx, s.master_idx, s.headphone_idx, s.output_devices.len(), s.midi_devices.len());
    for (i, dev) in s.output_devices.iter().enumerate() {
        println!("  Output {}: {}", i, dev);
    }
}

fn push_all_state(state: &SharedState, midi_out: &MidiOut) {
    let mut s = state.lock().unwrap();
    refresh_state(&mut s);
    let snap = s.clone();
    drop(s);
    let mut out = midi_out.lock().unwrap();
    if let Some(conn) = out.as_mut() {
        push_state_to_midi(&snap, conn);
    }
}

fn write_slot_to_cfg(s: &AppState, slot: usize) {
    let Some(dev_name) = s.midi_devices.get(slot) else { return };
    let midi_cfg = get_midi_config();
    let mut presets = midi_cfg.presets;
    let mut enabled = midi_cfg.enabled;
    enabled.insert(dev_name.clone(), if s.device_active[slot] { "1" } else { "0" }.to_string());
    if let Some(idx) = s.device_scripts[slot] {
        if let Some(m) = s.midi_mappings.get(idx) {
            presets.insert(dev_name.clone(), m.path.clone());
        }
    }
    let _ = write_midi_config(MidiConfigWrite { presets, enabled });
}

fn handle_note(note: u8, vel: u8, s: &mut AppState) {
    match note {
        0x01 => {
            println!("[MIDI] RELOAD");
            refresh_state(s);
        }
        0x03 => {
            println!("[MIDI] RESTART");
            let _ = restart_mixxx();
        }
        0x04 => {
            s.latency_idx = (vel as usize).min(5);
            println!("[MIDI] SET_LATENCY → {}", s.latency_idx);
            if let Ok(mut cfg) = get_sound_config() {
                cfg.latency = ["1","2","5","10","20","46"][s.latency_idx].to_string();
                let _ = write_sound_config(&cfg);
            }
        }
        0x05 => {
            s.decks_idx = if vel > 0 { 1 } else { 0 };
            println!("[MIDI] SET_DECKS → {}", if s.decks_idx == 1 { 4 } else { 2 });
            if let Ok(mut cfg) = get_sound_config() {
                cfg.deck_count = if s.decks_idx == 1 { "4" } else { "2" }.to_string();
                let _ = write_sound_config(&cfg);
            }
        }
        0x10 => {
            let n = s.output_devices.len();
            if n > 0 {
                let next = s.master_idx.map(|i| (i + 1) % n).unwrap_or(0);
                let dev  = s.output_devices[next].clone();
                s.master_idx = Some(next);
                println!("[MIDI] MASTER_NEXT → {}", dev);
                let _ = set_audio_output(SetOutputRequest {
                    output_type: "master".into(), device_name: dev, port_audio_index: "-1".into(),
                });
            }
        }
        0x11 => {
            let n = s.output_devices.len();
            if n > 0 {
                let next = s.headphone_idx.map(|i| (i + 1) % n).unwrap_or(0);
                let dev  = s.output_devices[next].clone();
                s.headphone_idx = Some(next);
                println!("[MIDI] HP_NEXT → {}", dev);
                let _ = set_audio_output(SetOutputRequest {
                    output_type: "headphone".into(), device_name: dev, port_audio_index: "-1".into(),
                });
            }
        }
        // Slot toggle/script-next: 0x20-0x27
        n @ 0x20..=0x27 => {
            let slot = ((n - 0x20) / 2) as usize;
            if n % 2 == 0 {
                // toggle active
                if slot < s.midi_devices.len() {
                    s.device_active[slot] = !s.device_active[slot];
                    println!("[MIDI] SLOT{}_TOGGLE → {}", slot, s.device_active[slot]);
                    write_slot_to_cfg(s, slot);
                }
            } else {
                // next script
                let mlen = s.midi_mappings.len();
                if slot < s.midi_devices.len() && mlen > 0 {
                    s.device_scripts[slot] = Some(
                        s.device_scripts[slot].map(|i| (i + 1) % mlen).unwrap_or(0)
                    );
                    println!("[MIDI] SLOT{}_SCRIPT_NEXT → {:?}", slot, s.device_scripts[slot]);
                    write_slot_to_cfg(s, slot);
                }
            }
        }
        _ => {}
    }
}

fn start_midi_virtual(state: SharedState, midi_out: MidiOut) {
    std::thread::spawn(move || {
        let out = match MidiOutput::new("mixxx-os Config Server") {
            Ok(o) => o,
            Err(e) => { eprintln!("[MIDI] output init error: {}", e); return; }
        };
        let conn = match out.create_virtual(MIDI_PORT_NAME) {
            Ok(c) => c,
            Err(e) => { eprintln!("[MIDI] create virtual output error: {}", e); return; }
        };
        println!("[MIDI] virtual output '{}' ready", MIDI_PORT_NAME);
        *midi_out.lock().unwrap() = Some(conn);

        // Push initial state after a short delay (let Mixxx connect)
        std::thread::sleep(std::time::Duration::from_secs(2));
        push_all_state(&state, &midi_out);

        loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
    });
}

fn start_midi_input(state: SharedState, midi_out: MidiOut) {
    std::thread::spawn(move || {
        let input = match MidiInput::new("mixxx-os Config Server") {
            Ok(i) => i,
            Err(e) => { eprintln!("[MIDI] input init error: {}", e); return; }
        };
        let _conn = match input.create_virtual(MIDI_PORT_NAME, move |_ts, msg: &[u8], _| {
            println!("[MIDI] Raw message received: {:?}", msg);
            if msg.len() < 3 || (msg[0] & 0xF0) != 0x90 || msg[2] == 0 { return; }
            let snap = {
                let mut s = state.lock().unwrap();
                handle_note(msg[1], msg[2], &mut s);
                s.clone()
            };
            let mut out = midi_out.lock().unwrap();
            if let Some(conn) = out.as_mut() {
                push_state_to_midi(&snap, conn);
            }
        }, ()) {
            Ok(c) => c,
            Err(e) => { eprintln!("[MIDI] create virtual input error: {}", e); return; }
        };
        println!("[MIDI] virtual input '{}' ready", MIDI_PORT_NAME);
        loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
    });
}

// ─── Main ──────────────────────────────────────────────────────────────────

fn main() {
    let midi_out:  MidiOut     = Arc::new(Mutex::new(None));
    let app_state: SharedState = Arc::new(Mutex::new(AppState::default()));

    start_midi_virtual(Arc::clone(&app_state), Arc::clone(&midi_out));
    start_midi_input(Arc::clone(&app_state), Arc::clone(&midi_out));

    // Hotplug Polling Thread
    let poll_state = Arc::clone(&app_state);
    let poll_midi  = Arc::clone(&midi_out);
    std::thread::spawn(move || {
        let mut last_device_list = Vec::new();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            
            // Check current MIDI ports
            let current_ports = if let Ok(mi) = MidiInput::new("mixxx-os-poll") {
                mi.ports().iter()
                    .filter_map(|p| mi.port_name(p).ok())
                    .filter(|n| !n.contains(MIDI_PORT_NAME))
                    .collect::<Vec<String>>()
            } else {
                Vec::new()
            };

            if current_ports != last_device_list {
                println!("[HOTPLUG] MIDI ports changed, refreshing state...");
                last_device_list = current_ports;
                push_all_state(&poll_state, &poll_midi);
            }
        }
    });

    println!("[mixxx-config-server] MIDI Bridge active (Hotplug enabled)");
    println!("[mixxx-config-server] Mixxx config dir: {}", mixxx_config_dir().display());

    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
