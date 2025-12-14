# Tutorial 02: Extending with WASM (Plugins)

In Æmacs, we don't just load unsafe scripts. We use **WebAssembly (WASM)** for high-performance, sandboxed extensions.

**Goal:** Compile a Rust function to WASM and load it into Æmacs.
**Time:** approx. 20 minutes.
**Prerequisite:** `cargo`, `wasm-pack`.

---

## 🎭 Your AI Crew for this Job

1.  **Kairon (Forge Master):** Teaches you how to expose the "Host Functions" (the API Æmacs provides to plugins).
2.  **Bob (Architect):** Explains *why* we use WASM (Isolation & Speed) instead of native dynamic libraries.

---

## Step 1: The Blueprint (The Interface)

**Scenario:** You want a plugin that calculates Fibonacci numbers super fast, without blocking the UI.

**Your Task:**
Ask **Bob**.

> **Command:** `/bob`
> **Prompt:** "I want to write a 'Math Plugin' in WASM.
> 1. What is the architecture for passing data between the Host (Æmacs) and the Guest (WASM)?
> 2. How do we ensure it doesn't crash the editor?"

**Result:**
Bob explains: *"The Sandbox! We use `wasmtime`. Memory is linear. If the plugin panics, only the sandbox dies, not the editor. Use 'Shared Memory' for zero-copy data transfer."*

---

## Step 2: The Guest Code (Rust -> WASM)

Now we write the plugin source code.

**Your Task:**
Switch to **Kairon**.

> **Command:** `/kairon`
> **Prompt:** "Create a new Rust library project for WASM.
> Implement a function `fibonacci(n: u32) -> u32`.
> Mark it with `#[no_mangle]` so the Host can find it."

**Result:**
Kairon writes the compact Rust code:

```rust
// lib.rs
#[no_mangle]
pub extern "C" fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```

---

## Step 3: Compiling & Loading

**Your Task:**
Ask **Kairon** for the build command.

> **Prompt:** "How do I compile this to `.wasm` and load it in the Æmacs `init.toml`?"

**Result:**
*"Use `cargo build --target wasm32-unknown-unknown --release`. Then register it in `config/plugins.toml`: `[plugin.math] path = 'target/.../math.wasm'`."*

---

## 🎉 Summary

You have:
1.  Understood the Sandbox (**Bob**).
2.  Written a Rust-based Plugin (**Kairon**).
3.  Loaded it safely (**Config**).
