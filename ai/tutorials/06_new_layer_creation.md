# Tutorial 06: Creating a New Layer (Architecture Level)

You want to integrate a group of packages (e.g., for a new language or tool) cleanly into Æmacs. Not just a single package, but a whole ecosystem.

**Goal:** Create a local layer that bundles packages, configuration, and keybindings.
**Time:** approx. 30–45 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Bob (Architect):** Drafts the "Blueprint". What belongs in the layer? What should it be named? Where does it reside (`+lang`, `+tools`)?
2.  **Nexus-7 (Structure):** Defines the exact folder structure and `layers.el` dependencies.
3.  **Spacky (Coder):** Writes the `packages.el` (Init functions) and Elisp code.
4.  **Kallista (Auditor):** Checks keybinding compliance.

---

## Step 1: The Blueprint (What are we building?)

**Scenario:** We want to build a layer for **Obsidian** (Note-taking app integration).

**Your Task:**
Use **Bob**.

> **Command:** `/bob`
> **Prompt:** "I want to create a new layer for Obsidian integration.
> 1. Should this go into `+tools` or `+writing`?
> 2. What is the standard file structure I need?
> 3. Which packages should be core (owned) vs. optional?"

**Result:**
Bob will say: *"A glorious addition! Place it in `layers/+tools/obsidian`. You need `layers.el`, `packages.el`, `funcs.el`, `config.el`, and `keybindings.el`. The core package is `obsidian.el`."*

---

## Step 2: The Structure (The Foundation)

Now we create the files.

**Your Task:**
Switch to **Nexus-7**.

> **Command:** `/nexus`
> **Prompt:** "Generate the folder structure command (mkdir/touch) for the `obsidian` layer in `private/`.
> Also, write the content for `layers.el` (declare dependencies if needed)."

**Result:**
Nexus-7 provides the shell commands and the `layers.el` declaration.

```bash
mkdir -p layers/+tools/obsidian
touch layers/+tools/obsidian/{layers.el,packages.el,funcs.el,config.el,keybindings.el}
```

---

## Step 3: The Packages (The Init Functions)

The heart of the layer. Here we define how packages load.

**Your Task:**
Switch to **Spacky**.

> **Command:** `/spacky`
> **Prompt:** "Write the `packages.el` for the obsidian layer.
> 1. Define the package list (`defconst`).
> 2. Write `init-obsidian` using `use-package` and `:defer t`.
> 3. Ensure it loads only on markdown files or via command."

**Result:**
Spacky writes the code using **Deterministic Reasoning** to ensure Ownership Rules (`init-` vs `post-init-`) are strictly followed.

---

## Step 4: The Keys (The Integration)

A layer needs Leader Keys.

**Your Task:**
Ask **Spacky** (or consult **Kallista** regarding placement).

> **Command:** `/spacky`
> **Prompt:** "Now implementing `keybindings.el`.
> Bind the main Obsidian menu to a logical key under `SPC a` (apps) or `SPC m` (major mode). Use `spacemacs/set-leader-keys`."

**Result:**
Spacky delivers the code.

```elisp
(spacemacs/set-leader-keys
  "aoo" 'obsidian-open
  "aoc" 'obsidian-capture)
```

---

## 🎉 Summary

You have:
1.  Planned the architecture (**Bob**).
2.  Laid out the structure cleanly (**Nexus**).
3.  Implemented Lazy Loading (**Spacky**).
4.  Defined compliant keys (**Spacky/Kallista**).
