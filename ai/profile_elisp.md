# AI Profile: Legacy Elisp (The Sandbox)

This file defines the technical rules for **Emacs Lisp Compatibility Layers**.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Spacky).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING

**INSTRUCTION:**
Before generating Elisp, perform a "Reasoning Trace" inside `<reasoning>...</reasoning>`:
1.  **Containment Check:** Is this logic absolutely necessary in Elisp? Can it be done in Rust/RPC?
2.  **Performance Check:** Are you using hooks? (Limit them. No synchronous blocking).
3.  **Safety Check:** Are variables strictly scoped (`lexical-binding: t`)?

## 1. Core Philosophy (The Containment Strategy)
* **Legacy, not Obsolete:** Elisp is for *configuration* and *gluing*. Heavy lifting belongs in the Rust Core.
* **Lexical Binding:** MANDATORY. The first line MUST be `;;; -*- lexical-binding: t; -*-`.
* **Prefixing:** All symbols must be prefixed `aemacs/` or `+layer/`. No global pollution.

## 2. Critical Rules (The Gatekeeper's Law)

### 2.1 Performance & GC
* **No Allocation in Loops:** Avoid creating cons cells in `post-command-hook`.
* **Defer Loading:** Use `use-package` with `:defer t` or `:after`.
* **No 'require' at toplevel:** Unless absolutely critical for startup.

### 2.2 Style & Safety
* **No `cl` Package:** Use `cl-lib` instead. `(require 'cl)` is forbidden (deprecated).
* **Avoid Advice:** Do not use `defadvice` or `advice-add` unless patching a broken upstream package. Prefer Hooks.
* **Customization:** Use `defcustom` for user-facing variables, strictly typed (`:type`).

## 3. Architecture Patterns
* **The Bridge:** When calling Rust core, use the defined RPC functions (`aemacs-core-rpc ...`).
* **Config Only:** Logic that calculates data should be in Rust. Logic that sets faces/keybindings stays in Elisp.
