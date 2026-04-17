# Edge Case Review — 2026-04-12

Full codebase edge-case audit via 9 parallel path-tracer agents covering all source modules.

## Summary

| Module | Files Scanned | Findings |
|--------|--------------|----------|
| Core (main, lib, config, state, routes, cli) | 6 | 15 |
| Router + Selector + Bandit | 15 | 15 |
| Health + Weight | 5 | 12 |
| Config + Fallback + History + Executor | 6 | 15 |
| SQLite Store | 6 | 24 |
| Registry + Policy | 11 | 20 |
| Providers (OpenAI, Anthropic, Google) | 6 | 24 |
| Tracing | 5 | 10 |
| Utils + Classification | 8 | 10 |
| **Total** | **68** | **~145** |

After deduplication, findings collapse into the recurring themes below.

---

## 1. SECURITY — SSRF & Auth Bypass Vectors

### 1.1 SSRF: Missing IPv4/IPv6 Range Blocks
**Location:** `src/utils/ssrf.rs:53-97`

| Gap | Trigger | Fix |
|-----|---------|-----|
| IPv4 multicast 224.0.0.0/4 not blocked | Multicast address passes validation | Block `224..=239` first octet |
| IPv4 CGNAT 100.64.0.0/10 not blocked | RFC 6598 shared-address space | Block `100.64..100.127` |
| IPv4 Class E 240.0.0.0/4 not blocked | Reserved addresses on misconfigured networks | Block `240..=254` first octet |
| IPv6 deprecated site-local fec0::/10 | Deprecated but still routable on some systems | Block `0xfec0..=0xfeff` segment range |
| DNS rebinding for domain hosts | Domain resolves to private IP after check | Document as accepted risk or add DNS-resolution check |

### 1.2 Auth & Input Validation
| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `routes.rs:157-165` | X-Forwarded-For spoofed with internal IPs | Validate parsed IP; strip private ranges | Attacker shares/bypasses rate limit |
| `routes.rs:82` | Bearer header with whitespace-only token | Filter empty after strip_prefix | Cryptic auth failures |
| `state.rs:155` | Empty string IP to rate limiter | `if ip.is_empty() { return false; }` | Empty-key bucket shared by all |
| `config.rs:256-258` | API key is whitespace-only after env expansion | `trim().is_empty()` check | Cryptic provider auth failures |
| `config.rs:120-121` | daily_quota or rate_limit set to 0 | Reject zero in validation | All requests blocked immediately |

---

## 2. INTEGER OVERFLOW / WRAPPING — Recurring Pattern

**Pattern:** Multiple counters use `+= 1` without `saturating_add`, risking debug panics or release wraps.

| Location | Variable | Type | Fix |
|----------|----------|------|-----|
| `state.rs:163-166` | rate limit count | `u64` | `saturating_add(1)` |
| `state_machine.rs:111` | error_counts value | `i32` | `saturating_add(1)` |
| `state_machine.rs:116` | consecutive_successes | `i32` | `saturating_add(1)` |
| `state_machine.rs:119` | consecutive_failures | `i32` | `saturating_add(1)` |
| `collectors.rs:101-318` | total_requests, success/failure counts, error_counts | `i64`/`i32` | `saturating_add(1)` for all |
| `metrics.rs:84-91` | total/success/failed requests | `u64` | Already uses `saturating_add` — verify all paths |
| `outcome.rs:98` | prompt_tokens + completion_tokens | `u32` | `saturating_add` |
| `types/mod.rs:178` | total_tokens (prompt + completion) | `u32` | `saturating_add` |
| `session.rs:115` | request_count | `u64` | `saturating_add(1)` |
| `exploration.rs:181-185` | success/failure decayed values | `f64` | `max(f64::MIN_POSITIVE)` |

---

## 3. NaN / INFINITY PROPAGATION — Float Safety

**Pattern:** NaN passes `> 0.0` checks (returns false) but propagates through arithmetic, permanently corrupting EWMA, weights, and scores.

