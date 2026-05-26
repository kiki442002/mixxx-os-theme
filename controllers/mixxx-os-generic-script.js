// =============================================================================
// Mixxx OS Generic Controller – MANAGER
// Handles: init/shutdown, preset selection, LED dispatch, jog wheels,
//          radio buttons, deck selector.
//
// Each controller registers its profile via:
//   MixxxOSGeneric.registerPreset(index, profile)
// Profiles are defined in their own script files loaded after this one.
// =============================================================================

var MixxxOSGeneric = {};

// Registry populated by per-controller scripts
MixxxOSGeneric.PRESETS = {};

MixxxOSGeneric.registerPreset = function(idx, profile) {
    MixxxOSGeneric.PRESETS[idx] = profile;
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

    // Radio button select keys – each fires when a preset button is clicked
    var s0 = engine.makeConnection("[Skin]", "select_preset_0", function(v) { if (v > 0) MixxxOSGeneric.selectPreset(0); });
    var s1 = engine.makeConnection("[Skin]", "select_preset_1", function(v) { if (v > 0) MixxxOSGeneric.selectPreset(1); });
    var s2 = engine.makeConnection("[Skin]", "select_preset_2", function(v) { if (v > 0) MixxxOSGeneric.selectPreset(2); });
    if (s0) { MixxxOSGeneric.connections.push(s0); }
    if (s1) { MixxxOSGeneric.connections.push(s1); }
    if (s2) { MixxxOSGeneric.connections.push(s2); }

    // Deck selector buttons – cut crossfader to deck 1 or 2
    var d1 = engine.makeConnection("[Skin]", "select_deck_1", function(v) { if (v > 0) MixxxOSGeneric.cutToDeck(1); });
    var d2 = engine.makeConnection("[Skin]", "select_deck_2", function(v) { if (v > 0) MixxxOSGeneric.cutToDeck(2); });
    var cf = engine.makeConnection("[Master]", "crossfader", MixxxOSGeneric.onCrossfaderChange);
    if (d1) { MixxxOSGeneric.connections.push(d1); }
    if (d2) { MixxxOSGeneric.connections.push(d2); }
    if (cf) { MixxxOSGeneric.connections.push(cf); }

    for (var deck = 1; deck <= 2; deck++) {
        var group = "[Channel" + deck + "]";
        var c1 = engine.makeConnection(group, "play",          MixxxOSGeneric.makeCallback(deck, "play",  "play"));
        var c2 = engine.makeConnection(group, "cue_indicator", MixxxOSGeneric.makeCallback(deck, "cue",   "cue_indicator"));
        var c3 = engine.makeConnection(group, "sync_enabled",  MixxxOSGeneric.makeCallback(deck, "sync",  "sync_enabled"));
        if (c1) { MixxxOSGeneric.connections.push(c1); }
        if (c2) { MixxxOSGeneric.connections.push(c2); }
        if (c3) { MixxxOSGeneric.connections.push(c3); }
    }

    MixxxOSGeneric.updateRadioButtons(MixxxOSGeneric.currentPreset);
    MixxxOSGeneric.onCrossfaderChange(engine.getValue("[Master]", "crossfader"));
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
// Preset selection
// -----------------------------------------------------------------------------
MixxxOSGeneric.selectPreset = function(idx) {
    MixxxOSGeneric.currentPreset = idx;
    engine.setValue("[Skin]", "controller_preset", idx);
    MixxxOSGeneric.updateRadioButtons(idx);
    MixxxOSGeneric.refreshAllLEDs();
};

MixxxOSGeneric.updateRadioButtons = function(activeIdx) {
    engine.setValue("[Skin]", "preset_active_0", activeIdx === 0 ? 1 : 0);
    engine.setValue("[Skin]", "preset_active_1", activeIdx === 1 ? 1 : 0);
    engine.setValue("[Skin]", "preset_active_2", activeIdx === 2 ? 1 : 0);
};

// -----------------------------------------------------------------------------
// Deck selector
// -----------------------------------------------------------------------------
MixxxOSGeneric.cutToDeck = function(deck) {
    engine.setValue("[Master]", "crossfader", deck === 1 ? -1.0 : 1.0);
};

MixxxOSGeneric.onCrossfaderChange = function(value) {
    engine.setValue("[Skin]", "deck_active_1", value <= 0 ? 1 : 0);
    engine.setValue("[Skin]", "deck_active_2", value > 0  ? 1 : 0);
};

// -----------------------------------------------------------------------------
// LED helpers
// -----------------------------------------------------------------------------
MixxxOSGeneric.sendLED = function(deckNum, ledName, on) {
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (!preset) { return; }
    var led = preset.leds["ch" + deckNum] && preset.leds["ch" + deckNum][ledName];
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
        MixxxOSGeneric.sendLED(deck, "play", engine.getValue(group, "play")          > 0);
        MixxxOSGeneric.sendLED(deck, "cue",  engine.getValue(group, "cue_indicator") > 0);
        MixxxOSGeneric.sendLED(deck, "sync", engine.getValue(group, "sync_enabled")  > 0);
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
MixxxOSGeneric.groupToDeck = function(group) {
    if (group === "[Channel1]") { return 1; }
    if (group === "[Channel2]") { return 2; }
    return 0;
};

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

// Platter turn (vinyl scratch CC or non-vinyl pitch bend CC) – uses scratch
// when touch is active, otherwise pitch bends.
MixxxOSGeneric.jogTurn = function(channel, control, value, status, group) {
    var deck = MixxxOSGeneric.groupToDeck(group);
    if (deck < 1) { return; }
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (!preset) { return; }

    var delta = value - 64;
    if (engine.isScratching(deck)) {
        engine.scratchTick(deck, delta);
    } else {
        engine.setValue(group, "jog", delta * preset.bendScale);
    }
};

// Side ring – ALWAYS pitch bend, never scratch.
// Prevents double scratch ticks when platter and side ring move simultaneously.
MixxxOSGeneric.jogSideRing = function(channel, control, value, status, group) {
    var deck = MixxxOSGeneric.groupToDeck(group);
    if (deck < 1) { return; }
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (!preset) { return; }

    engine.setValue(group, "jog", (value - 64) * preset.bendScale);
};

// -----------------------------------------------------------------------------
// High-res tempo slider (14-bit Pioneer: CC 0/32 ch1, CC 0/32 ch2)
// rate formula: 1 - (fullValue / 0x2000)
//   center (8192) → 0 (normal speed)
//   min (0)       → +1 (max pitch up)
//   max (16383)   → -1 (max pitch down)
// -----------------------------------------------------------------------------
MixxxOSGeneric._tempoMSB = {"[Channel1]": 0, "[Channel2]": 0};

MixxxOSGeneric.tempoMSB = function(channel, control, value, status, group) {
    if (MixxxOSGeneric._tempoMSB[group] !== undefined) {
        MixxxOSGeneric._tempoMSB[group] = value;
    }
};

MixxxOSGeneric.tempoLSB = function(channel, control, value, status, group) {
    var msb = MixxxOSGeneric._tempoMSB[group];
    if (msb === undefined) { return; }
    var fullValue = (msb << 7) + value;
    engine.setValue(group, "rate", 1 - (fullValue / 0x2000));
};

// -----------------------------------------------------------------------------
// Loop size controls (CUE/LOOP CALL buttons)
// -----------------------------------------------------------------------------
MixxxOSGeneric.loopHalve = function(channel, control, value, status, group) {
    if (value <= 0) { return; }
    engine.setValue(group, "loop_halve", 1);
};

MixxxOSGeneric.loopDouble = function(channel, control, value, status, group) {
    if (value <= 0) { return; }
    engine.setValue(group, "loop_double", 1);
};

// -----------------------------------------------------------------------------
// Delegating input handlers – route to current preset's handler if defined
// -----------------------------------------------------------------------------
MixxxOSGeneric._delegate = function(handlerName, channel, control, value, status, group) {
    var preset = MixxxOSGeneric.PRESETS[MixxxOSGeneric.currentPreset];
    if (preset && typeof preset[handlerName] === "function") {
        preset[handlerName](channel, control, value, status, group);
    }
};

MixxxOSGeneric.shiftPressed    = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("shiftPressed",    ch, ctrl, val, st, grp); };
MixxxOSGeneric.syncPressed     = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("syncPressed",     ch, ctrl, val, st, grp); };
MixxxOSGeneric.hotcueDispatch  = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("hotcueDispatch",  ch, ctrl, val, st, grp); };
MixxxOSGeneric.syncCueDispatch = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("syncCueDispatch", ch, ctrl, val, st, grp); };
MixxxOSGeneric.cueTouchDispatch= function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("cueTouchDispatch",ch, ctrl, val, st, grp); };
MixxxOSGeneric.beatFxSelect    = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("beatFxSelect",    ch, ctrl, val, st, grp); };
MixxxOSGeneric.beatFxSelectShift= function(ch, ctrl, val, st, grp){ MixxxOSGeneric._delegate("beatFxSelectShift",ch, ctrl, val, st, grp); };
MixxxOSGeneric.beatFxLeft      = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("beatFxLeft",      ch, ctrl, val, st, grp); };
MixxxOSGeneric.beatFxRight     = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("beatFxRight",     ch, ctrl, val, st, grp); };
MixxxOSGeneric.beatFxOnOff     = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("beatFxOnOff",     ch, ctrl, val, st, grp); };
MixxxOSGeneric.beatFxLevel     = function(ch, ctrl, val, st, grp) { MixxxOSGeneric._delegate("beatFxLevel",     ch, ctrl, val, st, grp); };

