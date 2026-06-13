/**
 * mixxx-os Config Bridge — virtual MIDI controller
 *
 * CC protocol (channel 1, 0xB0):  server → Mixxx
 *   0x03  latency_idx      0-5
 *   0x04  decks_idx        0-1
 *   0x05  server_online    0|1
 *   0x10  master_out_idx   0-6  (127 = none)
 *   0x11  headphone_idx    0-6  (127 = none)
 *   0x12  num_outputs      0-127
 *   0x20  num_midi         0-4
 *   0x21..0x24  slot 0..3 state  0=empty 1=inactive 2=active
 *   0x25..0x28  slot 0..3 script 0-126  (127 = none)
 *
 * Note protocol (channel 1, 0x90):  Mixxx → server (velocity > 0)
 *   0x01  RELOAD                   (re-push all CCs)
 *   0x03  RESTART
 *   0x04  SET_LATENCY   vel = latency_idx
 *   0x05  SET_DECKS     vel = decks_idx
 *   0x10  MASTER_NEXT
 *   0x11  HEADPHONE_NEXT
 *   0x20..0x27  slot 0..3 toggle / script-next (even=toggle, odd=next)
 */

var MixxxOSConfig = {};
var connections   = [];

var MIDI_CH = 0x00;
var NOTE_ON = 0x90;

function sendNote(note, vel) {
    var v = (vel === undefined) ? 0x7F : vel;
    midi.sendShortMsg(NOTE_ON | MIDI_CH, note, v);
    midi.sendShortMsg(NOTE_ON | MIDI_CH, note, 0x00);
}

function setKey(key, val) {
    engine.setValue("[Skin]", key, val);
}

function getKey(key) {
    return engine.getValue("[Skin]", key);
}

/** Watch a pulse key: fires fn when key goes > 0, then resets to 0. */
function watchPulse(key, fn) {
    connections.push(engine.makeConnection("[Skin]", key, function (val) {
        if (val > 0) {
            fn();
            engine.setValue("[Skin]", key, 0);
        }
    }));
}

// ── Incoming CC  (server → skin) ─────────────────────────────────────────────

MixxxOSConfig.incomingCC = function (channel, control, value) {
    switch (control) {
        // Core config
        case 0x03: setKey("os_latency_idx",    value); break;
        case 0x04: setKey("os_decks_idx",      value); break;
        case 0x05: setKey("os_server_online",  value > 0 ? 1 : 0); break;

        // Audio outputs  (display states: 0-6 = device 1-7, 7 = "—")
        case 0x10: setKey("os_master_out",     value >= 7 ? 7 : value); break;
        case 0x11: setKey("os_headphone_out",  value >= 7 ? 7 : value); break;
        case 0x12: setKey("os_num_outputs",    value); break;

        // MIDI slots
        case 0x20: setKey("os_num_midi",       value); break;
        case 0x21: setKey("os_midi_0_state",   value); break;
        case 0x22: setKey("os_midi_1_state",   value); break;
        case 0x23: setKey("os_midi_2_state",   value); break;
        case 0x24: setKey("os_midi_3_state",   value); break;
        case 0x25: setKey("os_midi_0_script",  value >= 7 ? 7 : value); break;
        case 0x26: setKey("os_midi_1_script",  value >= 7 ? 7 : value); break;
        case 0x27: setKey("os_midi_2_script",  value >= 7 ? 7 : value); break;
        case 0x28: setKey("os_midi_3_script",  value >= 7 ? 7 : value); break;
    }
};

// ── Init / Shutdown ───────────────────────────────────────────────────────────

MixxxOSConfig.init = function (id, debug) {
    print("[MixxxOSConfig] init");
    setKey("os_server_online", 0);

    // Core config
    watchPulse("os_save", function () {
        sendNote(0x04, Math.round(getKey("os_latency_idx")));
        sendNote(0x05, Math.round(getKey("os_decks_idx")));
    });
    watchPulse("os_reload",  function () { sendNote(0x01); });
    watchPulse("os_restart", function () { sendNote(0x03); });

    // Audio output cycling
    watchPulse("os_master_next",    function () { sendNote(0x10); });
    watchPulse("os_headphone_next", function () { sendNote(0x11); });

    // MIDI slots
    watchPulse("os_midi_0_toggle",      function () { sendNote(0x20); });
    watchPulse("os_midi_0_script_next", function () { sendNote(0x21); });
    watchPulse("os_midi_1_toggle",      function () { sendNote(0x22); });
    watchPulse("os_midi_1_script_next", function () { sendNote(0x23); });
    watchPulse("os_midi_2_toggle",      function () { sendNote(0x24); });
    watchPulse("os_midi_2_script_next", function () { sendNote(0x25); });
    watchPulse("os_midi_3_toggle",      function () { sendNote(0x26); });
    watchPulse("os_midi_3_script_next", function () { sendNote(0x27); });

    // Initial state request
    sendNote(0x01);
};

MixxxOSConfig.shutdown = function () {
    setKey("os_server_online", 0);
    connections.forEach(function (c) { c.disconnect(); });
    connections = [];
    print("[MixxxOSConfig] shutdown");
};
