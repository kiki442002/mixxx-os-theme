// =============================================================================
// Mixxx OS Generic Controller Script
// Supports:
//   Preset 0 – Pioneer DDJ-400 / FLX4 / DDJ-200
//   Preset 1 – Hercules Inpulse 300 / 500
//   Preset 2 – Numark Mixtrack Platinum FX
//
// The active preset is read from [Skin],controller_preset (set via the skin
// Config tab). Changing the preset live re-sends LED state to the new device.
// =============================================================================

var MixxxOSGeneric = {};

// -----------------------------------------------------------------------------
// Preset definitions
// -----------------------------------------------------------------------------
MixxxOSGeneric.PRESETS = {
    0: {
        name: "Pioneer DDJ-400/FLX4/DDJ-200",
        jogResolution: 720,
        jogRpm: 33 + 1/3,
        jogAlpha: 1/8,
        jogBeta: (1/8) / 32,
        bendScale: 0.8,
        leds: {
            ch1: {
                play: { status: 0x90, note: 0x0B },
                cue:  { status: 0x90, note: 0x0C },
                sync: { status: 0x90, note: 0x58 }
            },
            ch2: {
                play: { status: 0x91, note: 0x0B },
                cue:  { status: 0x91, note: 0x0C },
                sync: { status: 0x91, note: 0x58 }
            }
        }
    },
    1: {
        name: "Hercules Inpulse 300/500",
        jogResolution: 128,
        jogRpm: 33 + 1/3,
        jogAlpha: 1/8,
        jogBeta: (1/8) / 32,
        bendScale: 0.5,
        leds: {
            ch1: {
                play: { status: 0x91, note: 0x07 },
                cue:  { status: 0x91, note: 0x06 },
                sync: { status: 0x91, note: 0x05 }
            },
            ch2: {
                play: { status: 0x92, note: 0x07 },
                cue:  { status: 0x92, note: 0x06 },
                sync: { status: 0x92, note: 0x05 }
            }
        }
    },
    2: {
        name: "Numark Mixtrack Platinum FX",
        jogResolution: 128,
        jogRpm: 33 + 1/3,
        jogAlpha: 1/8,
        jogBeta: (1/8) / 32,
        bendScale: 0.5,
        leds: {
            ch1: {
                play: { status: 0x90, note: 0x00 },
                cue:  { status: 0x90, note: 0x01 },
                sync: { status: 0x90, note: 0x02 }
            },
            ch2: {
                play: { status: 0x91, note: 0x00 },
                cue:  { status: 0x91, note: 0x01 },
                sync: { status: 0x91, note: 0x02 }
            }
        }
    }
};

// -----------------------------------------------------------------------------
// Runtime state
// -----------------------------------------------------------------------------
MixxxOSGeneric.currentPreset = 0;
MixxxOSGeneric.shift     = [false, false];   // shift held per deck (0=deck1, 1=deck2)
MixxxOSGeneric.touching  = [false, false];   // jog touch state per deck
MixxxOSGeneric.connections = [];             // engine connections for cleanup

// -----------------------------------------------------------------------------
// init / shutdown
// -----------------------------------------------------------------------------
MixxxOSGeneric.init = function(id) {
    var raw = engine.getValue("[Skin]", "controller_preset");
    MixxxOSGeneric.currentPreset = (raw !== undefined && !isNaN(raw)) ? Math.round(raw) : 0;

    var conn = engine.makeConnection("[Skin]", "controller_preset", MixxxOSGeneric.onPresetChange);
    if (conn) { MixxxOSGeneric.connections.push(conn); }

    for (var deck = 1; deck <= 2; deck++) {
        var group = "[Channel" + deck + "]";
        var c1 = engine.makeConnection(group, "play",          MixxxOSGeneric.makeCallback(deck, "play",  "play"));
        var c2 = engine.makeConnection(group, "cue_indicator", MixxxOSGeneric.makeCallback(deck, "cue",   "cue_indicator"));
        var c3 = engine.makeConnection(group, "sync_enabled",  MixxxOSGeneric.makeCallback(deck, "sync",  "sync_enabled"));
        if (c1) { MixxxOSGeneric.connections.push(c1); }
        if (c2) { MixxxOSGeneric.connections.push(c2); }
        if (c3) { MixxxOSGeneric.connections.push(c3); }
    }

    MixxxOSGeneric.refreshAllLEDs();
};

MixxxOSGeneric.shutdown = function(id) {
    MixxxOSGeneric.allLEDsOff();
    for (var i = 0; i < MixxxOSGeneric.connections.length; i++) {
        if (MixxxOSGeneric.connections[i]) {
            MixxxOSGeneric.connections[i].disconnect();
        }
    }
    MixxxOSGeneric.connections = [];
};

// -----------------------------------------------------------------------------
// Preset change handler
// -----------------------------------------------------------------------------
MixxxOSGeneric.onPresetChange = function(value) {
    MixxxOSGeneric.currentPreset = Math.round(value) || 0;
    MixxxOSGeneric.refreshAllLEDs();
};

// -----------------------------------------------------------------------------
// LED helpers
// -----------------------------------------------------------------------------
MixxxOSGeneric.sendLED = function(deckNum, ledName, on) {
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (!preset) { return; }
    var deckKey = "ch" + deckNum;
    var led = preset.leds[deckKey] && preset.leds[deckKey][ledName];
    if (!led) { return; }
    midi.sendShortMsg(led.status, led.note, on ? 0x7F : 0x00);
};

MixxxOSGeneric.makeCallback = function(deck, ledName, control) {
    return function(value) {
        MixxxOSGeneric.sendLED(deck, ledName, value > 0);
    };
};

