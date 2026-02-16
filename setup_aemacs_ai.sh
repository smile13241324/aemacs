#!/bin/bash

# ==============================================================================
# Æmacs AI Server Setup - The Airlock Pattern
# ==============================================================================
# Description:
#   This script automates the deployment of a secure, local AI inference server.
#   It uses a three-phase "Airlock" approach:
#     1. Loader Phase (Online):   Downloads models to a persistent volume.
#     2. Bunker Phase (Offline):  Runs the API in a network-isolated container.
#     3. Memory Phase (Offline):  Runs Qdrant DB in the same isolated network.
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

# --- SMART GPU DETECTION ---
# We check if 'nvidia-smi' works AND if Docker has the nvidia-container-runtime configured.
# This prevents crashes on laptops where drivers are installed but Docker is not linked.
if command -v nvidia-smi &> /dev/null && docker info | grep -i "name: nvidia" &> /dev/null; then
    GPU_STRATEGY="--gpus=all"
    echo "🎮 NVIDIA GPU detected & Docker configured. AI Acceleration ENABLED."
else
    GPU_STRATEGY=""
    echo "⚠️  GPU not found or Docker not configured for NVIDIA."
    echo "    Running in CPU Mode. (Install 'nvidia-container-toolkit' for speed!)"
fi

echo "========================================"
echo "🛡️  Initializing Æmacs AI Infrastructure"
echo "========================================"

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

# Ensure the isolated network exists (Internal = No Internet Gateway)
if ! docker network ls | grep -q $NETWORK_NAME; then
    docker network create --driver bridge --internal $NETWORK_NAME
    echo "   ✅ Secure Network '$NETWORK_NAME' created."
else
    echo "   ✅ Secure Network '$NETWORK_NAME' exists."
fi


# 2. PHASE 1: THE LOADER (ONLINE)
# ------------------------------------------------------------------------------
echo "⬇️  [Step 2/5] Pre-fetching Docker Images..."
# Ensure we have the base images while online
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

# Wait for Ollama API to initialize inside the container
echo "   ⏳ Waiting for Ollama API to initialize..."
until docker logs $LOADER_CONTAINER 2>&1 | grep -q "Listening on"; do
    sleep 1
done
echo "   ✅ Loader is ready."


# 3. PULLING MODELS
# ------------------------------------------------------------------------------
echo "📦 [Step 4/5] Pulling Models..."

# Function to pull a model and handle errors
pull_model() {
    echo "   🔹 Pulling: $1 ..."
    docker exec -t $LOADER_CONTAINER ollama pull "$1"
}

# --- CORE MODELS (Required for Basic Operation) ---
pull_model "cognitivecomputations/dolphin-llama3.1" # General Chat / Logic / Coding / Uncensored
pull_model "nomic-embed-text"    # RAG Embeddings
pull_model "llama3.2-vision"     # Vision / Multimodal (Replaces Llava)

# --- OPTIONAL / SPECIALIZED MODELS (Uncomment as needed) ---

# [RAG Specialist - 35B]
# WARNING: Requires ~24GB VRAM. Optimized for long context RAG.
# pull_model "command-r"

# [Coding Heavyweight - 70B]
# WARNING: Requires 48GB+ VRAM (or offloading to RAM).
# pull_model "dolphin-llama3.1:70b"

# [Creative Writing / Roleplay - 70B]
# WARNING: Requires 48GB+ VRAM. Uncensored creative engine.
# pull_model "vanilj/midnight-miqu-70b-v1.5"


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
  --network $NETWORK_NAME \
  -p 127.0.0.1:11434:11434 \
  -v $VOLUME_NAME:/root/.ollama \
  $OLLAMA_IMAGE >/dev/null

echo "   🧠 Engine active (http://127.0.0.1:11434)"

# Start Vector Memory (Qdrant)
# Qdrant runs in the same isolated network, mapping port 6333 to localhost only.
docker run -d \
  --name $QDRANT_CONTAINER \
  --restart unless-stopped \
  --network $NETWORK_NAME \
  -p 127.0.0.1:6333:6333 \
  -v $QDRANT_VOLUME:/qdrant/storage \
  $QDRANT_IMAGE >/dev/null

echo "   💾 Memory active (http://127.0.0.1:6333)"

echo "========================================"
echo "🎉 SETUP COMPLETE"
echo "   AI Engine:   http://127.0.0.1:11434"
echo "   Vector DB:   http://127.0.0.1:6333"
echo "   Security:    ISOLATED (No outbound internet access)"
echo "========================================"
