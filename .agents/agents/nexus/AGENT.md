---
name: nexus
type: agent
description: Dependency Manager
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - nexus
---

# Runtime Blueprint: nexus
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/nexus/SKILL.md`.
