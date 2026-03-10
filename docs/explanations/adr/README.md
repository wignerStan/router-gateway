# Architecture Decision Records (ADR)

This directory contains Architecture Decision Records for the LLM Gateway project.

## What is an ADR?

An Architecture Decision Record is a document that captures an important architectural decision along with its context and consequences. ADRs help future developers understand why the system is built the way it is.

## ADR Format

Each ADR follows this structure:

- **Title:** A short noun phrase describing the decision
- **Status:** Proposed, Accepted, Deprecated, Superseded
- **Context:** The issue motivating this decision
- **Decision:** The change being proposed or made
- **Consequences:** What becomes easier or harder as a result

## Naming Convention

ADR files are numbered sequentially and use kebab-case:

```
NNNN-short-title.md
```

Example: `0001-record-architecture-decisions.md`

## Creating a New ADR

1. Copy `0000-template.md` (if available) or use the format above
2. Assign the next available number
3. Write a clear title that describes the decision, not the problem
4. Fill in all sections
5. Submit for review if working in a team

## When to Write an ADR

Write an ADR when you make a decision that:

- Affects the structure or characteristics of the system
- Is difficult to change later
- Has significant trade-offs
- Other developers will ask about

## Index

| Number                                        | Title                         | Status   |
| --------------------------------------------- | ----------------------------- | -------- |
| [0001](0001-record-architecture-decisions.md) | Record Architecture Decisions | Accepted |

## References

- [Documenting Architecture Decisions (Michael Nygard)](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- [Architecture Decision Records (GitHub)](https://adr.github.io/)
