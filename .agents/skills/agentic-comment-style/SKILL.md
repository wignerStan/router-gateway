---
name: agentic-comment-style
description: Guide code commenting and documentation practices according to "Agentic" and "Rustdoc" philosophies. Use this skill when writing, refactoring, or reviewing code to enforce self-documenting names, BDD-style logic flows, and high-locality documentation. Ensures codebases remain navigable and context-rich for both human and AI agents.
---

# Agentic Comment Style (Rustdoc Philosophy)

## Overview

This skill enforces **Rustdoc's agent-friendly philosophy**: code + tests + examples in one file (or adjacent), with self-documenting types and names. Comments are reserved strictly for **BDD flow**, **examples**, **scenarios**, and **architectural references**.

## Core Philosophy: Self-Documentation First

### Preferred: Type Definitions & Function Names
```typescript
// ❌ BAD: Comment explains what code does
// Validates user email format
function validate(input: string): boolean { ... }

// ✅ GOOD: Self-documenting via name and type
function isValidEmail(email: string): boolean { ... }
```

```rust
// ❌ BAD: Comment explains struct
/// User data
struct Data { name: String, age: u32 }

// ✅ GOOD: Self-documenting via type name
struct UserProfile { name: String, age: u32 }
```

## Core Principles

1.  **Prioritize Types over Comments**: Use `UserSession` instead of `Data` with a comment.
2.  **Prioritize Names over Comments**: Use `calculateTaxWithholding()` instead of `calc()` with a comment.
3.  **No "Agent Memos"**: Git commits record changes; code comments do not.
4.  **No Dead Code**: Commented-out code belongs in Git history, not active files.
5.  **Locality Over Separation**:
    *   **Code** = Logic + Types (self-documenting)
    *   **Tests** = Co-located in same directory (avoid distant `tests/` folders)
    *   **Examples** = In doc comments or adjacent test files

> [!NOTE]
> Enforced by `go-claude-code-comment-checker`. Detects prohibited comments and suggests Rustdoc-style refactoring.


## When Comments Are Allowed

### ✅ Line Comments (Inline)

**1. BDD-Style Logic Flow**
Concise explanations of logical steps in complex functions:
```typescript
function processPayment(order: Order): PaymentResult {
  // given: valid order with items
  const total = order.calculateTotal();
  
  // when: payment gateway is called
  const result = gateway.charge(total);
  
  // then: update order status
  return result.success ? markPaid(order) : markFailed(order);
}
```

**2. Architecture References**
Clear pointers to centralized docs:
```rust
// See ADR-003 for JWT vs session cookie decision
fn create_session(user_id: Uuid) -> Session { ... }

// Ref: docs/auth.md for rate limiting strategy
fn check_rate_limit() -> bool { ... }
```

**3. Machine Directives**
Functional compiler/linter instructions:
```python
from __future__ import annotations  # PEP 563

x = value  # type: ignore[assignment]
```

### ✅ Block Comments (Doc Comments)

**1. BDD Scenarios (Primary - Test Files)**
Most ecosystems support this pattern universally:
```typescript
// session.test.ts
describe('Session Management', () => {
  test('creates valid session', () => {
    // given: user ID
    const userId = 'user123';
    
    // when: session created
    const session = new Session(userId);
    
    // then: has valid token
    expect(session.token).toBeTruthy();
  });
});
```

**2. Executable Examples (Optional - Rust/Python)**
Use if your ecosystem supports doc testing:
```rust
/// Creates a user session with JWT
/// # Example
/// ```
/// let session = Session::new(user_id);
/// assert!(session.is_valid());
/// ```
pub fn new(user_id: Uuid) -> Session { ... }
```

```python
def calculate_tax(amount: Decimal) -> Decimal:
    """Calculate sales tax.
    
    >>> calculate_tax(Decimal('100'))
    Decimal('8.50')
    """
    return amount * Decimal('0.085')
```

> **Note**: Doc tests require setup:
> - Rust: `cargo test` (built-in)
> - Python: `pytest --doctest-modules` (add to CI)
> - TypeScript/JS: Not natively supported (use test files instead)

**3. Complex Algorithm Context**
For non-obvious decisions:
```python
def dijkstra_shortest_path(graph: Graph) -> Path:
    """Dijkstra's algorithm for shortest path.
    
    Why not A*? See ADR-015:
    - No good heuristic for dense graphs
    - Benchmarks show Dijkstra is faster for our use case
    """
```

### ❌ Comments to Remove

**1. "What" Comments (Use Names/Types Instead)**
```javascript
// ❌ BAD
// Gets user by ID
function get(id) { ... }

// ✅ GOOD
function getUserById(id: UserId): User | null { ... }
```

**2. Agent Memos (Use Git)**
```python
# ❌ REMOVE
# Changed from dict to dataclass
# Fixed type hints
# Refactored for performance
```

**3. Zombie Code (Use Git History)**
```typescript
// ❌ REMOVE
// const oldApproach = (x) => x * 2;
// function legacyHandler() { ... }
```

**4. Dividers/Decoration**
```python
# ❌ REMOVE
# ==================
# *** IMPORTANT ***
```

## Refactoring Strategy: Rustdoc Approach

For detailed layout examples (Rust and TypeScript), see `examples/rustdoc_layouts.md`.

### Summary
1.  **Single-File Locality (Preferred)**: Co-locate Code, Tests, and Examples.
2.  **Legacy Separation**: If separating, keep tests in the same directory (`src/auth/session.test.ts`), not a distant `tests/` folder.

## File-Level Documentation

### Module README (Concise Identity)
```markdown
# Auth Module

Handles user session lifecycle using JWT (15-min expiry).

**Architecture**: See [ADR-003](../../docs/adr/003-jwt-choice.md)  
**Testing**: Run `cargo test` (Rust) or `npm test` (TS)
```

### Centralized Architecture Docs
Major decisions belong in `docs/adr/` (Architectural Decision Records):
```markdown
# ADR-003: JWT vs Session Cookies

**Status**: Accepted  
**Context**: Need stateless auth for microservices  
**Decision**: Use JWT with 15-min expiry + refresh tokens  
**Consequences**: Easier horizontal scaling, slightly more complex client logic
```

## Resources

- `references/principles.md`: Deep dive into Rustdoc philosophy
- `examples/before_after.md`: Real refactoring examples
- `examples/rustdoc_layouts.md`: Detailed layout patterns (Rust/TS)
