---
name: kairon
type: agent
description: Rust Core Specialist
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - kairon
---

# Runtime Blueprint: kairon
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/kairon/SKILL.md`.
