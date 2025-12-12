# Tutorial 05: Writing Layer Documentation

A layer without a README is like an IKEA shelf without instructions: You might get it built, but there will be screws left over.
This tutorial shows you how to create professional documentation effortlessly.

**Goal:** Create a `README.org` (or `.md`) for your layer, including perfect keybinding tables.
**Time:** approx. 10 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Scribe Veridian (Documentation):** The Librarian. He writes the introduction, installation instructions, and feature lists.
2.  **G.O.L.E.M. (Doc Reviewer):** The Guardian. He checks if you adhered to the standard format (Æmacs has strict documentation rules regarding headers and licenses).

---

## Step 1: The Draft (Content)

**Scenario:** You finished the `bisexual-pride` layer. Now the world needs to know what it does.

**Your Task:**
Use **Scribe Veridian**.

> **Command:** `/scribe`
> **Prompt:** "I need a README for my new layer `bisexual-pride`.
> Features:
> 1. Adds a bi-flag banner to the home buffer.
> 2. Sets the theme to purple/pink/blue.
> 3. Adds a keybinding `SPC o b` to toggle the mode.
> Please draft the 'Description' and 'Install' sections."

**Result:**
Scribe Veridian (likely in "Knight" mode) will provide a flowery yet precise text:
*"This layer brings visibility and color to your editor..."*

---

## Step 2: The Keybinding Table (The Grunt Work)

Æmacs documentation requires specific table formatting. Doing this by hand is painful.

**Your Task:**
Stay with **Scribe Veridian**.

> **Prompt:** "Create the Keybindings table for this layer in Org-mode format.
> - `SPC o b` : Toggle Pride Mode
> - `SPC o B` : Cycle Color Palette"

**Result:**
He generates the perfectly formatted table:

```org
| Key Binding | Description         |
|-------------+---------------------|
| ~SPC o b~   | Toggle Pride Mode   |
| ~SPC o B~   | Cycle Color Palette |
```

---

## Step 3: The Quality Check (The Golem)

Before committing, the Guardian must inspect the work. Æmacs requires specific headers.

**Your Task:**
Switch to **G.O.L.E.M.**.

> **Command:** `/golem`
> **Prompt:** "Review this README draft. Does it comply with the 'Document Statutes'? Are the headers correct?"

**Result:**
G.O.L.E.M. will complain (or praise):
*"Grind... The 'Install' section... is missing the layer variable example. Add `(configuration-layer/declare-layer 'bisexual-pride)` example code. Compliance: 80%."*

---

## 🎉 Summary

You have:
1.  Created a professional text (**Veridian**).
2.  Automated table formatting (**Veridian**).
3.  Adhered to strict rules (**G.O.L.E.M.**).
