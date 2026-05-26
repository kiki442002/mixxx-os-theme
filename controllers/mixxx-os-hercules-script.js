// =============================================================================
// Mixxx OS – Hercules Inpulse 300 / 500  (Preset 1)
// =============================================================================

MixxxOSGeneric.registerPreset(1, {
    name: "Hercules Inpulse 300/500",

    // Jog wheel parameters
    jogResolution: 128,
    jogRpm:        33 + 1/3,
    jogAlpha:      1/8,
    jogBeta:       (1/8) / 32,
    bendScale:     0.5,

    // LED MIDI addresses
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
    },

    // Conflict 0x97/0x00-0x03: Hercules → deck2 hotcue
    hotcueDispatch: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        engine.setValue("[Channel2]", "hotcue_" + (control + 1) + "_activate", 1);
    },

    // Conflict 0x91/0x05: Hercules → deck1 sync toggle
    syncCueDispatch: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        var cur = engine.getValue("[Channel1]", "sync_enabled");
        engine.setValue("[Channel1]", "sync_enabled", cur > 0 ? 0 : 1);
    },

    // Conflict 0x91/0x06: Hercules → deck1 cue
    cueTouchDispatch: function(channel, control, value, status, group) {
        engine.setValue("[Channel1]", "cue_default", value > 0 ? 1 : 0);
    }
});
