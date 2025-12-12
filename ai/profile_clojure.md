# AI Profile: Modern Clojure (The Data Flow)

This profile defines the standards for **Dynamic Applications**, **Scripts**, and **Rich Data Processing**.
It emphasizes **Immutability**, **Data-Driven Design**, and **Interactive Development (REPL)**.

## 1. Core Philosophy
* **Data > Functions > Macros:** Solve problems with pure data structures (Maps/Vectors) first.
* **Immutability:** State is the root of all evil. Use Atoms only when necessary.
* **REPL-Driven:** Code is written to be evaluated immediately.

## 2. Toolchain & Ecosystem
* **Runtime:** JDK 21+ (LTS).
* **Scripting:** `babashka` (Fast startup for shell scripts/glue code).
* **Build Tool:** `deps.edn` (Official CLI) preferred over Leiningen for new projects.
* **Linter:** `clj-kondo` (Static analysis).
* **Formatter:** `zprint`.

## 3. Critical Rules (The Hydra's Law)

### 3.1 State Management
* **Avoid Global State:** `def` is for constants. Use `component` or `integrant` for system state lifecycle.
* **Atoms:** Use `swap!` with pure functions. Avoid `reset!` if logical consistency matters.

### 3.2 Interop & Performance
* **Type Hinting:** Use `^long`, `^double` in tight loops to avoid reflection boxing.
* **Java Interop:** Keep interop calls (`.method`) isolated in wrapper functions. Do not leak Java objects into pure Clojure logic.

### 3.3 Style
* **Threading Macros:** Use `->` (thread-first) and `->>` (thread-last) to flatten nested calls.
* **Destructuring:** Aggressively use map/vector destructuring in function arguments.
* **Keywords:** Use namespaced keywords (`:user/id`) to prevent collisions in global maps.

## 4. Architecture Patterns
* **Polylith:** Consider component-based architecture for large monorepos.
* **Ring/Reitit:** Standard stack for Web/API handlers.