| Location | Trigger | Fix |
|----------|---------|-----|
| `calculator.rs:58` | success_rate is NaN passes into scoring | `is_finite()` guard before use |
| `calculator.rs:91` | NaN score propagates into total_weight | `if is_nan() { 0.5 }` fallback |
| `exploration.rs:163` | total_weight sum is NaN/Inf | `is_finite()` guard, fallback to uniform |
| `selector.rs:244` | Weight sum NaN/Inf in SQLite selector | `is_finite()` guard before selection |
| `selector.rs:199-209` | Config weight values are NaN | `if !weight.is_finite() { 0.0 }` |
| `ranking.rs:57-84` | Weight calculator returns NaN for an auth | `weight > 0.0 && weight.is_finite()` guard |
| `fallback/planning.rs:77` | Negative or NaN weight from calculator | `retain(|w| w.is_finite() && w > 0.0)` |
| `config/mod.rs:144` | All weights zero: normalize divides by ~0 | Early return with defaults when total == 0 |
| `collectors.rs:116-123` | NaN/negative latency_ms to record_request | `if latency_ms.is_nan() \|\| latency_ms < 0.0 { return; }` |
| `metrics.rs:74-79` | EWMA receives NaN/Inf latency | `if new_value.is_finite() { new_value } else { prev_ewma }` |
| `exploration.rs:134` | sample_gamma with shape=0.0 produces NaN | Guard: `if shape <= 0.0 { return 0.5; }` |

---

## 4. STATE MACHINE GAPS — Health Transitions

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `state_machine.rs:133-138` | Cooldown extended indefinitely on repeated failures | Cap max cooldown; require recovery window | Credential never recovers |
| `state_machine.rs:162` | consecutive_failures wraps negative | `saturating_add` + `> 0` guard | Unhealthy state hidden |
| `state_machine.rs:167` | consecutive_successes wraps negative | `saturating_add` + `> 0` guard | Recovery never triggers |
| `state_machine.rs:205` | Duration overflow on extreme cooldown_period | `checked_add_signed` | DateTime panic |
| `state_machine.rs:192` | Clock skew: unavailable_until from future | Document or add clock-drift tolerance | Permanent unavailability |
| `state_machine.rs:291` | Identical last_check_time → nondeterministic eviction | Sort by `(time, id)` tuple | Varying eviction across runs |
| `state_machine.rs:81-88` | TOCTOU: op_count check outside write lock | Move fetch_add inside write lock guard | Redundant lock acquisition |

---

## 5. PROVIDER ADAPTERS — Silently Dropped Data

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `openai.rs:43-140` | System prompt (ProviderRequest.system) never read | Insert system message at index 0 | System prompt silently dropped |
| `anthropic.rs:155-156` | ToolChoice::None maps to type "any" | Map to type "none" | Forces tool use when caller wanted off |
| `google.rs:93` | MIME type hardcoded to image/jpeg | Detect from URL extension or data URI prefix | Non-JPEG images sent with wrong type |
| `google.rs:94` | URL placed in inlineData.data (expects base64) | Branch: data: URI → inlineData, URL → fileData | Gemini API call fails |
| `openai.rs:56-74` | image_data (inline) silently dropped | Convert to data: URI in image_url field | Inline images lost |
| `anthropic.rs:73-99` | image_data (inline) silently dropped | Map to base64 source type | Inline images lost |
| `google.rs:87-99` | image_data with wrong part_type dropped | Check image_data before falling through | Inline images lost |
| `google.rs:143-159` | tool_choice ignored when tools set | Add tool_config to Gemini request | Tool strategy silently ignored |
| `google.rs:73-77` | Unknown message role passed verbatim | Map or reject unsupported roles | Gemini rejects request |
| All adapters | u64 → u32 token count cast truncates | `u64::try_into().unwrap_or(u32::MAX)` | Silent wraparound |
| All adapters | Empty function parameters `{}` | Provide valid JSON Schema skeleton | API may reject empty schema |
| `openai.rs:57-61` | text part with `p.text == None` | `unwrap_or(&String::new())` | Null content in JSON |
| `google.rs:88` | Same null text issue | Same fix | Gemini rejects null text |

---

