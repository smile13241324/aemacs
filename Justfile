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
test:
    cargo test --workspace
check:
    cargo check --workspace
doc:
    cargo doc --workspace --open
sync-ai:
    python3 ai/sync-agents.py
clean:
    cargo clean
fmt:
    cargo fmt --all
demo:
    cargo run -p aemacs-ai --example simple_chat
    cargo run -p aemacs-ai --example simple_chat_async
    cargo run -p aemacs-ai --example ollama_chat
    cargo run -p aemacs-ai --example ollama_chat -- assets/gplv3.png
    cargo run -p aemacs-ai --example rag_demo
    echo("cargo run -p aemacs-ai --example agent_cli")
    echo("cargo run -p aemacs-ai --example index_workspace")