MixxxOSGeneric.refreshAllLEDs = function() {
    for (var deck = 1; deck <= 2; deck++) {
        var group = "[Channel" + deck + "]";
        MixxxOSGeneric.sendLED(deck, "play", engine.getValue(group, "play")         > 0);
        MixxxOSGeneric.sendLED(deck, "cue",  engine.getValue(group, "cue_indicator") > 0);
        MixxxOSGeneric.sendLED(deck, "sync", engine.getValue(group, "sync_enabled") > 0);
    }
};

MixxxOSGeneric.allLEDsOff = function() {
    var presetKeys = Object.keys(MixxxOSGeneric.PRESETS);
    for (var p = 0; p < presetKeys.length; p++) {
        var preset = MixxxOSGeneric.PRESETS[presetKeys[p]];
        var deckKeys = ["ch1", "ch2"];
        for (var d = 0; d < deckKeys.length; d++) {
            var leds = preset.leds[deckKeys[d]];
            var ledKeys = Object.keys(leds);
            for (var l = 0; l < ledKeys.length; l++) {
                var led = leds[ledKeys[l]];
                midi.sendShortMsg(led.status, led.note, 0x00);
            }
        }
    }
};

// -----------------------------------------------------------------------------
// Jog wheels
// -----------------------------------------------------------------------------
MixxxOSGeneric.jogTouch = function(channel, control, value, status, group) {
    var deck = MixxxOSGeneric.groupToDeck(group);
    if (deck < 1) { return; }
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (!preset) { return; }

    MixxxOSGeneric.touching[deck - 1] = (value > 0);

    if (value > 0) {
        engine.scratchEnable(deck, preset.jogResolution, preset.jogRpm, preset.jogAlpha, preset.jogBeta);
    } else {
        engine.scratchDisable(deck);
    }
};

MixxxOSGeneric.jogTurn = function(channel, control, value, status, group) {
    var deck = MixxxOSGeneric.groupToDeck(group);
    if (deck < 1) { return; }
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (!preset) { return; }

    var delta = value - 64;  // center at 64; <64 = rewind, >64 = forward

    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * preset.bendScale);
    }
};

MixxxOSGeneric.groupToDeck = function(group) {
    if (group === "[Channel1]") { return 1; }
    if (group === "[Channel2]") { return 2; }
    return 0;
};

// -----------------------------------------------------------------------------
// Shift (Pioneer only)
// -----------------------------------------------------------------------------
MixxxOSGeneric.shiftPressed = function(channel, control, value, status, group) {
    var idx = (group === "[Channel1]") ? 0 : 1;
    MixxxOSGeneric.shift[idx] = (value > 0);
};

// -----------------------------------------------------------------------------
// Sync with shift-to-master (Pioneer)
// -----------------------------------------------------------------------------
MixxxOSGeneric.syncPressed = function(channel, control, value, status, group) {
    if (value <= 0) { return; }
    var idx = (group === "[Channel1]") ? 0 : 1;
    if (MixxxOSGeneric.shift[idx]) {
        engine.setValue(group, "sync_master", 1);
    } else {
        var current = engine.getValue(group, "sync_enabled");
        engine.setValue(group, "sync_enabled", current > 0 ? 0 : 1);
    }
};

// -----------------------------------------------------------------------------
// Conflict dispatch: 0x97/0x00-0x03
//   Preset 0 (Pioneer)  → deck1 hotcue
//   Preset 1 (Hercules) → deck2 hotcue
// -----------------------------------------------------------------------------
MixxxOSGeneric.hotcueDispatch = function(channel, control, value, status, group) {
    if (value <= 0) { return; }
    var targetGroup = (MixxxOSGeneric.currentPreset === 0) ? "[Channel1]" : "[Channel2]";
    engine.setValue(targetGroup, "hotcue_" + (control + 1) + "_activate", 1);
};

// -----------------------------------------------------------------------------
// Conflict dispatch: 0x91/0x05
//   Preset 1 (Hercules) → deck1 sync toggle
//   Preset 2 (Numark)   → deck2 cue
// -----------------------------------------------------------------------------
MixxxOSGeneric.syncCueDispatch = function(channel, control, value, status, group) {
    if (MixxxOSGeneric.currentPreset === 1) {
        if (value > 0) {
            var cur = engine.getValue("[Channel1]", "sync_enabled");
            engine.setValue("[Channel1]", "sync_enabled", cur > 0 ? 0 : 1);
        }
    } else if (MixxxOSGeneric.currentPreset === 2) {
        engine.setValue("[Channel2]", "cue_default", value > 0 ? 1 : 0);
    }
};

// -----------------------------------------------------------------------------
// Conflict dispatch: 0x91/0x06
//   Preset 1 (Hercules) → deck1 cue
//   Preset 2 (Numark)   → deck2 jog touch
// -----------------------------------------------------------------------------
MixxxOSGeneric.cueTouchDispatch = function(channel, control, value, status, group) {
    if (MixxxOSGeneric.currentPreset === 1) {
        engine.setValue("[Channel1]", "cue_default", value > 0 ? 1 : 0);
    } else if (MixxxOSGeneric.currentPreset === 2) {
        var preset = MixxxOSGeneric.PRESETS[2];
        MixxxOSGeneric.touching[1] = (value > 0);
        if (value > 0) {
            engine.scratchEnable(2, preset.jogResolution, preset.jogRpm, preset.jogAlpha, preset.jogBeta);
        } else {
            engine.scratchDisable(2);
        }
    }
};
