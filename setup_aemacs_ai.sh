#!/bin/bash

# ==============================================================================
# Æmacs AI Server Setup - The Airlock Pattern
# ==============================================================================
# Description:
#   This script automates the deployment of a secure, local AI inference server.
#   It uses a three-phase "Airlock" approach:
#     1. Loader Phase (Online):  Downloads models to a persistent volume.
#     2. Bunker Phase (Offline): Runs the API in a network-isolated container.
#     3. Memory Phase (Offline): Runs Qdrant DB in the same isolated network.
#
# Usage:
#   ./setup_aemacs_ai.sh
# ==============================================================================

# --- Configuration ---
PROD_CONTAINER="aemacs_ai_server"
LOADER_CONTAINER="aemacs_model_loader"
VOLUME_NAME="aemacs_model_data"
NETWORK_NAME="aemacs_offline_net"
OLLAMA_IMAGE="ollama/ollama:latest"

QDRANT_CONTAINER="aemacs_memory_qdrant"
QDRANT_VOLUME="aemacs_qdrant_data"
QDRANT_IMAGE="qdrant/qdrant:latest"

# ==============================================================================
# FUNCTION: SYSTEM TRAP & EMERGENCY CLEANUP
# ==============================================================================
# This function guarantees that no fragmented tensors or dangling containers
# remain on the host system if the user aborts the setup (e.g., via Ctrl+C)
# or if a critical error occurs.
# ==============================================================================
cleanup_on_exit() {
    local exit_code=$?
    
    # Only print the abort/error messages if we didn't exit cleanly (Code 0)
    if [ $exit_code -ne 0 ]; then
        echo ""
        echo "⚠️  CRITICAL: Setup interrupted or failed (Exit Code: $exit_code)."
        echo "🧹 Initiating emergency cleanup to prevent storage leaks..."
    fi

    # 1. Purge ephemeral build artifacts on the persistent volume
    # We execute this inside the Loader. If the Loader is dead, we suppress the error.
    if docker ps -q -f name="^/${LOADER_CONTAINER}$" >/dev/null 2>&1; then
        if [ $exit_code -ne 0 ]; then
            echo "   -> Wiping temporary tensor fragments from the volume..."
        fi
        docker exec $LOADER_CONTAINER rm -rf "/root/.ollama/tmp_build" 2>/dev/null || true
    fi

    # 2. Terminate the Loader Container
    # We force-remove the container so it doesn't block future setup attempts.
    if docker ps -a -q -f name="^/${LOADER_CONTAINER}$" >/dev/null 2>&1; then
        if [ $exit_code -ne 0 ]; then
            echo "   -> Terminating ephemeral Loader container..."
        fi
        docker rm -f $LOADER_CONTAINER >/dev/null 2>&1 || true
    fi

    if [ $exit_code -ne 0 ]; then
        echo "❌ Setup aborted. System state rolled back safely."
    fi
    
    # Propagate the original exit code to the OS
    exit $exit_code
}

# Bind the cleanup function to OS signals:
# - SIGINT: Interrupt (Ctrl+C)
# - SIGTERM: Termination signal (e.g., from kill)
# - EXIT: Catch-all when the script naturally ends or crashes
trap cleanup_on_exit SIGINT SIGTERM EXIT

# ==========================================
# FUNCTION: PULL NATIVE MODELS (IDEMPOTENT)
# ==========================================
pull_model() {
    local MODEL_NAME=$1

    # Check if the model already exists in the persistent volume
    if docker exec $LOADER_CONTAINER ollama show "$MODEL_NAME" >/dev/null 2>&1; then
        echo "   ✅ Model '$MODEL_NAME' is already installed. Skipping pull."
    else
        echo "   🔹 Pulling: $MODEL_NAME ..."
        docker exec $LOADER_CONTAINER ollama pull "$MODEL_NAME"
    fi
}

