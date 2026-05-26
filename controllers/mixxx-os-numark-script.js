// =============================================================================
// Mixxx OS – Numark Mixtrack Platinum FX  (Preset 2)
// =============================================================================

MixxxOSGeneric.registerPreset(2, {
    name: "Numark Mixtrack Platinum FX",

    // Jog wheel parameters
    jogResolution: 128,
    jogRpm:        33 + 1/3,
    jogAlpha:      1/8,
    jogBeta:       (1/8) / 32,
    bendScale:     0.5,

    // LED MIDI addresses
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
    },

    // Conflict 0x91/0x05: Numark → deck2 cue
    syncCueDispatch: function(channel, control, value, status, group) {
        engine.setValue("[Channel2]", "cue_default", value > 0 ? 1 : 0);
    },

    // Conflict 0x91/0x06: Numark → deck2 jog touch
    cueTouchDispatch: function(channel, control, value, status, group) {
        var preset = MixxxOSGeneric.PRESETS[2];
        MixxxOSGeneric.touching[1] = (value > 0);
        if (value > 0) {
            engine.scratchEnable(2, preset.jogResolution, preset.jogRpm, preset.jogAlpha, preset.jogBeta);
        } else {
            engine.scratchDisable(2);
        }
    }
});
