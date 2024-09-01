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
