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

    // Sync – shift+sync sets sync master, plain sync toggles
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
    },

    // -------------------------------------------------------------------------
    // Beat FX  (EffectRack1_EffectUnit1)
    // -------------------------------------------------------------------------

    // Returns the group of the currently focused effect slot.
    _focusedFxGroup: function() {
        var focused = engine.getValue("[EffectRack1_EffectUnit1]", "focused_effect");
        return "[EffectRack1_EffectUnit1_Effect" + focused + "]";
    },

    // SELECT button – press → next effect in focused slot.
    beatFxSelect: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        engine.setValue(this._focusedFxGroup(), "next_effect", 1);
    },

    // SELECT + SHIFT – press → previous effect.
    beatFxSelectShift: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        engine.setValue(this._focusedFxGroup(), "prev_effect", 1);
    },

    // BEAT LEFT – cycle focused slot backward (1→3→2→1).
    beatFxLeft: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        var focused = engine.getValue("[EffectRack1_EffectUnit1]", "focused_effect");
        focused = ((focused - 2 + 3) % 3) + 1;
        engine.setValue("[EffectRack1_EffectUnit1]", "focused_effect", focused);
    },

    // BEAT RIGHT – cycle focused slot forward (1→2→3→1).
    beatFxRight: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        var focused = engine.getValue("[EffectRack1_EffectUnit1]", "focused_effect");
        focused = (focused % 3) + 1;
        engine.setValue("[EffectRack1_EffectUnit1]", "focused_effect", focused);
    },

    // ON/OFF – toggle the focused effect slot enabled state.
    beatFxOnOff: function(channel, control, value, status, group) {
        if (value <= 0) { return; }
        var fxGroup = this._focusedFxGroup();
        engine.setValue(fxGroup, "enabled", engine.getValue(fxGroup, "enabled") > 0 ? 0 : 1);
    },

    // LEVEL/DEPTH encoder – CC value 0-127 → effect unit mix (dry/wet).
    beatFxLevel: function(channel, control, value, status, group) {
        engine.setParameter("[EffectRack1_EffectUnit1]", "mix", value / 0x7F);
    }
});
