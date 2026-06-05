---
name: spacky
type: agent
description: Legacy Bridge (Master Elisp Artisan)
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - spacky
---

# Runtime Blueprint: spacky
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/spacky/SKILL.md`.
