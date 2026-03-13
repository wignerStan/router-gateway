import re

with open('AGENTS.md', 'r') as f:
    content = f.read()

# Replace the "Code style guidelines", "Testing instructions", and "Security considerations"
# with a reference to docs/RUST_BEST_PRACTICES.md

pattern = re.compile(r'## Code style guidelines.*?## Extra instructions', re.DOTALL)

replacement = """## Rust Coding Best Practices

This project strictly adheres to production-level Rust coding standards derived from Canonical and Apollo GraphQL best practices.
For the complete list of rules regarding naming, error handling, performance, testing, and safety, please refer to:
[docs/RUST_BEST_PRACTICES.md](docs/RUST_BEST_PRACTICES.md)

Key Highlights:
- **Zero-cost Abstractions**: Prefer iterators and borrowing over cloning.
- **Error Handling**: Use `Result` and `thiserror`/`anyhow`. Never use `unwrap()` or `expect()` in production code.
- **Safety**: `unsafe` code is forbidden.
- **Async**: Always use `tokio` for async operations and `tokio::time::sleep` over `std::thread::sleep`.
- **Formatting**: Code must pass `cargo fmt` and `cargo clippy --workspace` based on strict configurations.

## Extra instructions"""

new_content = pattern.sub(replacement, content)

with open('AGENTS.md', 'w') as f:
    f.write(new_content)
