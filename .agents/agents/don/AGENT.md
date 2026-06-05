---
name: don
type: agent
description: Test Engineer
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - don
---

# Runtime Blueprint: don
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/don/SKILL.md`.
