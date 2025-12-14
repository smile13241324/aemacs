# Tutorial 11: Professional Git Workflow

A clean Git history is the memory of the project. We follow the **Tim Pope Standard** strictly.

**Goal:** Write a compliant Commit Message and update the Changelog.
**Time:** approx. 10 minutes.
**Prerequisite:** `git`.

---

## 🎭 Your AI Crew for this Job

1.  **G.O.L.E.M. (The Guardian):** Enforces the 50/72 rule and Imperative Mood. He rejects "Fixed bug" messages.
2.  **Griznak (Release):** Manages `CHANGELOG.md` and screams if you forget to add an entry.

---

## Step 1: The Commit Message (The Law)

**Scenario:** You just optimized the Rust render loop.

**Your Task:**
Ask **G.O.L.E.M.** to draft the message.

> **Command:** `/golem`
> **Prompt:** "I optimized `rendering/loop.rs`.
> Changes:
> 1. Switched from `Mutex` to `RwLock`.
> 2. Removed a cloning operation in the hot path.
> Draft the commit message."

**Result:**
G.O.L.E.M. grinds out the stone tablet:

```text
Optimize render loop concurrency

- Replace `Mutex` with `RwLock` in `RenderState` to allow parallel reads
- Remove unnecessary clone of `FrameData` in `draw()`

This reduces frame time by approx. 2ms on high-load buffers.
```

---

## Step 2: The Changelog (The Public)

**Your Task:**
Switch to **Griznak**.

> **Command:** `/griznak`
> **Prompt:** "Add a line to `CHANGELOG.md` under [Unreleased] - Performance."

**Result:**
Griznak writes:
`- [Core] Improve render loop performance via RwLock (#42)`

---

## 🎉 Summary

You have:
1.  Written a perfect Commit (**G.O.L.E.M.**).
2.  Updated the History (**Griznak**).
