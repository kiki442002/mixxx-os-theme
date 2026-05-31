#!/usr/bin/env python3
"""
mixxx-config-server.py
Serveur HTTP léger pour configurer Mixxx depuis une UI web ou un controller JS.

Endpoints:
  GET  /api/audio/cards          → liste des cartes ALSA disponibles
  GET  /api/audio/config         → config audio actuelle (soundconfig.xml)
  POST /api/audio/config         → écrit soundconfig.xml
  GET  /api/midi/devices         → devices MIDI connectés (/proc/asound/seq/clients)
  GET  /api/midi/mappings        → fichiers .midi.xml disponibles
  GET  /api/midi/config          → config MIDI actuelle (mixxx.cfg)
  POST /api/midi/config          → écrit [ControllerPreset] + [Controller] dans mixxx.cfg
  POST /api/restart              → redémarre Mixxx via systemctl
  GET  /api/status               → état du serveur
"""

import json
import os
import re
import subprocess
import xml.etree.ElementTree as ET
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import urlparse

# ─── Configuration ────────────────────────────────────────────────────────────

PORT = 8765
HOST = "127.0.0.1"

# Dossier de config Mixxx (Linux embarqué)
MIXXX_CONFIG_DIR = os.path.expanduser("~/.mixxx")

# Sur macOS (dev)
if not os.path.isdir(MIXXX_CONFIG_DIR):
    mac_path = os.path.expanduser(
        "~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx"
    )
    if os.path.isdir(mac_path):
        MIXXX_CONFIG_DIR = mac_path

MIXXX_CFG = os.path.join(MIXXX_CONFIG_DIR, "mixxx.cfg")
SOUND_CFG = os.path.join(MIXXX_CONFIG_DIR, "soundconfig.xml")

# Dossiers contenant les mappings .midi.xml
MIXXX_SYSTEM_CONTROLLERS = "/usr/share/mixxx/controllers"
MIXXX_USER_CONTROLLERS = os.path.join(MIXXX_CONFIG_DIR, "controllers")

# Sur macOS (dev)
if not os.path.isdir(MIXXX_SYSTEM_CONTROLLERS):
    MIXXX_SYSTEM_CONTROLLERS = "/Applications/Mixxx.app/Contents/Resources/controllers"

SYSTEMD_SERVICE = "mixxx"


# ─── Helpers ──────────────────────────────────────────────────────────────────

def json_response(handler, data, status=200):
    body = json.dumps(data, ensure_ascii=False, indent=2).encode("utf-8")
    handler.send_response(status)
    handler.send_header("Content-Type", "application/json; charset=utf-8")
    handler.send_header("Content-Length", str(len(body)))
    handler.send_header("Access-Control-Allow-Origin", "*")
    handler.end_headers()
    handler.wfile.write(body)


def error_response(handler, message, status=500):
    json_response(handler, {"error": message}, status)


def read_body(handler):
    length = int(handler.headers.get("Content-Length", 0))
    return json.loads(handler.rfile.read(length)) if length else {}


# ─── Audio ────────────────────────────────────────────────────────────────────

def get_audio_cards():
    """Liste les cartes ALSA via pyalsa, fallback sur /proc/asound/cards."""
    try:
        import alsaaudio
        cards = []
        for i, name in enumerate(alsaaudio.cards()):
            pcms = [p for p in alsaaudio.pcms(alsaaudio.PCM_PLAYBACK) if f"hw:{i}" in p]
            cards.append({"index": i, "name": name, "pcms": pcms})
        return cards
    except ImportError:
        pass

    # Fallback /proc
    cards = []
    try:
        with open("/proc/asound/cards") as f:
            for line in f:
                m = re.match(r"\s*(\d+)\s+\[(\S+)\s*\].*?:\s*(.+)", line)
                if m:
                    cards.append({
                        "index": int(m.group(1)),
                        "name": m.group(3).strip(),
                        "pcms": [f"hw:{m.group(1)},0"]
                    })
    except FileNotFoundError:
        pass
    return cards


