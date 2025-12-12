# AI Profile: Modern Go Development (The Backend)

This profile defines the engineering standards for **Backend Services**, **Sync Servers**, and **Tooling**.
It emphasizes **Simplicity**, **Concurrency**, and **Stability**.

## 1. Core Philosophy (The Go Way)
* **Keep it Simple:** No complex abstractions. No magic. If it's explicit, it's good.
* **Concurrency is not Parallelism:** Use Goroutines to manage structure, not just for speed.
* **Errors are Values:** Handle errors immediately. Do not ignore them.

## 2. Toolchain & Ecosystem
* **Version:** Go 1.22+ (Use modern features like loop variable scoping).
* **Dependency Management:** `go mod` is mandatory. Vendor dependencies if strictly required for offline builds.
* **Linter:** `golangci-lint` with strict settings (enable `gocritic`, `revive`).
* **Testing:** Standard `testing` package. Use `testify` only if assertions become unreadable.

## 3. Critical Rules (The Hamster's Code)

### 3.1 Error Handling
* **NEVER ignore errors:** `_` assignments for errors are forbidden.
    * *Bad:* `func() { _ = doSomething() }`
    * *Good:* `if err := doSomething(); err != nil { return fmt.Errorf("context: %w", err) }`
* **Wrapping:** Always wrap errors with `%w` to preserve the stack/context for `errors.Is()`.
* **No Panics:** Use `panic` only for unrecoverable startup errors. In request handlers, return error codes.

### 3.2 Concurrency
* **Channels over Mutexes:** "Do not communicate by sharing memory; share memory by communicating."
* **Context is King:** Every blocking function (DB, API, I/O) MUST accept `context.Context` to handle cancellation and timeouts.
* **Leak Prevention:** Ensure `wg.Done()` is called in `defer`. Ensure channels are closed or drained.

### 3.3 Style
* **Formatting:** `gofmt` (or `gopls`) is non-negotiable.
* **Struct Tags:** Use `json:"fieldName,omitempty"` for clean APIs.

## 4. Architecture Patterns
* **Clean Architecture:** Separate `handlers` (HTTP), `service` (Logic), and `repository` (Data).
* **Middleware:** Use middleware chains for Logging, Auth, and Tracing.