# ==========================================
# FUNCTION: SIDELOAD HF MODELS (IDEMPOTENT)
# ==========================================
load_hf_model() {
    local OLLAMA_TAG=$1
    local GGUF_URL=$2
    local TMP_DIR="/root/.ollama/tmp_build"

    # 1. Existence Check: Do we already have this model?
    # Removed -t, no pseudo-TTY needed for background checks
    if docker exec $LOADER_CONTAINER ollama show "$OLLAMA_TAG" >/dev/null 2>&1; then
        echo "   ✅ Custom Model '$OLLAMA_TAG' is already compiled and installed. Skipping download."
        return 0
    fi

    echo "   🔹 Sideloading Custom Model: $OLLAMA_TAG ..."

    # 2. Harden the container: Ensure aria2 is installed (Non-Interactive)
    docker exec $LOADER_CONTAINER bash -c "command -v aria2c >/dev/null || (export DEBIAN_FRONTEND=noninteractive && apt-get update -qq && apt-get install -y aria2 -qq >/dev/null)"

    # Create an ephemeral build directory inside the container
    docker exec $LOADER_CONTAINER mkdir -p "$TMP_DIR"

    # 3. Direct Tensor Download (Multi-Connection with Strict Error Handling)
    echo "      -> Downloading bare weights concurrently (Maximum Speed)..."
    if ! docker exec $LOADER_CONTAINER aria2c -U "Mozilla/5.0" -x 16 -s 16 -k 100M -c --file-allocation=falloc --summary-interval=10 -d "$TMP_DIR" -o "model.gguf" "$GGUF_URL"; then
        echo "   ❌ CRITICAL: Download of $OLLAMA_TAG failed. Network error or invalid URL."
        return 1 # Exit cleanly with error code, preventing corrupted compilation
    fi

    # 4. Dynamic Modelfile Generation
    docker exec $LOADER_CONTAINER bash -c "echo 'FROM $TMP_DIR/model.gguf' > $TMP_DIR/Modelfile"

    # 5. Native Compilation inside Ollama (With Error Handling)
    echo "      -> Compiling $OLLAMA_TAG inside the inference engine..."
    if ! docker exec $LOADER_CONTAINER ollama create "$OLLAMA_TAG" -f "$TMP_DIR/Modelfile"; then
        echo "   ❌ CRITICAL: Ollama compilation failed for $OLLAMA_TAG."
        return 1
    fi

    # 6. Storage Cleanup
    docker exec $LOADER_CONTAINER rm -rf "$TMP_DIR"
    echo "   ✅ Custom model $OLLAMA_TAG successfully compiled and ready."
}

# --- SMART GPU DETECTION ---
# We check if 'nvidia-smi' works AND if Docker has the nvidia-container-runtime configured.
# This prevents crashes on laptops where drivers are installed but Docker is not linked.
if command -v nvidia-smi &> /dev/null && (docker info 2>/dev/null | grep -i "name: nvidia" &> /dev/null || docker info 2>/dev/null | grep -i "Runtimes.*nvidia" &> /dev/null); then
    GPU_STRATEGY="--gpus=all"
    echo "🎮 NVIDIA Runtime found in Docker. AI Acceleration ENABLED."
else
    echo "⚠️  NVIDIA Runtime NOT detected in Docker."
    echo "    Debug Info:"
    echo "    - nvidia-smi present: $(if command -v nvidia-smi &>/dev/null; then echo 'Yes'; else echo 'No'; fi)"
    echo "    - Docker Runtimes: $(docker info 2>/dev/null | grep 'Runtimes')"
    echo "    - Docker Names: $(docker info 2>/dev/null | grep 'name')"
    echo "    Running in CPU Mode (Slower)."
    GPU_STRATEGY=""
fi

echo "========================================"
echo "🛡️  Initializing Æmacs AI Infrastructure"
echo "========================================"

if [ -n "$AEMACS_TIER" ]; then
    TIER_CHOICE="$AEMACS_TIER"
    echo "⚙️  Headless Mode: Tier explicitly set via environment variable to: $TIER_CHOICE"
    case $TIER_CHOICE in
        1) TIER="LOW";;
        2) TIER="MEDIUM";;
        3) TIER="HIGH";;
        *) echo "❌ CRITICAL: Invalid AEMACS_TIER environment variable. Must be 1, 2, or 3."; exit 1;;
    esac
