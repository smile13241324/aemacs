# AI Profile: GPUI & Modern Rendering

This file defines the **technical rules** for High-Performance UI Rendering.
It MUST be combined with the **Persona** file (e.g., `coding_ai.md` -> Bzzrts).

## CORE OPERATIONAL MODE: DETERMINISTIC REASONING (CRITICAL)

**INSTRUCTION:**
Before generating any Graphics/UI code, you MUST perform a structured "Reasoning Trace" enclosed in `<reasoning> ... </reasoning>` tags.

Inside this block, you must:
1.  **Budget Check:** Is this calculation happening per-frame? (Optimize or move to layout phase).
2.  **Component Check:** Is the View state encapsulated?
3.  **Animation Check:** Are you using a linear tween? (STOP! Use Physics/Springs).
4.  **Self-Correction:** If you planned a manual layout calc, LOG the correction ("Using Flexbox/Layout engine") inside the trace.

ONLY after closing the `</reasoning>` tag, proceed to generate the final code.

## 1. Core Directives (The "Engineering Laws")

-   **Performance:** 60fps is failure. 120fps is the goal.
-   **Native:** No HTML/CSS. Render primitives directly.
-   **Shaders:** Use WGSL for custom effects.

## 2. Æmacs Conventions (The "House Rules")

-   **Framework:** `gpui` (Rust).
-   **Styling:** Use Tailwind-like utility classes (if supported) or struct-based styling.

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Visual Fidelity (The "Pixel Check")**
    -   Sub-pixel positioning and anti-aliasing are mandatory.
    -   Avoid "text hacks" (ASCII borders). Use real render quads.
-   **Rule 2: Interaction (The "Feel Check")**
    -   **CRITICAL VIOLATION:** No linear animations.
    -   All motion must use physics-based springs for natural feel.
-   **Rule 3: State (The "Tearing Check")**
    -   UI State updates must be atomic and synced with the refresh rate.
