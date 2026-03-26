# AGENTS.md

## Build System

This project uses [moonrepo](https://moonrepo.dev/) as the build/task orchestration system. All task execution (build, test, lint, etc.) MUST go through moon.

Workspace config: `.moon/workspace.yml`
Projects: root (`moon.yml`) + each crate under `crates/*/moon.yml`.

## Checking & Building

All checks and builds MUST go through moon. **Never use `cargo check` or `cargo build` directly.**

```bash
# Check a specific project
moonx <project-id>:check

# Check all projects
moonx :check
```

Examples:
```bash
moonx tonlibjson-client:check
moonx ton-liteserver-client:check
moonx tl-parser:check
```

## Testing

### Methodology

Follow **TDD** (Test-Driven Development):
1. Write a failing test first.
2. Write the minimum code to make it pass.
3. Refactor while keeping tests green.

### Test Structure

All tests MUST follow the **AAA** (Arrange-Act-Assert) pattern:

Unit tests MUST be placed in a `mod tests` module:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn should_do_something() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected);
    }
}
```

### Integration Tests

Integration tests and tests that depend on Docker MUST be placed in a `mod integration` module.

```rust
#[cfg(test)]
mod integration {
    #[tokio::test]
    async fn should_query_blockchain() {
        // tests requiring Docker or external services go here
    }
}
```

### Running Tests

Run tests via moon:

```bash
moonx <project-id>:test
```

Examples:
```bash
moonx tonlibjson-client:test
moonx ton-liteserver-client:test
moonx tl-parser:test
```
