---
name: dok
type: agent
description: Debugger & Fixer
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - dok
---

# Runtime Blueprint: dok
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/dok/SKILL.md`.