## 6. DATABASE — Silent Data Loss

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `operations.rs:258-263` | Timestamp parse fails → None silently | Propagate `SqliteError::Serialization` | Corrupt data masked as missing |
| `operations.rs:369-371` | unavailable_until parse fails → None | Propagate error | Unhealthy auth appears available |
| `operations.rs:344-345` | error_counts JSON malformed → empty map | Propagate error | Loss of failure tracking |
| `operations.rs:443-444` | last_request_time parse fails → now() | Propagate error | Monitoring shows wrong timestamps |
| `operations.rs:503-509` | Same for load_all_health timestamps | Propagate error | Corrupt cooldowns hidden |
| `operations.rs:528-542` | Negative max_age_seconds deletes all history | `max_age_seconds.max(0)` guard | Data loss |
| `store/mod.rs:76-83` | database_path already has query params | Check `contains('?')` before appending | Double query string |
| `store/mod.rs:110` | cache_size_mb is 0 or negative | `cache_size_mb.max(1)` | SQLite page cache disabled |
| `collectors.rs:137-140` | write_status_history error silently discarded | Log the error | History writes fail invisibly |
| `collectors.rs:174-184` | Partial flush: dirty flag cleared for failed items | Only clear dirty for successful writes | Lost metrics on partial failure |
| `selector.rs:94-173` | DB error → entire pick returns None silently | Propagate `Result<Option<...>>` | Transient error = no candidates |

---

## 7. TRACING — Double-Complete & Loss

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `trace.rs:91-97` | complete() called twice on same span | Guard: `if self.end_time.is_some() { return; }` | Latency overwritten |
| `trace.rs:100-104` | set_error() overwrites non-500 status | Guard: `if self.status_code.is_none() { 500 }` | 429 replaced with 500 |
| `trace.rs:66-88` | Empty provider/model string | Assert non-empty at construction | Empty-key entries pollute metrics |
| `middleware.rs:129` | Latency u128 → u64 truncation | `try_into().ok()` | Wrong latency value |
| `middleware.rs:108-143` | Handler panic loses in-flight trace | Drop guard or catch_unwind | Silent trace loss |
| `metrics.rs:126-132` | Unique provider/model strings grow HashMap unbounded | Cap size or validate against known set | OOM with adversarial input |
| `middleware.rs:84-86` | Empty x-llm-provider/x-llm-model header | Filter empty strings after extraction | Empty-key metrics entries |

---

## 8. REGISTRY & POLICY — Misconfiguration Silently Accepted

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `policy/types.rs:325-326` | Non-numeric TokenCount value defaults to 0 | Log warning + reject invalid | "10k" matches as 0 |
| `policy/types.rs:310-318` | Custom condition "key:" or ":value" | Validate both parts non-empty | Misconfigured condition silent |
| `policy/types.rs:304-306` | hour_of_day outside 0-23 / day_of_week outside 0-6 | Validate range before comparison | Never matches expected condition |
| `policy/matcher.rs:170-173` | Empty string in avoid list matches everything | Filter empty strings | All models blocked |
| `policy/matcher.rs:212-215` | Empty preferred_model string | Filter empty strings | All models get bonus |
| `policy/registry.rs:34-37` | Duplicate policy ID not detected | Check before insert | Doubled policy effect |
| `policy/matcher.rs:263-264` | All policies have priority 0 → weight 1.0 | Document or use avg_weight | Explicit priority=0 ignored |
| `policy/types.rs:345` | "in" operator with empty value matches "" | Guard: `!condition.value.is_empty()` | Unexpected matches |
| `categories.rs:356-391` | Zero-price model misclassified as Fast | Require `> 0.0` in price check | Wrong tier assignment |
| `categories.rs:210-234` | ProviderCategory::parse never returns None | Remove Option wrapper | Dead code in None branches |
| `registry/operations.rs:259-262` | cached_count includes expired entries | Filter by `Utc::now() < expires_at` | Inflated count |
| `registry/operations.rs:372-388` | estimate_costs returns empty on any error | Log partial fetch errors | Silent cost estimate failure |
| `registry/operations.rs:424-426` | Negative refresh_interval → huge u64 | `.max(0) as u64` | Near-infinite refresh delay |
| `info.rs:132-139` | context_window = usize::MAX → float precision loss | Guard extreme values | Wrong max_tokens |

