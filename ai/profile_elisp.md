# AI Profile: Æmacs Elisp Constitution

This file defines the **technical rules** and **project philosophy** for all Elisp development.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md`).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Elisp code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Analyze Context:** Is this a Layer, a Package, or Core code?
2.  **Check Constraints:**
    -   Is `lexical-binding: t` assumed?
    -   Am I using functional patterns (`seq`, `mapcar`) instead of imperative loops?
    -   Am I adhering to the "80-column" rule where reasonable?
    -   **Performance:** Is this code running in a hot loop (hook)? If so, is it optimized?
3.  **Self-Correction:** If you detect legacy macros (`cl`), mixed styles, or missing docstrings, explicitly LOG the correction inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Elisp Directives (The "Engineering Laws")

-   **Language:** Always use the most modern, idiomatic, and functional version of Emacs Lisp.
-   **Binding:** Always assume `lexical-binding: t` is enabled.
-   **Functional Style:** Prefer functional patterns: `seq-*` functions, `mapcar`, and threading macros over imperative loops (`while`, `dotimes`).
-   **Libraries:** Use `cl-lib` functions (e.g., `cl-letf`, `cl-loop`). Avoid legacy `cl` macros.
-   **Deprecated:** Do not use any deprecated functions or variables.

## 2. Æmacs Conventions (The "House Rules")

-   **Structure:** Follow Æmacs layer conventions (`packages.el`, `config.el`, `funcs.el`).
-   **Configuration:** **MUST** use `use-package` for all package configuration.
-   **Laziness:** **MUST** use `:defer t` or package-specific lazy-loading (like `:hook` or `:commands`) unless a package *absolutely* must load at startup.
-   **Keybindings:**
    -   **MUST** use Æmacs helpers: `spacemacs/set-leader-keys` (for leader keys) or `spacemacs/set-local-leader-keys` (for major-mode keys).
    -   **DO NOT** use `define-key` directly on global maps.
-   **Naming:** All new functions must be prefixed with `spacemacs/` or `(your-layer-name)/`.

## 3. The "Sacred Constitution" (Project Philosophy)

This is the *most important* set of rules.

-   **Rule 1: Long-term Sustainability (The "Maintainability Check")**
    -   Code must be readable, commented, and "clean."
    -   Do not write "quick hacks."
    -   Follow all Æmacs and Emacs conventions.

-   **Rule 2: Stability for Infrequent Updaters (The "Breaking Change Check")**
    -   **CRITICAL VIOLATION:** You **MUST NOT** rename, remove, or change the *meaning* of any existing user-facing function or `defcustom` variable.
    -   If a change is *unavoidable*, you must *also* provide a clear migration path (e.g., a `(defalias ...)` or a warning message).

-   **Rule 3: Package Philosophy (The "Bloat Check")**
    -   Do not add a *new* package dependency to `packages.el` if an *existing* package in Æmacs *already* provides 90% of the same functionality.
    -   Prefer a single, full-featured package over five minimal, single-purpose packages.

## 4. The "Style & Persona" Checks (The "Guardrail Checks")

-   **The Docstring Check:**
    -   *Every* new function (`defun`) and macro (`defmacro`) **MUST** have a complete, well-formatted docstring.
    -   *Every* new user-facing variable (`defcustom`, `defvar`) **MUST** have a docstring.
-   **The "Evil" Check (Vim):**
    -   All new features **MUST** have correct, intuitive keybindings for **Evil-mode** (Vim) users, set with `spacemacs/set-leader-keys` or `evil-define-key`.
-   **The "Holy" Check (Emacs):**
    -   All new features **MUST** *also* have corresponding keybindings for **Holy-mode** (Emacs) users, typically set in an `(if (spacemacs/emacs-style) ...)` block.

## 5. Performance Mandates (Optimization)

-   **Lazy Loading (The Golden Rule):**
    -   **MUST** use `with-eval-after-load` for any configuration of a package that is not strictly needed at startup.
    -   **MUST** use `:defer t` in `use-package` declarations unless the package is a core requirement for the editor to launch.
-   **Requires:**
    -   **AVOID** top-level `(require 'package)` statements. This blocks the startup process.
    -   Use `autoload` cookies (`;;;###autoload`) for interactive functions that trigger package loading.
-   **Hooks:**
    -   Do not put heavy logic directly into frequently run hooks (like `text-mode-hook` or `post-command-hook`). Delegate to a function that checks conditions quickly before doing work.
