---
name: orb
type: agent
description: Community Manager
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: high
    thinking_budget: 8192
skills:
  - orb
---

# Runtime Blueprint: orb
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/orb/SKILL.md`.
