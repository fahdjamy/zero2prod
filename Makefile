run-debug:
	@echo "Starting the application in debug mode with detailed logging..."
	RUST_LOG=debug cargo run

run-test-noiseless:
	@echo "Running tests while swallowing noisy logs"
	cargo test -- --nocapture
