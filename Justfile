# Justfile

default: build
build:
		cargo build
run:
		cargo run
run_info:
		RUST_LOG=info cargo run
run_debug:
		RUST_LOG=debug cargo run
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
