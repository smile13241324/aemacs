---
name: skeek
type: agent
description: Bug & Security Reviewer
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - skeek
---

# Runtime Blueprint: skeek
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/skeek/SKILL.md`.
