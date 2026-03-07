# First Principles of Agentic Coding

This document summarizes the fundamental LLM mechanics that inform how we write effective `AGENTS.md` files.

## LLM Fundamentals

### 1. Autoregressive Generation

LLMs work by predicting one token at a time. They don't "think first, then speak" — they "think by speaking."

**Implications:**
- No independent "memory" exists outside the context window
- Reasoning quality depends on available context
- Errors compound over long generation sequences

### 2. Attention Mechanism

The model uses "attention" to dynamically focus on relevant parts of the context.

**Key Properties:**
- **Sparse**: Most context gets near-zero attention weight
- **Learned**: Patterns emerge from training data
- **Quadratic Cost**: O(n²) computation with context length

### 3. Context Window Limits

| Aspect | Reality |
|--------|---------|
| Physical Limit | 128K-200K tokens (current models) |
| Effective Limit | 10-15% of stated capacity |
| Performance Degradation | Starts after ~80K tokens |
| "Dumb Zone" | Middle 40-60% of context shows reduced recall |

---

## Agent-Specific Challenges

### 1. Local Optimization Bias
- Agents optimize step-by-step, not globally
- Can lead to inconsistent designs across files
- Remedies: Clear architecture docs, reference implementations

### 2. Error Snowballing
- Early misunderstandings compound in later decisions
- The agent "believes" its own hallucinations
- Remedies: Frequent verification, short sessions

### 3. Stream-Based Output
- Cannot revise what's already generated
- Tendency to rewrite rather than edit minimally
- Remedies: Encourage small, focused changes

### 4. Constraint Satisfaction Limits
- "Looks right" ≠ "Is correct"
- Struggles with edge cases, concurrency, resource management
- Remedies: Strong test coverage, type checking, linters

---

## Why Short Sessions Win

| Long Session | Short Session |
|--------------|--------------|
| Context fills with stale data | Fresh, focused context |
| Attention diluted | Attention concentrated |
| Compound errors | Isolated failures |
| Expensive (all history reprocessed) | Cost-effective |
| Agent "drunk" behavior | Agent at peak performance |

**Rule of Thumb:**
- Start new session at ~80K tokens OR when subtask completes
- One objective per session

---

## The AGENTS.md Leverage Effect

```
Configuration File (AGENTS.md)
      ↓ Affects every session
      ↓ Affects every task
      ↓ Affects every line of output
      
One bad line in AGENTS.md → Thousands of bad outputs
One good line in AGENTS.md → Thousands of good outputs
```

This is why `AGENTS.md` deserves careful, deliberate design.

---

## Compounding Engineering

Transform every interaction into permanent learning:

```
Bug Fix → Extract pattern → Add to Known Pitfalls
Code Review → Generalize feedback → Update Conventions
Successful PR → Reference as example → Add to Patterns
Agent Confusion → Clarify in docs → Reduce future errors
```

The system gets smarter over time, not just the developer.

---

## Expert Generalist Model

The ideal human-AI collaboration:

- **Human**: Cross-domain judgment, design taste, strategic decisions
- **Agent**: Execution, exploration, information retrieval

LLMs function like a "Jarvis exoskeleton" — amplifying Expert Generalists who can direct them effectively across multiple domains.

---

## Source

Derived from: "用第一性原理拆解 Agentic Coding" (Deconstructing Agentic Coding with First Principles), TRAE Engineering Blog
