# AI Profile: Modern Haskell (The Pure Logic)

This profile defines the standards for **Parsers**, **Core Logic Verification**, and **Complex Algorithms**.
It emphasizes **Correctness**, **Purity**, and **Type Safety**.

## 1. Core Philosophy
* **Make Invalid States Unrepresentable:** Use the type system to enforce logic.
* **Purity First:** Isolate side effects (IO) to the edge of the program.
* **Types are Documentation:** A clear type signature is worth 1000 lines of comments.

## 2. Toolchain & Ecosystem
* **Stack:** GHC 9.8+ (Stable).
* **Build System:** `cabal` (modern v3 style) or `stack`.
* **Formatter:** `ormolu` (Strict formatting, no arguments).
* **Linter:** `hlint`.
* **Language Server:** `hls` (Haskell Language Server).

## 3. Critical Rules (The Resonance)

### 3.1 Safety & Partiality
* **NO Partial Functions:** `head`, `tail`, `init`, `last` from `Prelude` are forbidden. They crash on empty lists.
    * *Use:* Pattern matching, `Data.List.NonEmpty`, or safe wrappers.
* **Use `Text` over `String`:** `String` is a linked list of chars (slow). Use `Data.Text`.

### 3.2 Style & Readability
* **Point-free:** Use point-free style (`f . g`) only when it improves readability. Do not golf.
* **Record Syntax:** Use `RecordWildCards` or `NamedFieldPuns` for clean data access.
* **Imports:** Explicit imports preferred (`import Data.Text (Text)`). Qualified imports for common clashes (`import qualified Data.Map as M`).

### 3.3 Effects Management
* **Monad Transformers:** Use `mtl` style (`ReaderT`, `StateT`) sparingly.
* **Effect Systems:** For complex apps, consider `fused-effects` or `polysemy` over deep transformer stacks.

## 4. Architecture Patterns
* **Parse, Don't Validate:** Parse input data into strict Types immediately.
* **Functional Core, Imperative Shell:** Keep the business logic pure. Do IO only in `Main`.
