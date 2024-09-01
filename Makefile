DOCKER_BUILD_TAG := zero2prod

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

docker-release-build:
	# “Using . we are telling Docker to use the current directory as the build context for this image; COPY .
	# app will therefore copy all files from the current directory (including our source code!)
	# into the app directory of our Docker image.
	# Using . as build context implies, for example,
	# that Docker will not allow COPY to see files from the parent directory or
	# from arbitrary paths on your machine into the image.
	@echo "Building a docker release image for the app"
	docker build --tag $(DOCKER_BUILD_TAG) --file Dockerfile .

docker-run-release-build:
	docker run -p 8001:8001 $(DOCKER_BUILD_TAG)

prepare-sqlx:
	# prepare performs the same work that is usually done when cargo build is invoked but it saves the outcome of
	# those queries into a directory (.sqlx) which can later be detected by sqlx itself and used to skip the queries
	# altogether and perform an offline build.
	# this will force sqlx to look at the saved metadata instead of querying a live DB
	cargo sqlx prepare --workspace

update-digital-ocean:
	doctl apps update YOUR-APP-ID --spec=spec.yaml
