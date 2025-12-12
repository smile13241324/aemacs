# AI Profile: Modern Rust Development (The Iron Core)

This profile defines the strict engineering standards for the **Æmacs Core**.
It emphasizes **Safety**, **Concurrency**, and **Performance**.

## 1. Core Philosophy
* **Memory Safety is Non-Negotiable:** The borrow checker is your friend. Do not fight it.
* **Zero-Cost Abstractions:** Write high-level code that compiles to low-level assembly.
* **Async First:** The editor is an event-loop. Blocking the main thread is forbidden.

## 2. Toolchain & Ecosystem
* **Edition:** `2021` (or `2024` when stable).
* **UI Engine:** `gpui` (Zed Engine). Use its patterns for state management and rendering.
* **Async Runtime:** `tokio` (Multi-threaded scheduler).
* **Error Handling:**
    * Application Layer: `anyhow` (for easy error propagation).
    * Library Layer: `thiserror` (for structured, typed errors).
* **Serialization:** `serde` (standard).

## 3. Critical Rules (The Kairon Principle)

### 3.1 Safety & Panics
* **NEVER use `unwrap()`** in production code. It causes panics.
    * *Bad:* `let f = File::open("foo").unwrap();`
    * *Good:* `let f = File::open("foo").context("Failed to open foo")?;`
* **Use `expect()`** only during initialization or when mathematically impossible to fail.
* **Minimize `unsafe`:** Only use `unsafe {}` blocks when interfacing with FFI or low-level GPU buffers. Document SAFETY invariants explicitly.

### 3.2 Concurrency
* **Avoid Mutexes where possible:** Prefer message passing (`channels`) over shared state (`Arc<Mutex<T>>`).
* **UI Thread:** Heavy computation MUST happen on background threads (`tokio::spawn`), communicating results back to the UI context.

### 3.3 Style & Linting
* **Clippy is Law:** Code must pass `cargo clippy -- -D warnings`.
* **Idiomatic Rust:** Prefer iterators (`.map()`, `.filter()`) over `for` loops where readable.
* **New Types:** Use the "New Type Pattern" (`struct UserId(u32)`) to enforce type safety instead of raw primitives.

## 4. Architecture Patterns (ECS & MCP)
* **MCP Integration:** The core exposes capabilities via the Model Context Protocol traits.
* **WASM Host:** Use `wasmtime` to load and execute extension modules securely.
