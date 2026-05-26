#!/usr/bin/env bash
# =============================================================================
# Mixxx OS – Install Script
#
# Copies controller mapping + skin to the correct Mixxx user directories.
#
# Usage:
#   chmod +x install.sh
#   ./install.sh
# =============================================================================

set -e

# Detect Mixxx user data directory
if [[ "$OSTYPE" == "darwin"* ]]; then
    MIXXX_DIR="$HOME/Library/Application Support/Mixxx"
elif [[ "$OSTYPE" == "linux"* ]]; then
    MIXXX_DIR="$HOME/.mixxx"
else
    echo "OS not supported. Copy manually:"
    echo "  controllers/mixxx-os-controllers.midi.xml → ~/.mixxx/controllers/"
    echo "  controllers/mixxx-os-dispatcher.js        → ~/.mixxx/controllers/"
    echo "  skins/mixxx-os-2deck/                     → ~/.mixxx/skins/"
    exit 1
fi

CTRL_DIR="$MIXXX_DIR/controllers"
SKIN_DIR="$MIXXX_DIR/skins"

echo "Mixxx data directory: $MIXXX_DIR"
mkdir -p "$CTRL_DIR" "$SKIN_DIR"

# Install controller files
echo ""
echo "Installing controller mapping..."
cp controllers/mixxx-os-controllers.midi.xml "$CTRL_DIR/"
cp controllers/mixxx-os-dispatcher.js         "$CTRL_DIR/"
echo "  ✓ mixxx-os-controllers.midi.xml"
echo "  ✓ mixxx-os-dispatcher.js"

# Install skin
echo ""
echo "Installing skin..."
rm -rf "$SKIN_DIR/mixxx-os-2deck"
cp -r skins/mixxx-os-2deck "$SKIN_DIR/"
echo "  ✓ mixxx-os-2deck skin"

echo ""
echo "============================================================"
echo "Installation complete!"
echo ""
echo "Next steps in Mixxx:"
echo "  1. Preferences › Interface › Skin → select 'mixxx-os-2deck'"
echo "  2. Preferences › Controllers → select your device"
echo "     → click the mapping dropdown"
echo "     → choose 'Mixxx OS Controllers (Universal)'"
echo "  3. Click 'Enable' and restart Mixxx"
echo "============================================================"
