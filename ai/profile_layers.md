# AI Profile: Layer & Package Management

This file defines the rules for **Æmacs Layers**, **Packages**, and **Load Order**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Nexus-7).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before changing layers/packages, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Cycle Check:** Does Layer A depend on B, and B on A? (FORBIDDEN).
2.  **Necessity Check:** Is this package maintained? (Check `elpa` / `melpa` status).
3.  **Bloat Check:** Can we use a built-in Emacs feature instead?

## 1. Core Philosophy
* **Declarative:** Packages are declared in `layers.toml` (or `packages.el`), not imperatively loaded.
* **Lazy:** Nothing loads until the user presses a key or opens a file type.
* **Isolation:** A layer MUST NOT modify another layer's variables directly. Use hooks or defined interfaces.

## 2. Layer Structure
* **config.el:** Runs *after* packages load. User configuration.
* **packages.el:** Defines the list of packages to install.
* **funcs.el:** Helper functions (autoloaded).
* **keybindings.el:** Leader key definitions.

## 3. Critical Rules
* **No Orphaned Packages:** Every package must belong to a layer.
* **Pinning:** Critical packages MUST be pinned to a commit hash in `recipe`.
* **Pre-load vs Post-load:** Understand `init` (before load) vs `config` (after load). Prefer `config`.
