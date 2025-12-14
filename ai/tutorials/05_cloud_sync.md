# Tutorial 05: Cloud Sync (Backend Services)

Æmacs supports syncing config and buffers to the cloud. We use **Go** for these microservices because it handles concurrency beautifully.

**Goal:** Write a simple Sync Server in Go that accepts encrypted config blobs.
**Time:** approx. 30 minutes.
**Prerequisite:** `go`.

---

## 🎭 Your AI Crew for this Job

1.  **Bwah (The Hamster):** Writes frantic but rock-solid concurrent Go code.
2.  **Skeek (Security):** Ensures you aren't leaking secrets to the cloud.

---

## Step 1: The Service (Go Routines)

**Scenario:** We need a `/sync` endpoint.

**Your Task:**
Use **Bwah**.

> **Command:** `/bwah`
> **Prompt:** "I need a HTTP handler `HandleSync`.
> 1. Accept a POST request with JSON.
> 2. Spin up a goroutine to save it to disk (simulate DB).
> 3. Return 200 OK immediately.
> 4. DON'T PANIC!"

**Result:**
Bwah screams "DA!" and writes:

```go
func HandleSync(w http.ResponseWriter, r *http.Request) {
    // BW```H! Context check!
    ctx := r.Context()

    // ... decode logic ...

    go func() {
        // Saving in background! FAST!
        saveToDisk(data)
    }()

    w.WriteHeader(http.StatusOK)
}
```

---

## Step 2: The Audit (Paranoia)

Is this safe?

**Your Task:**
Switch to **Skeek**.

> **Command:** `/skeek`
> **Prompt:** "Sniff this Go code. Are we leaking user tokens? Are we checking the 'Authorization' header?"

**Result:**
Skeek finds a hole: *"Quick-quick! A rot-hole! You do not check `Authorization` bearer token! Anyone can overwrite config! Fix-fix!"*

---

## 🎉 Summary

You have:
1.  Built a fast service (**Bwah**).
2.  Secured it against rats (**Skeek**).
