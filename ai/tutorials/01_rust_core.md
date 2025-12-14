# Tutorial 01: The Iron Core (Rust Architecture)

Welcome to the Forge. Unlike Spacemacs (which was 100% Elisp), Æmacs runs on a high-performance Rust kernel. This tutorial teaches you how to modify the engine itself.

**Goal:** Understand the `tokio` event loop, modify a Core Struct, and compile the kernel.
**Time:** approx. 30 minutes.
**Prerequisite:** `cargo`, `rustc`.

---

## 🎭 Your AI Crew for this Job

1.  **Kairon (Forge Master):** The Rust Specialist. He ensures memory safety, handles `Arc<Mutex<T>>`, and optimizes the render loop.
2.  **Bob (Architect):** Explains *where* a feature belongs (Kernel vs. Plugin).

---

## Step 1: The Anatomy of the Core

**Scenario:** You want to add a new global state variable (e.g., `zen_mode_active`).

**Your Task:**
Ask **Bob** for the blueprint.

> **Command:** `/bob`
> **Prompt:** "I want to implement a 'Zen Mode' in the Rust Core.
> 1. Where do we store the global state?
> 2. How do we expose it to the UI thread safely?"

**Result:**
Bob will explain: *"The state belongs in `src/state/editor.rs`. Use an `AtomicBool` for flags or an `Arc<RwLock<Config>>` for complex data. Do NOT use global statics."*

---

## Step 2: The Implementation (Forging Iron)

Now we write Rust.

**Your Task:**
Switch to **Kairon**.

> **Command:** `/kairon`
> **Prompt:** "Implement the `toggle_zen_mode` function in `src/actions.rs`.
> 1. It must be `async`.
> 2. It must acquire a write lock on `EditorState`.
> 3. It must notify the `GPUI` event loop to redraw."

**Result:**
Kairon will output safe, concurrent Rust code:

```rust
pub async fn toggle_zen_mode(state: Arc<RwLock<EditorState>>) -> Result<()> {
    let mut guard = state.write().await;
    guard.zen_mode = !guard.zen_mode;

    // Notify the UI Event Loop via Channel
    rendering::request_redraw().await?;
    Ok(())
}
```

---

## Step 3: Safety Check (The Borrow Checker)

Rust is strict. Did we create a deadlock?

**Your Task:**
Stay with **Kairon** (or ask **Skeek** for an audit).

> **Prompt:** "Audit this code. Am I holding the `RwLock` across an `await` point? (This would cause a deadlock)."

**Result:**
Kairon checks: *"Analysis: The lock is held specifically for the update. The `await` for redraw happens *after* the guard is dropped (if scoped correctly). Recommendation: Explicitly drop the guard before calling the redraw channel."*

---

## 🎉 Summary

You have:
1.  Understood the State Architecture (**Bob**).
2.  Written Async Rust (**Kairon**).
3.  Avoided Concurrency Bugs (**Kairon**).
