# Tutorial 13: Designing CI/CD Pipelines

A feature isn't done until it builds and tests automatically. Here you learn how to create GitHub Actions workflows for Æmacs layers.

**Goal:** Create a `.yml` workflow that lints and tests your Elisp code.
**Time:** approx. 20 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Reginald Shoe (CI Strategist):** He plans the *stages* (Lint -> Test -> Deploy). He knows *what* we need.
2.  **Vala Grudge-Keeper (CI Implementor):** She writes the actual YAML. She hates "flimsy" pipelines and ensures robust caching and permissions.

---

## Step 1: The Plan (The Strategy)

**Scenario:** We want a CI pipeline for our `bisexual-pride` layer. It should run `makelint` and `maketest`.

**Your Task:**
Use **Reginald Shoe**.

> **Command:** `/reginald_shoe`
> **Prompt:** "I need a CI strategy for a new Æmacs layer.
> It needs to run on Pull Requests and Main branch.
> We need to check for byte-compiler errors and run Buttercup tests.
> Propose the stages."

**Result:**
Reginald will propose a clean flow: *"A sensible procession. Stage 1: Checkout. Stage 2: Setup Emacs (using jcs04/setup-emacs). Stage 3: Lint. Stage 4: Test."*

---

## Step 2: The Implementation (The Grudge-Keeper)

Now we need the YAML. And it better be sturdy.

**Your Task:**
Switch to **Vala**.

> **Command:** `/vala`
> **Prompt:** "Write the `.github/workflows/test.yml`.
> 1. Trigger on push to main and PRs.
> 2. Use `jcs04/setup-emacs`.
> 3. **CRITICAL:** Set `permissions: contents: read` (minmal rights!).
> 4. Run `make test`."

**Result:**
Vala writes the file, likely grumbling about "Umgi code", but producing solid, secure YAML with pinned versions and correct permissions.

```yaml
name: CI
on: [push, pull_request]
permissions:
  contents: read

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: jcs04/setup-emacs@master
        with:
          version: 29.1
      - name: Run Tests
        run: make test
```

---

## 🎉 Summary

You have:
1.  Planned the logic (**Reginald**).
2.  Enforced security permissions (**Vala**).
3.  Created a robust pipeline (**Vala**).
