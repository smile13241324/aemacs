#!/bin/bash

# ==============================================================================
# Æmacs AI Server Teardown
# ==============================================================================
# Description:
#   Stops and removes all AI infrastructure containers and the isolated network.
#   NOTE: Persistent Volumes (Models & Memories) are PRESERVED.
#
# Usage:
#   ./teardown_aemacs_ai.sh
# ==============================================================================

# --- Configuration (Must match setup script) ---
PROD_CONTAINER="aemacs_ai_server"
LOADER_CONTAINER="aemacs_model_loader"
QDRANT_CONTAINER="aemacs_memory_qdrant"
NETWORK_NAME="aemacs_offline_net"

echo "========================================"
echo "🛑  Stopping Æmacs AI Infrastructure"
echo "========================================"

# 1. STOP & REMOVE CONTAINERS
# ------------------------------------------------------------------------------
echo "🧹 [Step 1/2] Stopping Containers..."

# We use 'docker rm -f' (force remove).
# This sends SIGKILL to the container and removes it immediately.
# We redirect errors to /dev/null in case they are already stopped.

if docker rm -f $PROD_CONTAINER 2>/dev/null; then
    echo "   ✅ AI Engine (Ollama) stopped."
else
    echo "   ℹ️  AI Engine was not running."
fi

if docker rm -f $QDRANT_CONTAINER 2>/dev/null; then
    echo "   ✅ Memory (Qdrant) stopped."
else
    echo "   ℹ️  Memory was not running."
fi

# Just in case the loader got stuck or was left running
docker rm -f $LOADER_CONTAINER 2>/dev/null || true


# 2. REMOVE NETWORK
# ------------------------------------------------------------------------------
echo "🔗 [Step 2/2] Removing Isolated Network..."

if docker network rm $NETWORK_NAME 2>/dev/null; then
    echo "   ✅ Network '$NETWORK_NAME' removed."
else
    echo "   ℹ️  Network was already clean."
fi

echo "========================================"
echo "💤 SYSTEM SLEEPING"
echo "   Resources freed. GPU is idle."
echo "   💾 NOTE: Your Models and Vector Data are SAFE in the docker volumes."
echo "========================================"
