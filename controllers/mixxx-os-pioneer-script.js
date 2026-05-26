// =============================================================================
// Mixxx OS – Pioneer DDJ-400 / FLX4 / DDJ-200  (Preset 0)
// =============================================================================

MixxxOSGeneric.registerPreset(0, {
    name: "Pioneer DDJ-400/FLX4/DDJ-200",

    // Jog wheel parameters
    jogResolution: 720,
    jogRpm:        33 + 1/3,
    jogAlpha:      1/8,
    jogBeta:       (1/8) / 32,
    bendScale:     0.8,

    // LED MIDI addresses
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
    },

    // Shift button – stores shift state for syncPressed
    shiftPressed: function(channel, control, value, status, group) {
        var idx = (group === "[Channel1]") ? 0 : 1;
        MixxxOSGeneric.shift[idx] = (value > 0);
    },

    // Sync – long press (shift held) sets master, short press toggles sync
    syncPressed: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        var idx = (group === "[Channel1]") ? 0 : 1;
        if (MixxxOSGeneric.shift[idx]) {
            engine.setValue(group, "sync_master", 1);
        } else {
            var current = engine.getValue(group, "sync_enabled");
            engine.setValue(group, "sync_enabled", current > 0 ? 0 : 1);
        }
    },

    // Conflict 0x97/0x00-0x03: Pioneer → deck1 hotcue
    hotcueDispatch: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        engine.setValue("[Channel1]", "hotcue_" + (control + 1) + "_activate", 1);
    }
});
