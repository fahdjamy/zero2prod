#!/usr/bin/env bash

export RUST_LOG="sqlx=error,info"
export TEST_LOG=true
cargo t subscribe_fails_if_there_is_a_db_fatal_err | bunyan
