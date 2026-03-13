# Rust Coding Best Practices

This document compiles the best practices distilled from Canonical and Apollo GraphQL for writing robust, maintainable, and highly performant Rust code. It serves as the standard coding style and principles guide for this repository.

## 1. Naming and Cosmetic Discipline
- **Casing Rules:** Adhere to the standard Rust naming conventions (`snake_case` for variables/functions, `CamelCase` for types/traits, `UPPER_SNAKE_CASE` for constants).
- **Format Strings:** Always inline `format!` args when possible (`format!("Hello, {name}")`).
- **Use Declarations:** Group imports cleanly. Prefer absolute paths and structure `use` statements hierarchically (Std, External Crates, Internal Modules).
- **Match Statements:** Make match statements exhaustive. Avoid wildcard arms (`_`) whenever possible; explicitly handle arms for maintainability.

## 2. Idiomatic Code and Performance Mindset
- **Borrowing over Cloning:** Prefer borrowing (`&T`, `&mut T`) over `.clone()` whenever possible to avoid unnecessary allocations.
- **Passing by Value vs Reference:** Follow official guidelines for when to pass by value (e.g., small `Copy` traits) vs by reference.
- **Iterators:** Prefer `.iter()` and iterator combinators over manual `for` loops for zero-cost abstractions, better readability, and performance.
- **Prevent Early Allocation:** Do not allocate memory (e.g., collecting into a `Vec`) unless explicitly necessary for returning or crossing async boundaries.
- **Stack vs Heap:** Be size-smart. Avoid boxing small or frequently accessed types unless dynamic dispatch or recursive types require it.

## 3. Error and Panic Discipline
- **Result over Panic:** Prefer returning `Result<T, E>` and strictly avoid `panic!`.
- **Avoid unwrap/expect:** Do not use `unwrap()` or `expect()` in production code. Handle errors gracefully using pattern matching or the `?` operator.
- **Error Types:** Use `thiserror` for library/crate level errors and reserve `anyhow` strictly for binaries/applications.
- **Error Bubbling:** Use the `?` operator to bubble errors up naturally.

## 4. Testing Guidelines
- **Tests as Living Documentation:** Write tests that show how your code is intended to be used.
- **Unit vs Integration:** Unit tests should test isolated logic (including error paths), while integration tests verify the module's public interface.
- **Async Tests:** Always mark async tests as `#[tokio::test]`. Avoid `std::thread::sleep` in async contexts; always use `tokio::time::sleep`.
- **Snapshot Testing:** Use snapshot testing (e.g., `cargo insta`) for output validation where appropriate, but review snapshot diffs carefully.

## 5. Architectural Patterns
- **Type State Pattern:** Use the Type State pattern for complex state machines to guarantee correctness at compile time.
- **Dispatch:** Use static dispatch (`impl Trait` or `<T: Trait>`) by default. Use dynamic dispatch (`dyn Trait`) only when necessary for heterogeneous collections or compile-time performance trade-offs.

## 6. Comments vs Documentation
- **Context, Not Clutter:** Comments should explain the *why*, not the *what*. If code is complex, refactor instead of over-commenting.
- **No Living Comments:** Do not write "living documentation" (comments that act as out-of-sync documentation). Use Rustdoc for documentation.
- **TODOs:** `TODO` comments should generally be tracked issues, not ignored forever.

## 7. Safety
- **Unsafe Code:** `unsafe` code is generally forbidden in this repository (enforced via `unsafe_code = "forbid"` in `Cargo.toml`). If absolutely necessary, it must be isolated, heavily documented, and fiercely reviewed.