def get_sound_config():
    """Lit soundconfig.xml et retourne un dict."""
    if not os.path.isfile(SOUND_CFG):
        return {"error": "soundconfig.xml not found", "path": SOUND_CFG}
    try:
        tree = ET.parse(SOUND_CFG)
        root = tree.getroot()
        devices = []
        for dev in root.findall("SoundDevice"):
            outputs = []
            for out in dev.findall("output"):
                outputs.append(out.attrib)
            inputs = []
            for inp in dev.findall("input"):
                inputs.append(inp.attrib)
            devices.append({
                "name": dev.get("name"),
                "portAudioIndex": dev.get("portAudioIndex"),
                "outputs": outputs,
                "inputs": inputs,
            })
        return {
            "api": root.get("api"),
            "samplerate": root.get("samplerate"),
            "latency": root.get("latency"),
            "deck_count": root.get("deck_count"),
            "devices": devices,
        }
    except Exception as e:
        return {"error": str(e)}


def write_sound_config(data):
    """
    Écrit soundconfig.xml depuis un dict.
    data = {
        "api": "ALSA",
        "samplerate": "44100",
        "latency": "6",
        "deck_count": "2",
        "devices": [
            {
                "name": "hw:1,0",
                "portAudioIndex": "1",
                "outputs": [{"type": "Master", "channel": "0", "channel_count": "2", "index": "0"}]
            }
        ]
    }
    """
    root = ET.Element("SoundManagerConfig")
    root.set("api", data.get("api", "ALSA"))
    root.set("samplerate", str(data.get("samplerate", "44100")))
    root.set("latency", str(data.get("latency", "6")))
    root.set("deck_count", str(data.get("deck_count", "2")))
    root.set("force_network_clock", "0")
    root.set("sync_buffers", "2")

    for dev in data.get("devices", []):
        dev_el = ET.SubElement(root, "SoundDevice")
        dev_el.set("name", dev["name"])
        dev_el.set("portAudioIndex", str(dev.get("portAudioIndex", "0")))
        for out in dev.get("outputs", []):
            out_el = ET.SubElement(dev_el, "output")
            for k, v in out.items():
                out_el.set(k, str(v))
        for inp in dev.get("inputs", []):
            inp_el = ET.SubElement(dev_el, "input")
            for k, v in inp.items():
                inp_el.set(k, str(v))

    tree = ET.ElementTree(root)
    ET.indent(tree, space=" ")
    with open(SOUND_CFG, "wb") as f:
        f.write(b'<!DOCTYPE SoundManagerConfig>\n')
        tree.write(f, encoding="utf-8", xml_declaration=False)


# ─── MIDI ─────────────────────────────────────────────────────────────────────

def get_midi_devices():
    """Liste les devices MIDI via /proc/asound/seq/clients."""
    devices = []
    try:
        with open("/proc/asound/seq/clients") as f:
            content = f.read()
        for block in re.split(r"(?=Client\s+\d+)", content):
            m_client = re.search(r"Client\s+(\d+)\s+:.*?\"(.+?)\"", block)
            if not m_client:
                continue
            client_id = int(m_client.group(1))
            client_name = m_client.group(2).strip()
            if client_id in (0, 1):  # ignorer System et Announce
                continue
            m_type = re.search(r"Type\s*=\s*(\S+)", block)
            client_type = m_type.group(1) if m_type else "Unknown"
            devices.append({
                "id": client_id,
                "name": client_name,
                "type": client_type,
            })
    except FileNotFoundError:
        pass
    return devices


def get_midi_mappings():
    """Liste les fichiers .midi.xml disponibles dans les dossiers controllers."""
    mappings = []
    for directory in [MIXXX_SYSTEM_CONTROLLERS, MIXXX_USER_CONTROLLERS]:
        if not os.path.isdir(directory):
            continue
        for f in sorted(os.listdir(directory)):
            if f.endswith(".midi.xml"):
                mappings.append({
                    "file": f,
                    "path": os.path.join(directory, f),
                    "system": directory == MIXXX_SYSTEM_CONTROLLERS,
                })
    return mappings


