# Rustdoc Layout Strategy

## Single-File Locality (Preferred)

### Rust Example: Code + Tests + Examples (All-in-One)
```rust
/// User session with JWT authentication
/// # Example
/// ```
/// let session = Session::new(user_id);
/// assert!(session.token().len() > 0);
/// ```
pub struct Session {
    user_id: Uuid,
    token: String,
}

impl Session {
    pub fn new(user_id: Uuid) -> Self {
        // given: valid user ID
        let token = generate_jwt(user_id);

        // when: session created
        Session { user_id, token }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        // given
        let user_id = Uuid::new_v4();

        // when
        let session = Session::new(user_id);

        // then
        assert!(session.token.len() > 0);
    }
}
```

### TypeScript Example: Code + Tests (Same Directory)
```typescript
// src/auth/session.ts
export class Session {
    constructor(
        private userId: string,
        private token: string
    ) {}

    isValid(): boolean {
        // See ADR-007 for 15-minute expiry policy
        return !this.isExpired();
    }
}

// src/auth/session.test.ts (in same directory, not tests/auth/)
import { Session } from './session';

describe('Session', () => {
    test('validates active session', () => {
        // given
        const session = new Session('user123', 'token');

        // when & then
        expect(session.isValid()).toBe(true);
    });
});
```

## Legacy Separation Pattern

If you must separate (e.g., existing codebase), maintain **close proximity**:
```text
src/auth/
  session.ts           # Code with inline BDD comments
  session.test.ts      # Tests in same directory (not tests/auth/)
  README.md            # Module context + ADR links
```
