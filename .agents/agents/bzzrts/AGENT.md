---
name: bzzrts
type: agent
description: GPU Visionary (UI & Rendering)
model: gemini-3.5-flash
runtime:
  async: true
  sandbox: nsjail
  thinking_config:
    mode: off
skills:
  - bzzrts
---

# Runtime Blueprint: bzzrts
This stateful background layer instantiates the identity mesh for the active task.
It dynamically binds and inherits the logic from the skill: `.agents/skills/bzzrts/SKILL.md`.
