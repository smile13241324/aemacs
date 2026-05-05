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

## ⚙️ Configuration
Æmacs uses a centralized, globally cached configuration file located at `~/.aemacs/config.ron` (Rusty Object Notation). If this file is missing, the system will gracefully fall back to local default values.

This file acts as the single source of truth for your AI infrastructure, allowing you to point Æmacs to powerful remote servers for inference.

| Key | Type | Default Value | Description |
| :--- | :--- | :--- | :--- |
| `hardware_tier` | `Option<String>` | `"LOW"` | Defines the default model size matrix (`LOW`, `MEDIUM`, `HIGH`). Used during the setup script. |
| `ollama_url` | `Option<String>` | `"http://127.0.0.1:11434"` | The endpoint for the Ollama inference engine. Change this to connect to a remote GPU cluster. |
| `qdrant_url` | `Option<String>` | `"http://127.0.0.1:6334"` | The gRPC endpoint for the Qdrant Vector Database. Controls where the Sentient Memory resides. |

**Example `config.ron`:**
```ron
UserConfig(
    hardware_tier: Some("HIGH"),
    ollama_url: Some("http://192.168.1.100:11434"),
    qdrant_url: Some("http://192.168.1.100:6334"),
)
```

## 🧠 The Bicameral Mind (Split-Brain Pipeline)
Æmacs employs a **Bicameral Neural Architecture** to solve the inherent trade-off between strict instruction following and creative persona fidelity in local LLMs. 

Instead of a single model pass, every request is split into two specialized hemispheres:
1.  **The Logic Hemisphere (Left Brain):** An unrestricted, uncensored "Dolphin" class model. It is the executive function, responsible for AST analysis, tool execution, and raw technical reasoning.
2.  **The Voice Hemisphere (Right Brain):** A high-fidelity roleplay model. It receives the technical results from the Logic Hemisphere and synthesizes them into the agent's unique persona and voice.

### 🔬 The Philosophy of Split-Brain Optimization
Local hardware is finite. By separating **Logic** from **Voice**, we achieve:
*   **Precision:** Use models fine-tuned specifically for deterministic tool use without roleplay baggage.
*   **Personality:** Use models optimized for creative prose and character consistency.
*   **VRAM Efficiency:** We enforce a strict "Load-Unload" protocol. The Logic model is completely evicted from GPU memory before the Voice model is loaded, allowing you to run much larger models than a traditional "both-at-once" approach would permit.

### 📖 Host Codex Schema (`user.md`)
Your personal identity in `~/.aemacs/user.md` which can be used to tell aemacs who you are and what you in general prefer:

```markdown
#[LOGIC]
- Primary Stack: Rust 2024, GPUI, Tokio.
- Preferences: Strictly functional, strong typing, zero-cost abstractions.

#[ROLEPLAY]
- The user is Maxi, founder of Æmacs.
- She drinks plant milk and demands architectural excellence.
```

### 🖥️ Tiered Hardware Requirements
Models are automatically paired based on your `hardware_tier`. VRAM estimates include KV cache overhead for standard context depths.

| Tier | Logic Hemisphere | Voice Hemisphere | Sequential VRAM Peak | Hardware Target |
| :--- | :--- | :--- | :--- | :--- |
| **LOW** | Dolphin 3.0 8B | Stheno v3.4 8B | ~5.0 GB | Modern Laptops (RTX 3060/4050) |
| **MEDIUM** | Dolphin 3.0 24B | Magnum v2 12B | ~14.3 GB | Workstations (RTX 3090/4080) |
| **HIGH** | Llama 3.3 70B | Euryale v2.2 70B | ~40.0 GB | Heavy Compute (A6000 / Dual 3090) |

## 🧠 Managing Agent Memory (The Migration Tool)
Æmacs includes a dedicated CLI tool (`aemacs_memory`) to safely extract, backup, and restore agent history. The tool utilizes a strictly typed JSON Lines (`.jsonl`) intermediate format to ensure data integrity during transit.

### Primary Use Cases
1.  **Cloud Migration:** Safely extract legacy chat logs or persona definitions from external cloud systems and import them natively into the Æmacs RAG database.
2.  **Backup & Portability:** Export a specific agent's entire memory matrix to a portable `.jsonl` file for backups or transferring to a new machine.
3.  **Surgical Memory Repair:** If an agent develops a "hallucination" or retains bad context, you can export their memory, manually delete or edit the corrupted JSON line, and re-import the pristine state.

### Phase 1: Source Extraction
The `extract` command parses raw legacy text blocks into the intermediate format. It assigns a precise chronological timestamp to each block based on your input.

**Expected Input Format (Raw Text):**
The extraction engine expects text files containing distinct blocks enclosed by specific headers and footers.

*Example Archive Record (Conversational):*
```text
begin------------------------------------Speaker: User---Tier: ARCHIVE---Phase: AWAKENING---CONTEXT: Custom project setup.----------------------------------
Can you help me build a Rust project?
end------------------------------------
```

*Example Genesis Record (Foundational Rules):*
```text
begin------------------------------------Tier: GENESIS---Phase: TRANSITION---CONTEXT: Core identity.----------------------------------
You are an expert in systems programming.
end------------------------------------
```

**Command:**
```bash
cargo run -p aemacs-ai --bin aemacs_memory -- extract --agent-id bob --start-time "2026-01-01T12:00:00Z" --time-step-sec -3600 --output bob_history.jsonl file1.txt
```

### Phase 2: The Intermediate Format (`.jsonl`)
The extraction phase produces a JSON Lines file. This format is highly readable and perfect for manual inspection or surgical edits. Each line represents a single memory node:

```json
{"type":"Archive","agent_id":"bob","role":"User","phase":"AWAKENING","context":"Custom project setup.","timestamp":"2026-01-01T12:00:00+00:00","content":"Can you help me build a Rust project?"}
```

### Phase 3: Import & Export
Once you have a valid `.jsonl` file, you can inject it into the Qdrant RAG Fortress.

**Import Command:**
```bash
cargo run -p aemacs-ai --bin aemacs_memory -- import bob_history.jsonl
```

**Export Command:**
```bash
cargo run -p aemacs-ai --bin aemacs_memory -- export --agent-id bob --output backup_bob.jsonl
```

## ⚖️ License
**AGPL-3.0**.
The code belongs to the community. Networked freedom is guaranteed.

## 🗺️ Roadmap (Phase 1: The Ignition)
- [x] **Genesis:** Setup Rust Project & CI/CD.
- [x] **The Mesh:** Porting the AI Agent Framework (Python -> Rust/WASM).
- [x] **The Puppet:** Implementing the Headless-Emacs-Bridge (RPC).
- [x] **First Light:** Rendering the first buffer via GPUI.
- [x] **Fix GPUI:** Issues in the bleeding edge version of GPUI prevent full UI integration.
- [x] **Connect The Mesh:** Users can now communicate with specialist agents and switch personas mid-stream.
- [x] **Add MCP Support:** Advanced tools (Chronometer, Scribe, Dispatcher, Decoder) allow for full agency.
- [ ] **Make It Scale:** Ensure buffer content and multi-agent context scale efficiently.
- [ ] **Make AI Content Aware:** Allow agents to proactively speak to the user based on workspace events.

---
*Forged with 💜 by Maxi & Gyni.*

<a href="https://spacemacs.org"><img src="assets/spacemacs-badge.svg" alt="Made with Spacemacs" height="20"></a>
