# Contract: Dependency Evaluation Record

**Feature**: Dependency Simplification  
**Date**: 2026-08-11  
**Related model**: [data-model.md](../data-model.md)

## Purpose

This contract defines the maintainer-facing record that must exist before a simplification candidate may introduce or expand a third-party dependency. It is not an HTTP API. It is a documentation and review contract for implementation tasks generated from this feature.

## Required Record Shape

Each dependency evaluation should be stored with the candidate task or in a dedicated implementation note using this shape:

```markdown
## Dependency Evaluation: <dependency-name>

**Candidate IDs**: <candidate-id>[, <candidate-id>...]  
**Decision**: approve | approve-with-wrapper | defer | reject  
**Version Policy**: <workspace dependency version and feature policy>

### Responsibility

<Commodity responsibility this dependency would take over.>

### Maintenance Health

- Recent releases: <summary>
- Ecosystem usage: <summary>
- API stability: <summary>
- Rust edition/MSRV considerations: <summary>

### License Compatibility

<License and compatibility with the project’s Apache-2.0 distribution.>

### Security Posture

- Advisory check: <summary>
- Unsafe exposure: <none | documented reason>
- Untrusted input handling considerations: <summary>

### Transitive Dependency Footprint

- Direct dependency features: <features>
- Major transitive dependencies: <summary>
- Footprint rationale: <why acceptable or unacceptable>

### Dependency Direction Fit

<How this dependency respects workspace layering and avoids core/provider/infra coupling.>

### Duplicate Responsibility Check

<Existing crates or helpers that perform the same job; explain consolidation or exception.>

### Compatibility Fit

<Behavior differences, wrapper requirements, legacy compatibility concerns, and public behavior preserved.>

### Decision Rationale

<Why this dependency decision is acceptable.>
```

## Validation Rules

A dependency evaluation is valid only when all of these checks pass:

1. `Decision` is one of `approve`, `approve-with-wrapper`, `defer`, or `reject`.
2. `Candidate IDs` reference candidates listed in `data-model.md` or the current `tasks.md` candidate inventory.
3. Approved dependencies record a concrete version policy and feature policy.
4. License compatibility is explicitly compatible with Apache-2.0 project distribution.
5. Security posture includes an advisory check and notes any unsafe-code exposure.
6. Transitive footprint is documented, including why the dependency is worth its cost.
7. Dependency direction fit confirms no forbidden crate dependency edge is introduced.
8. Duplicate responsibility check either consolidates existing helpers or documents why duplication is intentional.
9. Compatibility fit names the project-owned behavior that must remain stable.
10. `approve-with-wrapper` explicitly states what the wrapper preserves.

## Candidate-to-Dependency Expectations

| Candidate | Expected dependency class | Contract emphasis |
|-----------|---------------------------|-------------------|
| `skill-frontmatter-parser` | YAML/frontmatter parser | malformed input fallback and block scalar compatibility |
| `memory-frontmatter-parser` | YAML/frontmatter parser | persistent file round-trip and legacy read compatibility |
| `pi-rust-file-discovery` | `globset`, `walkdir`, or `ignore` style crates | relative paths, hidden/symlink policy, caps, literal grep semantics |
| `typed-error-derives` | `thiserror` | public enum shape, display messages, source chains |
| `path-component-sanitization` | filename/slug helper | legacy persisted paths, collision policy, provider tool-name constraints |
| `tool-context-cache` | `indexmap`, `lru`, or local wrapper | serialized shape and eviction semantics |

## Non-Adoption Contract

For `defer` or `reject` decisions, the record may omit version policy details, but it must include:

- the blocking reason,
- the behavior or governance risk,
- what evidence would be required to revisit the decision.

Rejected protocol/security candidates must not be converted into implementation tasks unless a new spec explicitly changes the compatibility or security boundary.
