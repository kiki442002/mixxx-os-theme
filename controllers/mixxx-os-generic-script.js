// =============================================================================
// Mixxx OS – Skin Manager Script
//
// Loaded alongside each official controller mapping (not as a standalone).
// Handles only skin-specific features:
//   - "PISTE ACTIVE" deck selector (cuts crossfader to deck 1 or 2)
//   - Skin init/shutdown connections
//
// All controller-specific MIDI handling is done by the official scripts.
// =============================================================================

var MixxxOSGeneric = {};

MixxxOSGeneric.connections = [];

MixxxOSGeneric.init = function(id) {
    // Deck selector buttons cut the crossfader hard left (deck 1) or right (deck 2)
    var d1 = engine.makeConnection("[Skin]", "select_deck_1", function(v) {
        if (v > 0) { engine.setValue("[Master]", "crossfader", -1.0); }
    });
    var d2 = engine.makeConnection("[Skin]", "select_deck_2", function(v) {
        if (v > 0) { engine.setValue("[Master]", "crossfader",  1.0); }
    });

    // Keep the skin deck indicator in sync with the real crossfader position
    var cf = engine.makeConnection("[Master]", "crossfader", function(value) {
        engine.setValue("[Skin]", "deck_active_1", value <= 0 ? 1 : 0);
        engine.setValue("[Skin]", "deck_active_2", value >  0 ? 1 : 0);
    });

    if (d1) { MixxxOSGeneric.connections.push(d1); }
    if (d2) { MixxxOSGeneric.connections.push(d2); }
    if (cf) { MixxxOSGeneric.connections.push(cf); }

    // Sync initial deck indicator state
    var cfVal = engine.getValue("[Master]", "crossfader");
    engine.setValue("[Skin]", "deck_active_1", cfVal <= 0 ? 1 : 0);
    engine.setValue("[Skin]", "deck_active_2", cfVal >  0 ? 1 : 0);
};

MixxxOSGeneric.shutdown = function(id) {
    for (var i = 0; i < MixxxOSGeneric.connections.length; i++) {
        if (MixxxOSGeneric.connections[i]) {
            MixxxOSGeneric.connections[i].disconnect();
        }
    }
    MixxxOSGeneric.connections = [];
};
