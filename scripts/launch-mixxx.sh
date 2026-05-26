#!/usr/bin/env bash
# launch-mixxx.sh — Détecte le contrôleur USB branché, configure la sortie
# ALSA dans ~/.mixxx/mixxx.cfg, puis lance Mixxx.
#
# Usage :  ./scripts/launch-mixxx.sh
# Rendre exécutable :  chmod +x scripts/launch-mixxx.sh

set -euo pipefail

MIXXX_CFG="${HOME}/.mixxx/mixxx.cfg"

# ---------------------------------------------------------------------------
# Table contrôleurs : USB_ID -> (nom_alsa, ch_master, ch_headphones)
# Ajoutez vos contrôleurs ici.  Trouvez l'ID avec : lsusb
# Trouvez le nom ALSA avec    : aplay -l  (colonne "card N: NAME")
# ---------------------------------------------------------------------------
declare -A CTRL_ALSA_NAME=(
    ["2b73:000a"]="DDJ400"          # Pioneer DDJ-400
    ["2b73:001e"]="FLXFOUR"         # Pioneer DDJ-FLX4
    ["2b73:0006"]="DDJTWO"          # Pioneer DDJ-200
    ["06f8:d002"]="InpulseFive"     # Hercules Inpulse 500
    ["06f8:d001"]="InpulseThree"    # Hercules Inpulse 300
    ["09e8:0118"]="MixtrackPlatFX"  # Numark Mixtrack Platinum FX
)
# Canal master et casque (numéro de sous-périphérique ALSA, généralement 0)
declare -A CTRL_MASTER_CH=(
    ["2b73:000a"]="0"  ["2b73:001e"]="0"  ["2b73:0006"]="0"
    ["06f8:d002"]="0"  ["06f8:d001"]="0"  ["09e8:0118"]="0"
)
declare -A CTRL_HEAD_CH=(
    ["2b73:000a"]="1"  ["2b73:001e"]="1"  ["2b73:0006"]="1"
    ["06f8:d002"]="1"  ["06f8:d001"]="1"  ["09e8:0118"]="1"
)

# ---------------------------------------------------------------------------
# Détection du contrôleur
# ---------------------------------------------------------------------------
detected_id=""
for usb_id in "${!CTRL_ALSA_NAME[@]}"; do
    if lsusb | grep -qi "${usb_id}"; then
        detected_id="${usb_id}"
        break
    fi
done

# ---------------------------------------------------------------------------
# Mise à jour de mixxx.cfg
# ---------------------------------------------------------------------------
update_cfg() {
    local key="$1" value="$2"
    # Crée la clé si absente, sinon la remplace
    if grep -q "^${key}=" "${MIXXX_CFG}" 2>/dev/null; then
        sed -i "s|^${key}=.*|${key}=${value}|" "${MIXXX_CFG}"
    else
        # Cherche la section [Soundcard], sinon ajoute en fin de fichier
        if grep -q "^\[Soundcard\]" "${MIXXX_CFG}" 2>/dev/null; then
            sed -i "/^\[Soundcard\]/a ${key}=${value}" "${MIXXX_CFG}"
        else
            printf "\n[Soundcard]\n%s=%s\n" "${key}" "${value}" >> "${MIXXX_CFG}"
        fi
    fi
}

if [[ -n "${detected_id}" ]]; then
    alsa_name="${CTRL_ALSA_NAME[$detected_id]}"
    master_ch="${CTRL_MASTER_CH[$detected_id]}"
    head_ch="${CTRL_HEAD_CH[$detected_id]}"

    # Résout le numéro de carte ALSA depuis le nom (plus fiable que le numéro fixe)
    alsa_card=$(aplay -l 2>/dev/null \
        | grep -i "${alsa_name}" \
        | head -1 \
        | sed 's/card \([0-9]*\):.*/\1/')

    if [[ -n "${alsa_card}" ]]; then
        master_dev="hw:${alsa_card},${master_ch}"
        head_dev="hw:${alsa_card},${head_ch}"
        echo "[launch-mixxx] Contrôleur détecté : ${alsa_name} (USB ${detected_id})"
        echo "[launch-mixxx] Master  → ${master_dev}"
        echo "[launch-mixxx] Casque  → ${head_dev}"
    else
        echo "[launch-mixxx] ⚠️  Carte ALSA '${alsa_name}' introuvable — vérifiez 'aplay -l'"
        echo "[launch-mixxx] Lancement sans modifier la config audio."
        master_dev="" head_dev=""
    fi
else
    echo "[launch-mixxx] Aucun contrôleur connu détecté — config audio inchangée."
    master_dev="" head_dev=""
fi

if [[ -n "${master_dev}" ]]; then
    update_cfg "Output Master1" "${master_dev}"
    update_cfg "Output Master2" "$((master_ch + 1))"
    update_cfg "Output Headphones1" "${head_dev}"
    update_cfg "Output Headphones2" "$((head_ch + 1))"
fi

# ---------------------------------------------------------------------------
# Lancement de Mixxx
# ---------------------------------------------------------------------------
echo "[launch-mixxx] Démarrage de Mixxx…"
exec mixxx "$@"
