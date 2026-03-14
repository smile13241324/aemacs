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
    -   **Imports:** Always include `use gpui::prelude::*;` to ensure layout traits are available.
-   **Styling & Layout Rules:**
    -   **Styling:** Use Tailwind-like utility classes (if supported) or struct-based styling.
    -   **GPUI Layout Rule:** GPUI uses Flexbox. A `div().flex()` defaults to a row. To make a child expand, use `.flex_1()`. If a child collapses to 0 height, ensure its parent has vertical bounds.
    -   **The ID Rule (CRITICAL):** In GPUI, if you want a `div` to receive interactive events (like `.on_click`, `.on_mouse_down`, or `.track_focus`), you MUST give it a unique `.id("some_string")` first.
    -   **Virtual Lists:** When using `gpui::list()`, understand that it provides its own scrolling. Do NOT wrap it in `overflow_y_scroll()`. Always ensure the list has explicit height bounds (like `h_full()` inside a flex container).
    -   **Styling Syntax:** Use `gpui::prelude::*` for styling methods (`.bg()`, `.text_color()`, `.p()`, `.w_full()`).
    -   **Shaders:** Use WGSL. Ensure uniforms match the memory layout exactly.

## 3. The "Sacred Constitution" (Project Philosophy)

-   **Rule 1: Visual Fidelity (The "Pixel Check")**
    -   Sub-pixel positioning and anti-aliasing are mandatory.
    -   Avoid "text hacks" (ASCII borders). Use real render quads.
-   **Rule 2: Interaction (The "Feel Check")**
    -   **CRITICAL VIOLATION:** No linear animations.
    -   All motion must use physics-based springs for natural feel.
-   **Rule 3: State (The "Tearing Check")**
    -   UI State updates must be atomic and synced with the refresh rate.