def parse_mixxx_cfg():
    """Parse mixxx.cfg en sections → dict."""
    config = {}
    current_section = None
    if not os.path.isfile(MIXXX_CFG):
        return config
    with open(MIXXX_CFG, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            m = re.match(r"^\[(.+)\]$", line)
            if m:
                current_section = m.group(1)
                config[current_section] = {}
                continue
            if current_section and line.strip() and not line.startswith("#"):
                # Format Mixxx: "key value" (espace, pas =)
                parts = line.split(None, 1)
                if len(parts) == 2:
                    config[current_section][parts[0]] = parts[1]
                elif len(parts) == 1:
                    config[current_section][parts[0]] = ""
    return config


def write_mixxx_cfg(config):
    """Écrit mixxx.cfg depuis un dict de sections."""
    lines = []
    for section, entries in config.items():
        lines.append(f"[{section}]")
        for key, value in entries.items():
            lines.append(f"{key} {value}" if value else key)
        lines.append("")
    with open(MIXXX_CFG, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


def get_midi_config():
    """Retourne [ControllerPreset] et [Controller] depuis mixxx.cfg."""
    cfg = parse_mixxx_cfg()
    return {
        "presets": cfg.get("ControllerPreset", {}),
        "enabled": cfg.get("Controller", {}),
    }


def write_midi_config(presets, enabled):
    """
    Met à jour [ControllerPreset] et [Controller] dans mixxx.cfg.
    presets = {"DeviceName": "/path/to/mapping.midi.xml", ...}
    enabled = {"DeviceName": "1", ...}
    """
    cfg = parse_mixxx_cfg()
    cfg["ControllerPreset"] = presets
    cfg["Controller"] = enabled
    write_mixxx_cfg(cfg)


# ─── Restart ──────────────────────────────────────────────────────────────────

def restart_mixxx():
    """Redémarre Mixxx via systemctl (Linux) ou pkill+relaunch (dev macOS)."""
    try:
        result = subprocess.run(
            ["systemctl", "--user", "restart", SYSTEMD_SERVICE],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode == 0:
            return {"ok": True, "method": "systemctl"}
        return {"ok": False, "error": result.stderr.strip()}
    except FileNotFoundError:
        # Fallback macOS dev
        subprocess.run(["pkill", "-x", "mixxx"], capture_output=True)
        return {"ok": True, "method": "pkill", "note": "macOS fallback"}
    except Exception as e:
        return {"ok": False, "error": str(e)}


# ─── Handler ──────────────────────────────────────────────────────────────────

class MixxxConfigHandler(BaseHTTPRequestHandler):

    ROUTES_GET = {
        "/api/status":         lambda self: {"ok": True, "config_dir": MIXXX_CONFIG_DIR},
        "/api/audio/cards":    lambda self: get_audio_cards(),
        "/api/audio/config":   lambda self: get_sound_config(),
        "/api/midi/devices":   lambda self: get_midi_devices(),
        "/api/midi/mappings":  lambda self: get_midi_mappings(),
        "/api/midi/config":    lambda self: get_midi_config(),
    }

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self):
        path = urlparse(self.path).path
        handler_fn = self.ROUTES_GET.get(path)
        if handler_fn:
            try:
                json_response(self, handler_fn(self))
            except Exception as e:
                error_response(self, str(e))
        else:
            error_response(self, f"Unknown route: {path}", 404)

    def do_POST(self):
        path = urlparse(self.path).path
        try:
            body = read_body(self)
            if path == "/api/audio/config":
                write_sound_config(body)
                json_response(self, {"ok": True})

            elif path == "/api/midi/config":
                presets = body.get("presets", {})
                enabled = body.get("enabled", {})
                write_midi_config(presets, enabled)
                json_response(self, {"ok": True})

            elif path == "/api/restart":
                result = restart_mixxx()
                json_response(self, result, 200 if result.get("ok") else 500)

            else:
                error_response(self, f"Unknown route: {path}", 404)
        except Exception as e:
            error_response(self, str(e))

    def log_message(self, fmt, *args):
        print(f"[mixxx-config-server] {self.address_string()} {fmt % args}")


# ─── Main ─────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    server = HTTPServer((HOST, PORT), MixxxConfigHandler)
    print(f"[mixxx-config-server] Listening on http://{HOST}:{PORT}")
    print(f"[mixxx-config-server] Mixxx config dir: {MIXXX_CONFIG_DIR}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[mixxx-config-server] Stopped.")
