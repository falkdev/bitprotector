# Integration Tests — Overview

This folder covers the Rust integration test suite in `tests/integration/`. These tests verify end-to-end behavior of the compiled binary and the REST API against a real SQLite database — no mocks of the storage layer.

---

## Table of Contents

- [What Integration Tests Cover](#what-integration-tests-cover)
- [Two Test Harnesses](#two-test-harnesses)
- [Isolation Strategy](#isolation-strategy)
- [Shared Infrastructure — the `common` Module](#shared-infrastructure--the-common-module)
- [Documents in This Folder](#documents-in-this-folder)

---

## What Integration Tests Cover

Integration tests answer a different question than unit tests. Where unit tests verify that an individual function returns the right value given controlled inputs, integration tests verify that:

- The CLI command parses its arguments, calls the right internal logic, and produces the expected output text and exit code.
- The REST API route validates input, writes to the database, and returns the correct JSON shape and HTTP status code.
- Database state is correctly read back after a write — catching bugs where a function succeeds internally but persists the wrong data.
- Error conditions produce the right error messages and codes, not silent failures or crashes.

They deliberately do not mock the database layer. Every test that exercises the API or CLI reads and writes a real SQLite file, giving high confidence that the ORM queries, schema, and application logic work together correctly.

---

## Two Test Harnesses

The integration suite uses two different harnesses depending on what is being tested.

### Binary invocation via `assert_cmd`

CLI integration tests (all `cli_*.rs` files except `cli_auth.rs`) invoke the compiled `bitprotector` binary as a child process. This tests the full path: argument parsing, command dispatch, database access, and output formatting. Assertions are made on `stdout`, `stderr`, and the exit code.

This approach is the most realistic for CLI testing because it exercises the exact binary that will be shipped. It also catches bugs that only manifest at the process boundary — for example, an unhandled panic that would cause a non-zero exit code without a useful error message.

### In-process actix-web test client

API integration tests (`cli_auth.rs` and all `api_*.rs` files) start the actix-web application in-process using `actix_web::test` rather than spawning the binary. HTTP requests are constructed and dispatched through the test client, and responses are inspected directly as Rust values.

This approach is faster than spawning a binary for each assertion and makes it straightforward to test many request/response permutations without the overhead of process startup. It also enables direct inspection of response body structure as deserialized types rather than string matching.

---

## Isolation Strategy

**Every test gets its own database.** There is no shared database state between tests.

For CLI tests, each test creates a temporary SQLite file using `tempfile::NamedTempFile` and passes its path to every command via the `--db` flag. The file is automatically deleted when the `NamedTempFile` value is dropped at the end of the test.

For API tests, each test creates an in-memory SQLite connection pool via a shared factory in the `common` module. The in-memory database is never written to disk and is discarded when the pool is dropped.

For tests that require real directory paths (e.g., `drives add` validates that the paths exist on disk), `tempfile::TempDir` creates a real temporary directory. Like `NamedTempFile`, it cleans itself up when dropped.

This isolation means every test is independent and can run in any order, in parallel. There is no need for test setup/teardown hooks that reset shared state, and no flakiness from one test's writes interfering with another test's reads.

---

## Shared Infrastructure — the `common` Module

The `tests/integration/common/` module provides helpers shared across API test files:

- Two macros that initialize a full actix-web `App` with the configured routes, JWT middleware, and an in-memory repository injected as application state. `make_app!(repo)` is used by most API tests and passes a hardcoded `/tmp` path for the `DatabasePath` parameter. `make_app_with_db_path!(repo, path)` is the underlying form and is used directly by `api_database.rs` tests that need a real filesystem path (e.g. for backup write/restore operations). Both keep each test file free of boilerplate.
- A `make_repo()` function that returns a fresh `Repository` backed by an in-memory SQLite pool with the schema already initialized.
- A `bearer()` helper that issues a pre-signed JWT for a test user, so tests that need an authenticated request can get a valid `Authorization: Bearer` header without calling the login endpoint.

---

## Documents in This Folder

| Document | What it covers |
| --- | --- |
| [cli.md](cli.md) | Each CLI integration test file: what commands are exercised and what scenarios are covered |
| [api.md](api.md) | Each REST API integration test file: what endpoints are covered and what behavior is verified |
| [core.md](core.md) | Core mechanics tests, the 100k-row scaling test, checksum strategy tests, and packaging artifact tests |