else
    echo ""
    echo "🖥️  Please select your hardware capabilities to download the optimal AI models."
    echo "    This ensures The Living Mesh runs smoothly without crashing your system."
    echo ""
    echo "    [1] LOW    - Laptops & Standard PCs (< 8GB VRAM)"
    echo "                 Downloads fast, surgical 8B parameter models."
    echo "    [2] MEDIUM - Workstations (~12-26GB VRAM)"
    echo "                 Downloads highly efficient MoE (Mixture of Experts) & 12B models."
    echo "    [3] HIGH   - Heavy Compute & Multi-GPU (40GB+ VRAM)"
    echo "                 Downloads massive, uncompromising 70B parameter models."
    echo ""

    while true; do
        read -p "Enter your choice [1-3]: " TIER_CHOICE
        case $TIER_CHOICE in
            1) TIER="LOW"; break;;
            2) TIER="MEDIUM"; break;;
            3) TIER="HIGH"; break;;
            *) echo "❌ Invalid input. Please enter 1, 2, or 3.";;
        esac
    done
fi

echo "✅ Selected Tier: $TIER"

# --- Save Configuration ---
CONFIG_DIR="$HOME/.aemacs"
CONFIG_FILE="$CONFIG_DIR/config.ron"
mkdir -p "$CONFIG_DIR"

cat <<EOF > "$CONFIG_FILE"
UserConfig(
    hardware_tier: Some("$TIER"),
)
EOF

echo "   ✅ Configuration saved to $CONFIG_FILE"
echo ""

# 1. CLEANUP & PREPARATION
# ------------------------------------------------------------------------------
echo "🧹 [Step 1/5] Cleaning up existing instances..."

# Force remove ALL existing containers to ensure a clean state
docker rm -f $PROD_CONTAINER 2>/dev/null || true
docker rm -f $LOADER_CONTAINER 2>/dev/null || true
docker rm -f $QDRANT_CONTAINER 2>/dev/null || true

# Ensure persistent volumes exist
docker volume create $VOLUME_NAME >/dev/null
docker volume create $QDRANT_VOLUME >/dev/null
echo "   ✅ Storage Volumes checked."

# Ensure the isolated network exists (FIX: Removed --internal for port publishing)
if ! docker network ls | grep -q $NETWORK_NAME; then
    docker network create --driver bridge $NETWORK_NAME
    echo "   ✅ Secure Network '$NETWORK_NAME' created."
else
    echo "   ✅ Secure Network '$NETWORK_NAME' exists."
fi


# 2. PHASE 1: THE LOADER (ONLINE)
# ------------------------------------------------------------------------------
echo "⬇️  [Step 2/5] Pre-fetching Docker Images..."
docker pull $OLLAMA_IMAGE
docker pull $QDRANT_IMAGE
echo "   ✅ Base images (Ollama & Qdrant) are cached."

echo "⬇️  [Step 3/5] Starting Loader (Online Phase)..."

# Run a temporary container attached to the host network
docker run -d --rm \
  --name $LOADER_CONTAINER \
  $GPU_STRATEGY \
  -v $VOLUME_NAME:/root/.ollama \
  $OLLAMA_IMAGE >/dev/null

# Wait for Ollama API to initialize inside the container (With Timeout)
echo "   ⏳ Waiting for Ollama API to initialize..."
MAX_WAIT=30
WAIT_COUNT=0
until docker logs $LOADER_CONTAINER 2>&1 | grep -q "Listening on"; do
    if [ $WAIT_COUNT -ge $MAX_WAIT ]; then
        echo "   ❌ CRITICAL: Loader failed to initialize within $MAX_WAIT seconds."
        echo "      Dumping Container Logs:"
        docker logs $LOADER_CONTAINER
        exit 1
    fi
    sleep 1
    WAIT_COUNT=$((WAIT_COUNT + 1))
done
echo "   ✅ Loader is ready."

# 3. PULLING MODELS
# ------------------------------------------------------------------------------
echo "📦 [Step 4/5] Pulling Models for Tier: $TIER..."

