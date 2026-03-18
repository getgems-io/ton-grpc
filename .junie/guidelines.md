## Testing

- When writing tests, always follow the AAA (Arrange, Act, Assert) structure.
- Separate the data preparation, action, and result verification sections with empty lines.
- Avoid explicit `// Arrange`, `// Act`, `// Assert` comments to keep the code clean, unless otherwise specified.

## Development Workflow

- Follow the TDD (Test-Driven Development) approach with a strict feedback loop:
  1. Write tests first that cover the expected behavior.
  2. Run the tests and verify they fail (red phase).
  3. Write the minimal implementation code to make the tests pass.
  4. Run the tests again and verify they pass (green phase).
  5. If any tests fail, fix the implementation and re-run until all tests are green.
  6. Refactor if needed, ensuring tests remain green after each change.
