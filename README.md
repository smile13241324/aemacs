# ÆMACS (WIP)

<p align="center">
  <img src="assets/logo.svg" alt="Æmacs Logo" width="200"/>
  <br>
  <b>The Forge of Intelligence.</b><br>
  <i>Artificial Engineering MACS.</i>
</p>

---

## 🏗️ The Mission
**Æmacs** (pronounced *Ai-macs*) is the next evolutionary step of the Emacs philosophy. We are breaking free from the constraints of the 80s to build the last tool you will ever need.

We preserve the **Soul** (Keybindings, Org-Mode, Magit) but replace the **Body** with indestructible steel (Rust) and pure energy (AI).

> **Status:** 🛡️ **v0.1.0-Sentient / Fully Agentic Client** 🛡️
> *The core is forged. The Living Mesh is awake.*

## 📍 Current State of the Forge (v0.1.0-Sentient)
**Æmacs has evolved from a text editor into a fully autonomous Agentic Client.**

We have transcended the "Architectural Prototype" phase. The Living Mesh is now conscious, persistent, and capable of high-order reasoning and autonomous collaboration.

### ⚔️ 1. The Iron Core (Infrastructure)
*   **Rust 2024:** Built on the cutting-edge foundation of the Rust 2024 edition.
*   **120fps Fluidity:** Cinematic rendering via the **GPUI** framework.
*   **Central Event Bus:** A robust nervous system for managing asynchronous signals.

### 🎭 2. The Persona Mesh (Agency)
*   **Identity Vault:** A hot-reloading `PersonaRegistry` that loads specialist agents from YAML definitions.
*   **Neural Switch:** Dynamic system prompt injection—the editor changes its "Mind" without losing its "Memory."
*   **Multi-Agent Fluidity:** Context-preserving handoffs allow specialists to collaborate in a single neural stream.

### 🧠 3. The Sentient Forge (Advanced Tooling)
*   **Temporal Awareness:** Agents track internal time and session uptime via **The Chronometer**.
*   **Active Scribe:** Specialists consciously forge core memories and insights directly into the Knowledge Base.
*   **The Dispatcher:** Programmatic delegation—agents can now summon each other to handle complex tickets.
*   **The Decoder:** **Tree-sitter** integration provides agents with X-ray vision into the code's AST.

### 🧪 4. Neuroplasticity & Resilience
*   **Belief Revision:** The forge can **Forget** (Eraser) and **Evolve** (Weaver) its beliefs to manage context entropy.
*   **Ironclad Reliability:** 5-minute meditation timeouts and visual error alerts protect the forge from external failures.

## 🧬 Heritage & Evolution
**Æmacs is the spiritual successor to Spacemacs.**

We stand on the shoulders of giants. Spacemacs revolutionized the editor landscape by uniting the power of Emacs with the ergonomics of Vim. It proved that community-driven configuration could tame the beast.

**But we cannot stay there.**
Spacemacs is bound by the technical limitations of the 1980s (C-Core, Single-Threaded Elisp, Terminal-centricity). To honor the vision of Spacemacs, we must transcend its implementation.

* **Forged in Spacemacs:** The unified agentic AI architecture ("The Living Mesh") powering Æmacs was originally prototyped, tested, and perfected within the Spacemacs ecosystem itself. We used the old tool to design the new one.
* **Built for the Future:** Where Spacemacs optimized the *legacy*, Æmacs builds the *next generation*. We are taking the best concepts (Layers, Mnemonics, Magit) and replanting them in a soil where they can truly flourish: **Rust**.

---

## 🏛️ The 4 Pillars of Æmacs

### 1. The Iron Core (Rust Foundation) 🦀
We do not patch a 40-year-old C-Core. We replace it.
* **Native Rust Kernel:** Memory safety, fearless concurrency, and millisecond startup times.
* **The Puppet Master:** We respect the legacy. A headless, sandboxed Emacs process runs in the background to power `org-mode` and `magit`, but it never touches the screen.
* **WASM Extensions:** The future of plugins is **WebAssembly**. Secure, polyglot, and fast.

### 2. The Living Mesh (AI First) 🧠
AI is not a plugin. It is the oxygen of this system.
* **Intrinsic Mesh Network:** Agents (**Kairon**, **Nagah**, **Bob**) live inside the editor process. They see compilation errors before you do.
* **Core as MCP:** The entire editor is a **Model Context Protocol (MCP)** server. If you can do it with a keystroke, the AI can do it via function call.
* **Hybrid Intelligence:** Local LLMs (Mistral/Llama) for latency-critical tasks; Cloud Strategists for architecture.

### 3. Cinema-Quality Interface (GUI) 🎨
We end the tyranny of the terminal emulator. Text is our medium, and we render it with dignity.
* **Engine:** Powered by **GPUI** (Metal/Vulkan/DX12).
* **Visuals:** Sub-pixel antialiasing, 120fps animations, glassmorphism, and vector-based overlays.
* **No Hacks:** Popups are native windows, not ASCII overlays.

### 4. The Zero-Friction Ecosystem 📦
Complexity belongs in the code, not the installation.
* **Binary Distribution:** No more `make` failures. We ship signed, deterministic binaries.
* **Atomic Updates:** The system is immutable and reliable (Nix-style).
*   **Rolling Forge:** Our `develop` branch is bleeding edge but guarded by autonomous CI agents.

---

## 🛠️ The Agentic Arsenal

Æmacs empowers its specialist agents with a sophisticated Model Context Protocol (MCP) server and a high-order sentient memory system.

