// =============================================================================
// Mixxx OS – Controller Dispatcher
//
// Single entry point for all supported controllers.
// Loaded by mixxx-os-controllers.midi.xml alongside all official scripts.
//
// On init: auto-detects the connected controller from the device name,
//          then delegates to the right official script's init().
//
// Supported:
//   Pioneer DDJ-FLX4  → PioneerDDJFLX4
//   Pioneer DDJ-400   → PioneerDDJ400
//   Pioneer DDJ-200   → DDJ200
//   Hercules Inpulse 300 → DJCi300
//   Hercules Inpulse 500 → DJCi500
//   Numark Mixtrack Platinum FX → MixtrackPlatinumFX
//
// Also handles skin-specific features:
//   - "PISTE ACTIVE" deck selector (cuts crossfader to deck 1 or 2)
// =============================================================================

var MixxxOSDispatcher = {};

// Map device name substrings to their official script objects
MixxxOSDispatcher.CONTROLLERS = [
    { match: "FLX4",                script: "PioneerDDJFLX4",       label: "Pioneer DDJ-FLX4" },
    { match: "DDJ-400",             script: "PioneerDDJ400",         label: "Pioneer DDJ-400" },
    { match: "DDJ-200",             script: "DDJ200",                label: "Pioneer DDJ-200" },
    { match: "Inpulse 300",         script: "DJCi300",               label: "Hercules Inpulse 300" },
    { match: "Inpulse300",          script: "DJCi300",               label: "Hercules Inpulse 300" },
    { match: "Inpulse 500",         script: "DJCi500",               label: "Hercules Inpulse 500" },
    { match: "Inpulse500",          script: "DJCi500",               label: "Hercules Inpulse 500" },
    { match: "Mixtrack Platinum FX",script: "MixtrackPlatinumFX",    label: "Numark Mixtrack Platinum FX" },
    { match: "Mixtrack Platinum",   script: "MixtrackPlatinumFX",    label: "Numark Mixtrack Platinum FX" },
];

MixxxOSDispatcher.activeScript = null;
MixxxOSDispatcher.skinConnections = [];

MixxxOSDispatcher.init = function(id, debug) {
    // Detect controller from device name
    var detected = null;
    for (var i = 0; i < MixxxOSDispatcher.CONTROLLERS.length; i++) {
        var c = MixxxOSDispatcher.CONTROLLERS[i];
        if (id && id.indexOf(c.match) !== -1) {
            detected = c;
            break;
        }
    }

    if (detected) {
        print("[MixxxOS] Detected controller: " + detected.label);
        MixxxOSDispatcher.activeScript = detected.script;
        var scriptObj = this._getScript(detected.script);
        if (scriptObj && typeof scriptObj.init === "function") {
            scriptObj.init(id, debug);
        }
    } else {
        print("[MixxxOS] Unknown controller: " + id + ". No controller init called.");
        print("[MixxxOS] Supported: DDJ-FLX4, DDJ-400, DDJ-200, Inpulse 300, Inpulse 500, Mixtrack Platinum FX");
    }

    // Skin deck selector: cut crossfader to deck 1 or 2
    var d1 = engine.makeConnection("[Skin]", "select_deck_1", function(v) {
        if (v > 0) { engine.setValue("[Master]", "crossfader", -1.0); }
    });
    var d2 = engine.makeConnection("[Skin]", "select_deck_2", function(v) {
        if (v > 0) { engine.setValue("[Master]", "crossfader", 1.0); }
    });
    var cf = engine.makeConnection("[Master]", "crossfader", function(value) {
        engine.setValue("[Skin]", "deck_active_1", value <= 0 ? 1 : 0);
        engine.setValue("[Skin]", "deck_active_2", value > 0 ? 1 : 0);
    });
    MixxxOSDispatcher.skinConnections = [d1, d2, cf];
    d1.trigger();
    cf.trigger();
};

MixxxOSDispatcher.shutdown = function() {
    // Disconnect skin bindings
    for (var i = 0; i < MixxxOSDispatcher.skinConnections.length; i++) {
        MixxxOSDispatcher.skinConnections[i].disconnect();
    }
    MixxxOSDispatcher.skinConnections = [];

    // Delegate to the active controller's shutdown
    if (MixxxOSDispatcher.activeScript) {
        var scriptObj = this._getScript(MixxxOSDispatcher.activeScript);
        if (scriptObj && typeof scriptObj.shutdown === "function") {
            scriptObj.shutdown();
        }
    }
};

// Internal: get the global script object by name
MixxxOSDispatcher._getScript = function(name) {
    /* jshint evil:true */
    try {
        // In Mixxx JS context, global variables are accessible
        if (typeof PioneerDDJFLX4 !== "undefined" && name === "PioneerDDJFLX4") return PioneerDDJFLX4;
        if (typeof PioneerDDJ400  !== "undefined" && name === "PioneerDDJ400")  return PioneerDDJ400;
        if (typeof DDJ200          !== "undefined" && name === "DDJ200")          return DDJ200;
        if (typeof DJCi300         !== "undefined" && name === "DJCi300")         return DJCi300;
        if (typeof DJCi500         !== "undefined" && name === "DJCi500")         return DJCi500;
        if (typeof MixtrackPlatinumFX !== "undefined" && name === "MixtrackPlatinumFX") return MixtrackPlatinumFX;
    } catch (e) {
        print("[MixxxOS] Error getting script " + name + ": " + e);
    }
    return null;
};
