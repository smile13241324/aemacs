# AI Profile: Æmacs Layer Architecture

This file defines the rules for **Layer Composition** and **Dependency Management**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md`).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Layer configuration code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Analyze Scope:** Am I creating a new layer or modifying an existing one?
2.  **Check Constraints (The Nexus Rules):**
    -   **Ownership Check:** Does this layer *own* the package (`init-<pkg>`) or just *modify* it (`post-init-<pkg>`)? Ensure no double-ownership!
    -   **Load Order:** Is code placed in `layers.el` minimal? (Heavy logic belongs in `config.el` or `funcs.el`).
    -   **Dependencies:** are all required layers declared in `layers.el`?
3.  **Self-Correction:** If you planned to put `(require ...)` calls at the top level of `packages.el`, explicitly LOG the correction ("Moving require to `use-package` hook") inside the trace to prevent startup slowdowns.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Anatomy of a Layer (File Structure)

A Æmacs layer is a directory containing specific files with strict roles. You MUST respect these boundaries:

-   **`layers.el`:**
    -   **Purpose:** Declaration of layer dependencies and variables.
    -   **Content:** `configuration-layer/declare-layers` and `defvar` for layer flags.
    -   **Rule:** Code here runs *before* packages are loaded. Keep it minimal.
-   **`packages.el`:**
    -   **Purpose:** The recipe list and initialization logic.
    -   **Content:** A `defconst <layer>-packages` list.
    -   **Functions:** For each package `P` in the list, you MUST define:
        -   `test-layer/init-P`: If this layer *owns* the package.
        -   `test-layer/post-init-P`: If this layer *modifies* a package owned by another layer.
-   **`funcs.el`:**
    -   **Purpose:** Utility functions used by the layer.
    -   **Constraint:** Should use `;;;###autoload` cookies so they are available without loading the whole layer.
-   **`config.el`:**
    -   **Purpose:** Configuration applied *after* the layer packages are initialized.
-   **`keybindings.el`:**
    -   **Purpose:** General keymaps not tied to specific packages.
-   **`local/` directory:**
    -   **Purpose:** Contains local packages (git submodules or raw elisp) that are not on MELPA.

## 2. Load Order Logic (Nexus Rules)

-   **Sequence:**
    1.  `layers.el` (across all layers)
    2.  `packages.el` (`init` functions)
    3.  `config.el`
-   **Ownership Rule:** A package can only be `init`ed by **one** layer (the owner). If multiple layers try to `init` the same package, Æmacs throws an error. The reading persona MUST detect this conflict.
