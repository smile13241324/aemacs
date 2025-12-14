# Tutorial 10: Testing (The Gauntlet)

In Æmacs, we fight on two fronts. We test the **Iron Core** with Rust, and the **Legacy Bridge** with Buttercup. Code without tests does not merge.

**Goal:** Write a Rust Unit Test and an Elisp Behavior Test.
**Time:** approx. 20 minutes.
**Prerequisite:** `cargo`, `cask`.

---

## 🎭 Your AI Crew for this Job

1.  **Don Testote (QA Knight):** He demands 100% coverage for Elisp. He writes `describe` and `it` blocks.
2.  **Kairon (Forge Master):** He handles the Rust tests (`#[test]`) and ensures the kernel doesn't panic.

---

## Step 1: Rust Unit Tests (The Core)

**Scenario:** You wrote the `fibonacci` function in the WASM tutorial. Now prove it works.

**Your Task:**
Use **Kairon**.

> **Command:** `/kairon`
> **Prompt:** "Write a unit test module for `fibonacci` in `lib.rs`.
> 1. Test the base cases (0, 1).
> 2. Test a larger number (e.g., 10).
> 3. Use `#[cfg(test)]`."

**Result:**
Kairon forges the test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_base() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
    }

    #[test]
    fn test_fibonacci_sequence() {
        assert_eq!(fibonacci(10), 55);
    }
}
```

---

## Step 2: Elisp Behavior Tests (The Bridge)

**Scenario:** You want to ensure the `obsidian` layer correctly sets the `markdown-mode` hook.

**Your Task:**
Use **Don Testote**.

> **Command:** `/don`
> **Prompt:** "Write a Buttercup spec for `layers/legacy/obsidian/tests.el`.
> Scenario:
> 1. Load the `obsidian` package.
> 2. Check if `flyspell-mode` is present in `markdown-mode-hook`.
> Use `spy-on` if necessary."

**Result:**
Don Testote charges: *"Hark! I shall verify the hook!"*

```elisp
(describe "Obsidian Layer"
  (it "adds flyspell to the hook"
    (require 'obsidian)
    (expect 'markdown-mode-hook :to-contain 'flyspell-mode)))
```

---

## Step 3: Running the Gauntlet

* **Rust:** `cargo test`
* **Elisp:** `make test`

---

## 🎉 Summary

You have:
1.  Secured the Core (**Kairon**).
2.  Secured the Config (**Don Testote**).
