# Tutorial 09: Fonts & Ligatures (Typography)

A good font makes code readable, a Nerd Font makes it pretty, and ligatures make it magical. Here you will learn how to visually upgrade Æmacs.

**Goal:** Install a Nerd Font (e.g., JetBrains Mono), set it as default, and enable ligatures.
**Time:** approx. 15 minutes.
**Prerequisite:** CLI agents installed via `sync-agents.py`.

---

## 🎭 Your AI Crew for this Job

1.  **Magos Pixelis (UI Designer):** The Typographer. He advises you on font choice (Readability vs. Aesthetics) and optimal size for your resolution.
2.  **Spacky (Coder):** The Mechanic. He configures the `dotspacemacs-default-font` variable and enables the `ligatures` layer.

---

## Step 1: The Choice (The Holy Font)

**Scenario:** You want to switch from the default "Source Code Pro" to "JetBrains Mono" or "Fira Code" to get that modern look.

**Your Task:**
Use **Magos Pixelis**.

> **Command:** `/magos`
> **Prompt:** "I want to change the editor font.
> 1. Which font do you recommend for high readability and 'tech aesthetics'?
> 2. What is the optimal size for a 1440p monitor according to the Grid?"

**Result:**
Magos will rave: *"The Omnissiah favors **JetBrains Mono**. It has distinct glyphs for `1`, `l`, and `I`. Set the size to `14.0` for perfect pixel-alignment on high-res displays."*

---

## Step 2: The Configuration (The `.spacemacs`)

The font is defined centrally in `init.el` (or `.spacemacs`).

**Your Task:**
Switch to **Spacky**.

> **Command:** `/spacky`
> **Prompt:** "Change the default font to 'JetBrains Mono', size 14, weight 'normal'.
> Where do I put this in `.spacemacs`?"

**Result:**
Spacky points you to the `dotspacemacs/init` function and provides the code:

```elisp
   dotspacemacs-default-font '("JetBrains Mono"
                               :size 14
                               :weight normal
                               :width normal)
```

---

## Step 3: The Ligatures (The Magic)

We want `->` to appear as a single arrow glyph. Æmacs has a dedicated layer for this.

**Your Task:**
Stay with **Spacky**.

> **Prompt:** "I want to enable ligatures for this font.
> 1. How do I enable the `ligatures` layer in `dotspacemacs-configuration-layers`?
> 2. How do I configure it to support 'JetBrains Mono'?"

**Result:**
Spacky provides the layer configuration code:

```elisp
(ligatures :variables unicode-fonts-enable-ligatures t)
```

---

## 🎉 Summary

You have:
1.  Selected an ergonomic font (**Magos**).
2.  Set the base configuration (**Spacky**).
3.  Enabled modern ligatures (**Spacky**).
