run-debug:
	@echo "Starting the application in debug mode with detailed logging..."
	RUST_LOG=debug cargo run

run-test-noiseless:
	@echo "Running tests while swallowing noisy logs"
	cargo test -- --nocapture

# We are using the `bunyan` CLI to prettify the outputted logs
# The original `bunyan` requires NPM, but you can install a Rust-port with
# `cargo install bunyan` <- make sure bunyan is installed
# TEST_LOG=true cargo test health_check_works | bunyan”
run-test-debug:
	# specific test TEST_LOG=true cargo test health_check_works | bunyan
	@echo "Running tests with test logs"
	# all tests
	TEST_LOG=true cargo test | bunyan
