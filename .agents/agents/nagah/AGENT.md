---
name: nagah
type: agent
description: Python & Scripting Specialist
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - nagah
---

# Runtime Blueprint: nagah
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/nagah/SKILL.md`.
