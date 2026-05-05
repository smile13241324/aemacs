# Justfile

default: build
build:
    cargo build --workspace --all-features
release:
    cargo build --workspace --all-features -r
run:
    RUST_BACKTRACE=1 RUST_LOG=DEBUG cargo run
run_release:
    cargo run
run_wsl:
    WAYLAND_DISPLAY="" XDG_SESSION_TYPE=x11 WGPU_BACKEND=vulkan cargo run
test:
    cargo test --workspace --all-features
miri:
    cargo +nightly miri test --workspace --all-features
check:
    cargo check --workspace --all-targets --all-features
check_hard:
    cargo clippy --workspace --all-targets --all-features
doc:
    cargo doc --workspace --open
sync-ai:
    python3 ai/sync-agents.py
clean:
    cargo clean --workspace
fmt:
    cargo +nightly fmt --all
demo:
    cargo run -p aemacs-ai --example simple_chat
    cargo run -p aemacs-ai --example simple_chat_async
    cargo run -p aemacs-ai --example ollama_chat
    cargo run -p aemacs-ai --example ollama_chat -- assets/gplv3.png
    cargo run -p aemacs-ai --example rag_demo