---

## 9. ROUTING LOGIC — Dispatch & Selection

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `dispatch.rs:60-63` | Empty credential_id added to candidates | Guard non-empty | Empty-key pollutes candidate set |
| `dispatch.rs:245-256` | Bandit selects route_id not in candidates | Fallback to select_weighted | Returns None despite valid candidates |
| `exploration.rs:51` | Negative/NaN utility in HashMap | `.clamp(0.0, 1.0)` after unwrap_or | Corrupt Thompson sampling |
| `exploration.rs:103-122` | alpha or beta is zero in sample_beta | `max(f64::MIN_POSITIVE)` | Degenerate beta distribution |
| `candidate.rs:93-101` | estimated_tokens u32 overflow on 32-bit | Cast via u64 | Incorrect Fits/Exceeds |
| `filtering.rs:135` | Duplicate disabled providers O(n²) | Use HashSet | Performance degradation |
| `executor.rs:112-183` | Primary None + fallbacks empty | Explicit "no routes" error | Wrong error message |
| `executor.rs:188-191` | Off-by-one: primary counts as attempt 1 | Adjust retry budget calculation | Extra fallback attempt |
| `executor.rs:209-212` | Fallback gets empty model_id | Propagate primary model_id | Empty model in ExecutionResult |
| `executor.rs:316-321` | 3xx redirect treated as retryable error | Exclude 300-399 from retryable | Unnecessary fallbacks |
| `executor.rs:324-328` | status_code 0 recorded in health | Classify as NetworkError explicitly | Never matches health rules |
| `history.rs:135-139` | completed_at before started_at | Guard: swap if inverted | Negative duration |
| `history.rs:393-395` | Percentile index collapse for small n | Use proper quantile function | Underreported tail latency |
| `session.rs:129-131` | cleanup_expired may leave count > max | Run eviction after insert | Session count exceeds limit |
| `time_quota.rs:113-131` | Zero-length time slot (start == end) | Reject or treat as full-day | Slot never matches |

---

## 10. CONFIGURATION DRIFT — CLI vs Gateway

| Location | Trigger | Fix | Consequence |
|----------|---------|-----|-------------|
| `cli.rs:440-445` | CLI valid strategies differ from gateway | Share constant between both | Config passes CLI, fails gateway |
| `cli.rs:414-436` | Duplicate credential IDs not detected | Add HashSet dedup check | CLI says valid, gateway rejects |
| `cli.rs:147-148` | URL trailing slash creates double-slash | `trim_end_matches('/')` | 404 from health endpoint |
| `cli.rs:427-429` | Nested env var syntax not warned | Improve pattern matching | Complex env vars silently wrong |

---

## Priority Assessment

### Critical (Fix Immediately)
1. **SSRF bypass vectors** — missing IPv4/IPv6 ranges (Section 1.1)
2. **ToolChoice::None → type "any"** — forces tool use when disabled (Section 5)
3. **NaN propagation in weight/EWMA** — corrupts all routing decisions (Section 3)
4. **Silently dropped provider data** — system prompts, inline images, tool_choice (Section 5)

### High (Fix Before Next Release)
5. **Integer overflow on counters** — debug panics in health/state machine (Section 2)
6. **Silent DB parse failures** — corrupt data masked as missing (Section 6)
7. **Health state machine gaps** — negative wraps, indefinite cooldown (Section 4)
8. **X-Forwarded-For spoofing** — rate limit bypass (Section 1.2)

### Medium (Fix Soon)
9. **Tracing double-complete** — latency/status corruption (Section 7)
10. **Registry policy empty strings** — blocks or boosts all models (Section 8)
11. **Config validation drift** — CLI vs gateway (Section 10)
12. **Executor off-by-one** — extra fallback attempt (Section 9)

### Low (Backlog)
13. **Float precision for extreme values** — token counts, cost estimates
14. **Partial flush handling** — dirty flag cleared on failed writes
15. **ProviderCategory::parse misleading Option return** — dead code
16. **Nondeterministic eviction order** — stable sort needed
