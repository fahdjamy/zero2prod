#### Integration Tests

Each file in the tests directory is compiled as its own separate crate, which is useful for creating
separate scopes to more closely imitate the way end users will be using your crate.
However, this means files in the tests directory don’t share the same behavior as files in src do.

If a project is a binary crate that only contains a src/main.rs file and does not have a src/lib.rs file,
we can’t create integration tests in the tests directory and bring functions defined in the
src/main.rs file into scope with a use statement.
Only library crates expose functions that other crates can use; binary crates are meant to be run on
their own.

[Read-more...](https://doc.rust-lang.org/book/ch11-03-test-organization.html)

The tests folder is special - cargo knows to look into it searching for integration tests.
Each file within the tests folder gets compiled as its own crate.

We can check this out by running cargo build --tests and then looking under target/debug/deps:

# Build test code, without running tests

`cargo build --tests`

# Find all files with a name starting with `health_check`

ls target/debug/deps | grep health_check

health_check-fc23645bf877da35
health_check-fc23645bf877da35.d

The trailing hashes will likely be different everytime, but there should be two entries starting with
_test_file_name_-*.
tests will be executed for example if we decide to run a specific built filename

`./target/debug/deps/<file_name>-fc23645bf877da35`
