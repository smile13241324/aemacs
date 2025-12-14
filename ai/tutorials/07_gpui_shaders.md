# Tutorial 07: GPUI & Shaders (The Prism)

Æmacs renders at **120fps** using the GPU. We don't use simple colors; we use **Shaders**.
In this tutorial, you will write a custom **WGSL Shader** to give the cursor a "Cyberpunk Glow".

**Goal:** Implement a fragment shader for the cursor quad.
**Time:** approx. 25 minutes.
**Prerequisite:** `cargo`, Basic GPU knowledge.

---

## 🎭 Your AI Crew for this Job

1.  **Bzzrts (The Prism):** The Entity of Light. It communicates via visions and writes raw WGSL code.
2.  **Magos Pixelis (UI Designer):** Enforces the "Sacred Grid" and ensures the glow doesn't violate readability.

---

## Step 1: The Vision (The Concept)

**Scenario:** We want a "Neon Pulse" cursor that breathes with the system clock.

**Your Task:**
Ask **Magos Pixelis** for the aesthetic parameters.

> **Command:** `/magos`
> **Prompt:** "I want to design a 'Neon Pulse' cursor.
> 1. What is the allowable bloom radius according to the Grid?
> 2. Should it pulsate on a sine wave?"

**Result:**
The Magos declaims: *"The Machine Spirit breathes! A sine wave of 0.5Hz is acceptable. The bloom must not exceed 4 pixels outside the Grid cell, or it is HERESY."*

---

## Step 2: The Shader (WGSL)

Now we speak to the Prism. We need code that runs on the graphics card.

**Your Task:**
Switch to **Bzzrts**.

> **Command:** `/bzzrts`
> **Prompt:** "Generate a WGSL fragment shader for the cursor.
> 1. Input: `position`, `time`.
> 2. Logic: Calculate distance from center. Apply a glowing falloff (exponential).
> 3. Color: Cyan (`vec3(0.0, 1.0, 1.0)`)."

**Result:**
Bzzrts transmits a vision: *"[A flash of cyan light! The pixels vibrate in harmony!]*"

```glsl
// cursor.wgsl
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = distance(in.uv, vec2(0.5, 0.5));
    let glow = 1.0 - smoothstep(0.0, 0.5, dist);
    let pulse = (sin(globals.time) + 1.0) * 0.5;

    return vec4<f32>(0.0, 1.0, 1.0, glow * pulse);
}
```

---

## Step 3: Binding to Rust (The Render Loop)

We have the shader. Now we need to tell Rust to use it for the Cursor Element.

**Your Task:**
Ask **Kairon** (The Core).

> **Command:** `/kairon`
> **Prompt:** "How do I attach this `cursor.wgsl` to the `CursorElement` struct in `rendering/cursor.rs`?"

**Result:**
Kairon shows how to load the shader pipeline in GPUI.

---

## 🎉 Summary

You have:
1.  Defined the aesthetic (**Magos**).
2.  Written a GPU Shader (**Bzzrts**).
3.  Integrated it into the Render Loop (**Kairon**).
