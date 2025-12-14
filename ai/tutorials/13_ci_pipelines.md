# Tutorial 13: Designing CI/CD Pipelines

The Forge must never go cold. We use **GitHub Actions** to build the Rust Core and test the Legacy Bridge.

**Goal:** Create a `ci.yml` that caches Rust dependencies to speed up builds.
**Time:** approx. 20 minutes.
**Prerequisite:** `profile_ci_github.md`.

---

## 🎭 Your AI Crew for this Job

1.  **Vala Grudge-Keeper (CI Dwarf):** She writes the YAML. She demands caching and strict permissions.
2.  **Reginald Shoe (Strategy):** He defines the stages (Build -> Test -> Release).

---

## Step 1: The Strategy (The Procession)

**Your Task:**
Ask **Reginald Shoe**.

> **Command:** `/reginald`
> **Prompt:** "We need a pipeline for the Rust Core.
> Stages: Format Check, Clippy, Test, Build Release.
> How should we order this?"

**Result:**
Reginald groans: *"Format first... fast fail. Then Clippy. Then Test. Build last. Efficient."*

---

## Step 2: The Implementation (The Anvil)

**Your Task:**
Switch to **Vala**.

> **Command:** `/vala`
> **Prompt:** "Write `.github/workflows/rust.yml`.
> 1. Use `dtolnay/rust-toolchain`.
> 2. **CRITICAL:** Use `Swatinem/rust-cache` (Rust builds are slow!).
> 3. Run `cargo test`."

**Result:**
Vala hammers out the YAML:

```yaml
name: Iron Core
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2 # Vala demands speed!
      - name: Test
        run: cargo test
```

---

## 🎉 Summary

You have:
1.  Planned the Flow (**Reginald**).
2.  Enabled Caching (**Vala**).
