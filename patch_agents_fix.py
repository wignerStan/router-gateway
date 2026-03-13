import re

with open('AGENTS.md', 'r') as f:
    content = f.read()

# Let's cleanly replace everything from "## Coding Conventions" to the end of the file.
match = re.search(r'## Coding Conventions.*', content, flags=re.DOTALL)
if match:
    prefix = content[:match.start()]

    new_conventions = """## Coding Conventions

### Rust Best Practices

This project enforces production-level code style using `rustfmt` and `clippy`. Adhere to the following conventions:

#### General Idioms & Style
- **Format strings**: Always inline `format!` args when possible (`format!("Hello, {name}")` instead of `format!("Hello, {}", name)`).
- **If statements**: Always collapse if statements (`if x { if y { ... } }` becomes `if x && y { ... }`).
- **Closures**: Use method references over closures when possible (`.map(String::from)` instead of `.map(|s| String::from(s))`).
- **Match statements**: Make match statements exhaustive and avoid wildcard arms (`_`) whenever possible. Use explicit arms for maintainability.
- **Range checking**: Use `(start..=end).contains(&val)` instead of manual `>=` and `<=` checks.
- **Borrowing**: Prefer borrowing over cloning. Prevent early allocations.
- **Iterators**: Prefer `.iter()` and iterator combinators over manual `for` loops for zero-cost abstractions and better readability.
- **Passing by Value vs Reference**: Follow official guidelines for when to pass by value (e.g. `Copy` traits) vs by reference.

#### Error Handling
- **Result over Panic**: Prefer returning `Result` and avoid `panic!`.
- **Avoid unwraps**: Do not use `unwrap()` or `expect()` in production code. Handle errors gracefully.
- **Error Types**: Use `thiserror` for library/crate level errors and reserve `anyhow` strictly for binaries/applications.
- **Error Bubbling**: Use the `?` operator to bubble errors up.

#### Testing
- **Deep Equals**: Prefer deep equals comparisons whenever possible. Perform `assert_eq!()` on entire objects rather than individual fields. Use `pretty_assertions::assert_eq` for clearer diffs.
- **Environment**: Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.
- **Async Tests**: Always mark async tests as `#[tokio::test]`.
- **Sleeping**: Avoid `std::thread::sleep` in async contexts; always use `tokio::time::sleep`.
- **Snapshot Testing**: Use snapshot testing (e.g., `cargo insta`) for output validation where appropriate.
- **Test Errors**: Ensure unit tests exercise error conditions and not just the happy path.

#### Comments and Documentation
- **Context, Not Clutter**: Comments should explain the *why*, not the *what*. If code is complex, refactor instead of over-commenting. Don't write living comments when documentation is needed.
- **Living Documentation**: Treat tests as living documentation. Add test examples to your doc comments.
- **TODOs**: `TODO` comments should generally become tracked issues.

### Modules & Architecture

- **Modularity & Size**: Avoid large modules. Prefer adding new modules instead of growing existing ones. Target Rust modules under 500 LoC (excluding tests). If a file exceeds 800 LoC, extract functionality into a new module instead of extending the existing file unless there is a strong documented reason not to.
- **Locality**: When extracting code, move the related tests and module/type docs toward the new implementation so the invariants stay close to the code that owns them.
- **Helper Methods**: Do not create small helper methods that are referenced only once.
- **Type State Pattern**: Consider using the Type State pattern for complex state machines to guarantee correctness at compile time.
- **Dispatch**: Use static dispatch (`impl Trait` or `<T: Trait>`) by default. Use dynamic dispatch (`dyn Trait`) only when necessary for heterogeneous collections or compile-time performance trade-offs.

### Async/Tokio Conventions

- All async operations use Tokio. Always `.await` on registry/selector methods.
- Maintain clear boundaries between async and sync code.

### Code Style Enforcement

All code changes **must** pass formatting and linting rules. Before finalizing changes:

1. Run `cargo fmt`
2. Run `cargo clippy --workspace --all-targets` to fix lint issues (do not simply silence warnings).
3. Run `just qa` to run all project quality gates.
"""

    with open('AGENTS.md', 'w') as f:
        f.write(prefix + new_conventions)

import shutil
shutil.copyfile('AGENTS.md', 'CLAUDE.md')