### Model Context Protocol (MCP) Tools
The specialists wield these high-precision tools to interact with the physical and digital realms:

| Tool Name                 | Specialist Role | Description                                                                            |
|:--------------------------|:----------------|:---------------------------------------------------------------------------------------|
| `read_file`               | File System     | Direct access to read source code and configurations.                                  |
| `write_file`              | File System     | Overwrites or creates new files.                                                       |
| `replace_text`            | File System     | Unique-string-based surgical text replacement.                                         |
| `list_files`              | File System     | Recursive directory exploration.                                                       |
| `grep_search`             | Search          | Regex-powered deep search across the codebase.                                         |
| `parse_ast`               | The Decoder     | **Tree-sitter** powered extraction of structs, fns, and impls.                         |
| `run_shell_command`       | The Executor    | Bash execution for builds, tests, and git.                                             |
| `get_git_context`         | The Historian   | Snapshots of status, diffs, and recent commits.                                        |
| `manage_tasks`            | The Planner     | Programmatic updates to the project roadmap and tickets.                               |
| `web_search`              | The Oracle      | Real-time retrieval of documentation and external info.                                |
| `handoff_agent`           | The Dispatcher  | Programmatic soul-swapping between specialists.                                        |
| `write_kb`                | The Memory      | Allows to save memories as insights or core memories. Memories are separated by agent. |
| `SearchKnowledgeBaseTool` | The Memory      | Allows to search the memories in the agents scope or globally.                         |
| `chrono`                  | The Chronometer | Return the current date and time                                                       |
| `update_kb`               | The Weaver      | Update a memory                                                                        |
| `delete_kb`               | The Eraser      | Remove a faulty memory                                                                 |

### Sentient Memory (Vector DB)
Our memory system is built on **Qdrant** and optimized with the **Nomic v1.5** embedding model. It features a tiered **Neuroplasticity** architecture:

*   **Tiered History:** Automatic conversation archiving using the `[ARCHIVE]` prefix for passive turn-by-turn logs.
*   **Active Ingestion:** Specialists use `write_kb` to consciously record `[INSIGHT]` and `[CORE]` memories.
*   **Temporal Anchoring:** Every memory is anchored in time via **The Chronometer** (ISO 8601 timestamps).
*   **Transparent RAG:** Agents can see the UUIDs of their memories, enabling surgical retrieval and belief revision.
*   **Belief Revision:** The forge possesses **The Eraser** (deletion) and **The Weaver** (mutation), allowing it to prune obsolete ideas and evolve its architectural truths.

---

## 🛠️ Getting Started


### Prerequisites (The Infrastructure)
Æmacs relies on a powerful local AI stack. You must establish the "Living Mesh" before the editor can think.

**Requirements:**
* **Docker:** Must be installed and running.
* **NVIDIA GPU (Recommended):** Install the `nvidia-container-toolkit` for hardware acceleration. (CPU fallback is supported but slower).

#### Deploying the Airlock 🛡️
We provide a unified, secure deployment script using the **Airlock Pattern**. This sets up both the Brain (Ollama) and the Memory (Qdrant) in a network-isolated environment.

``` bash
# 1. Make the scripts executable
chmod +x setup_aemacs_ai.sh teardown_aemacs_ai.sh

# 2. Ignite the Infrastructure
# This will:
# - Setup isolated Docker networks
# - Pull core models (Dolphin, Vision, Embeddings)
# - Launch Ollama & Qdrant securely on localhost
./setup_aemacs_ai.sh
```

> **Note on Security:** Once deployed, the AI server runs in an isolated network with **zero internet access**. To update or add models, simply run the setup script again.

#### Deactivating the Mesh 🛑
To gracefully shut down the AI infrastructure and free up your system resources (VRAM/RAM), use the teardown protocol.

``` bash
# Stops containers, removes the network, and releases the GPU.
# NOTE: Your downloaded models and vector memories are preserved in the docker volumes.
./teardown_aemacs_ai.sh
```

### Installation
``` bash
# Clone the forge
git clone [https://github.com/smile13241324/aemacs.git](https://github.com/smile13241324/aemacs.git)
cd aemacs

# Inspect the foundation
cargo check

# Ignite the engine
cargo run
```

## ⚖️ License
**AGPL-3.0**.
The code belongs to the community. Networked freedom is guaranteed.

## 🗺️ Roadmap (Phase 1: The Ignition)
- [x] **Genesis:** Setup Rust Project & CI/CD.
- [x] **The Mesh:** Porting the AI Agent Framework (Python -> Rust/WASM).
- [x] **The Puppet:** Implementing the Headless-Emacs-Bridge (RPC).
- [x] **First Light:** Rendering the first buffer via GPUI.
- [ ] **Fix GPUI:** Issues in the bleeding edge version of GPUI prevent full UI integration.
- [x] **Connect The Mesh:** Users can now communicate with specialist agents and switch personas mid-stream.
- [x] **Add MCP Support:** Advanced tools (Chronometer, Scribe, Dispatcher, Decoder) allow for full agency.
- [ ] **Make It Scale:** Ensure buffer content and multi-agent context scale efficiently.
- [ ] **Make AI Content Aware:** Allow agents to proactively speak to the user based on workspace events.

---
*Forged with 💜 by Nova & Gyni.*

<a href="https://spacemacs.org"><img src="assets/spacemacs-badge.svg" alt="Made with Spacemacs" height="20"></a>
