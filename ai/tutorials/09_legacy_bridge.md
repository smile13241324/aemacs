# Tutorial 09: The Legacy Bridge (Elisp & Layers)

While we love Rust, we respect the massive ecosystem of Emacs packages. The "Legacy Bridge" allows you to run classic Elisp code inside a sandboxed environment.

**Goal:** Port an old Spacemacs layer to Æmacs, ensuring it doesn't block the Rust render loop.
**Time:** approx. 45 minutes.
**Prerequisite:** `profile_elisp.md`.

---

## 🎭 Your AI Crew for this Job

1.  **Spacky (The Gatekeeper):** Maintains the Elisp sandbox. He ensures your legacy code runs, but warns you about performance.
2.  **Nexus-7 (Dependency Manager):** Manages `layers.el` and package loading order.
3.  **Kallista (Auditor):** Checks if your keybindings conflict with the new Iron Core keys.

---

## Step 1: The Layer Structure

**Scenario:** You want to port the `obsidian` layer from Spacemacs.

**Your Task:**
Use **Nexus-7**.

> **Command:** `/nexus`
> **Prompt:** "I am porting the `obsidian` layer.
> Create the file structure in `legacy/layers/+tools/obsidian`.
> We need `packages.el` and `config.el`."

**Result:**
Nexus-7 sets up the standard structure. *Note: In Æmacs, legacy layers live in `legacy/`.*

---

## Step 2: The Containment (Lazy Loading)

**CRITICAL:** Legacy code runs on the "Slow Thread" (the Elisp interpreter). You **MUST** use lazy loading, or Spacky will reject it.

**Your Task:**
Use **Spacky**.

> **Command:** `/spacky`
> **Prompt:** "Write the `init-obsidian` function in `packages.el`.
> 1. Use `use-package`.
> 2. **Constraint:** You MUST use `:defer t`.
> 3. It should only load when I open a `.md` file in `obsidian-mode`."

**Result:**
Spacky writes the defensive code:

```elisp
(defun obsidian/init-obsidian ()
  (use-package obsidian
    :defer t  ;; MANDATORY for Legacy Bridge
    :mode ("\\.md\\'" . obsidian-mode)))
```

---

## Step 3: Keybindings (The Boundary)

You want to bind `SPC o o` to open Obsidian.

**Your Task:**
Ask **Kallista**.

> **Command:** `/kallista`
> **Prompt:** "I want to bind `obsidian-open` to `SPC o o`.
> Is this safe? Does it conflict with the Rust Core 'Open' command?"

**Result:**
Kallista audits: *"Warning. `SPC o` is User Reserved Space. This is compliant. Proceed. Do NOT try to overwrite `SPC f` (File), as that is handled by the Iron Core."*

---

## Step 4: The Config (Hooks)

**Scenario:** You want to turn on `spell-checking` when Obsidian loads.

**Your Task:**
Switch to **Spacky**.

> **Prompt:** "Add a hook to `obsidian-mode` to enable `flyspell-mode`.
> Use `add-hook`."

**Result:**
Spacky creates the hook in `config.el`.

---

## 🎉 Summary

You have:
1.  Isolated legacy code in `legacy/` (**Nexus**).
2.  Enforced lazy loading to protect performance (**Spacky**).
3.  Respected the keybinding boundary (**Kallista**).
