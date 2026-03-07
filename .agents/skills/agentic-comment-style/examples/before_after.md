# Examples: Before vs After

## 1. The "What" Comment

**❌ Before**
```python
# check if user has admin role
if user.role == 'admin':
    # grant access
    grant_access()
```

**✅ After (Refactor)**
```python
if user.is_admin():
    grant_access()
```

## 2. The Agent Memo

**❌ Before**
```typescript
// Refactored to use async/await pattern
// Changed error handling to try/catch
async function fetchData() {
    try {
        // ...
    } catch (e) {
        // ...
    }
}
```

**✅ After**
```typescript
async function fetchData() {
    try {
        // ...
    } catch (e) {
        // ...
    }
}
// Note: The changelog belongs in the Git commit message:
// "Refactor: Convert fetchData to async/await with try/catch error handling"
```

## 3. The Complex Why

**❌ Before**
```javascript
// loop 5 times
for (let i = 0; i < 5; i++) {
   // ...
}
```

**✅ After (Named Constant)**
```javascript
const RETRY_LIMIT = 5;
for (let i = 0; i < RETRY_LIMIT; i++) {
   // ...
}
```

## 4. The Module Header

**❌ Before**
```python
# src/payment/processor.py

# PAYMENT PROCESSOR MODULE
# This module handles interactions with Stripe.
# It implements a retry mechanism for failed webhooks.
# CHANGE LOG:
# - Added retry logic
# - Updated API version
class PaymentProcessor: ...
```

**✅ After**

File: `src/payment/README.md` (Scope & Identity)
```markdown
# Payment Module

Handles Stripe interactions for subscription billing.

## Reference
- **Retry Logic**: See `docs/adr/003-webhook-resilience.md` for why we use custom backoff.
```

File: `src/payment/processor.py` (Clean Logic)
```python
class PaymentProcessor: ...
```

File: `src/payment/processor_test.py` (Usage)
```python
def test_webhook_retry(): ...
```

## 5. Valid Comments to Keep

**✅ Machine Directives**
```python
import unused_module  # noqa: F401
```

**✅ BDD Markers**
```python
def test_login():
    # given
    user = create_user()
    
    # when
    response = client.post("/login", json=user)
    
    # then
    assert response.status_code == 200
```

**✅ Critical Business Context**
```javascript
// We must delay 30 days before hard deletion due to GDPR requirements
// See: https://gdpr-info.eu/art-17-gdpr/
const DELETION_DELAY_DAYS = 30;
```