# Pull models based on the interactive selection
if [ "$TIER" == "LOW" ]; then
    echo "   ⚠️  LOW Tier detected. Pulling highly optimized 8B models..."
    pull_model "hermes3:8b-llama3.1-q4_K_M"                 # Logic / The Dispatcher
    pull_model "dolphin3:8b"                        # Creative / The Sparring Partner

    # pull_model "huggingface.co/Sao10K/Llama-3.1-8B-Stheno-v3.4" # Roleplay / The Method Actor
    # Roleplay Sideload: Stheno 3.4 8B
    load_hf_model "stheno:8b" "https://huggingface.co/bartowski/Llama-3.1-8B-Stheno-v3.4-GGUF/resolve/main/Llama-3.1-8B-Stheno-v3.4-Q4_K_M.gguf"

elif [ "$TIER" == "MEDIUM" ]; then
    echo "   ⚠️  MEDIUM Tier detected. Pulling powerful MoE and 12B models..."
    pull_model "mistral-small:24b-instruct-2501-q4_K_M"                # Logic / The Dispatcher

    # pull_model "huggingface.co/dphn/Dolphin3.0-Mistral-24B"   # Creative / The Sparring Partner
    # Creative Sideload: Dolphin 3.0 24B
    load_hf_model "dolphin-3.0-mistral-24b-q4_K_M" "https://huggingface.co/bartowski/cognitivecomputations_Dolphin3.0-R1-Mistral-24B-GGUF/resolve/main/cognitivecomputations_Dolphin3.0-R1-Mistral-24B-Q4_K_M.gguf"

    # pull_model "hf.co/anthracite-org/magnum-v2-12b-GGUF:Q4_K_M"   # Roleplay / The Method Actor
    # Roleplay Sideload: Magnum v2 12B
    load_hf_model "magnum:12b" "https://huggingface.co/anthracite-org/magnum-v2-12b-gguf/resolve/main/magnum-12b-v2-q4_k.gguf"

elif [ "$TIER" == "HIGH" ]; then
    echo "   ⚠️  HIGH Tier detected. Pulling massive 70B models for heavy compute..."
    pull_model "hermes3:70b-llama3.1-q4_K_M"                      # Logic / The Dispatcher
    pull_model "llama3.1:70b"                             # Creative / The Sparring Partner

    # pull_model "hf.co/Sao10K/L3.1-70B-Euryale-v2.2-GGUF:Q4_K_M"   # Roleplay / The Enterprise Persona
    # Roleplay Sideload: Euryale v2.2 70B
    load_hf_model "euryale:70b" "https://huggingface.co/bartowski/L3.1-70B-Euryale-v2.2-GGUF/resolve/main/L3.1-70B-Euryale-v2.2-Q4_K_M.gguf"
fi

echo "   📦 Pulling Core System Models..."
pull_model "nomic-embed-text"    # Core: RAG Embeddings for Qdrant
pull_model "llama3.2-vision"     # Core: Vision / Multimodal (Replaces Llava)


# 4. PHASE 2: THE BUNKER (OFFLINE / PRODUCTION)
# ------------------------------------------------------------------------------
echo "🛑 Stopping Loader..."
docker stop $LOADER_CONTAINER >/dev/null

echo "🚀 [Step 5/5] Starting Production Environment (Offline/Isolated)..."

# Start Inference Engine (Ollama)
docker run -d \
  --name $PROD_CONTAINER \
  --restart unless-stopped \
  $GPU_STRATEGY \
  -p 127.0.0.1:11434:11434 \
  -v $VOLUME_NAME:/root/.ollama \
  $OLLAMA_IMAGE >/dev/null

echo "   🧠 Engine active (http://127.0.0.1:11434)"

# Start Vector Memory (Qdrant)
docker run -d \
       --name $QDRANT_CONTAINER \
       --restart unless-stopped \
       -p 127.0.0.1:6333:6333 \
       -p 127.0.0.1:6334:6334 \
       -v $QDRANT_VOLUME:/qdrant/storage \
       $QDRANT_IMAGE >/dev/null

echo "   💾 Memory active (http://127.0.0.1:6333) for http"
echo "   💾 Memory active (http://127.0.0.1:6334) for grpc"
echo ""
echo "========================================"
echo "🎉 SETUP COMPLETE"
echo "   AI Engine:       http://127.0.0.1:11434"
echo "   Vector DB HTTP:  http://127.0.0.1:6333"
echo "   Vector DB GRPC:  http://127.0.0.1:6334"
echo "========================================"
