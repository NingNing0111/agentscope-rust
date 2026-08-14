# Dependency Evaluation Records: Dependency Simplification

**Feature**: 028-dependency-simplification  
**Date**: 2026-08-11  
**Contract**: [dependency-evaluation-contract.md](./contracts/dependency-evaluation-contract.md)

This file records third-party dependency decisions required before implementing the first dependency-simplification batch. Approved dependencies are introduced through narrow project-owned wrappers so observable AgentScope Rust behavior remains stable.

## Dependency Evaluation: gray_matter

**Candidate IDs**: skill-frontmatter-parser, memory-frontmatter-parser  
**Decision**: approve-with-wrapper  
**Version Policy**: Use workspace dependency `gray_matter = { version = "0.3.2", default-features = false, features = ["yaml"] }` only from `agent_scope_frontmatter`; downstream crates depend on the project wrapper rather than parsing frontmatter directly.

### Responsibility

Parse YAML frontmatter metadata from markdown-like content so duplicated hand-written `SKILL.md` parsing and memory metadata parsing can be consolidated behind a compatibility wrapper.

### Maintenance Health

- Recent releases: crates.io reports current version 0.3.2.
- Ecosystem usage: Purpose-built crate for markdown frontmatter parsing with YAML/JSON/TOML support; narrower ecosystem footprint than a broad site-generator framework.
- API stability: Small API surface; use is isolated behind `agent_scope_frontmatter` to protect project callers from upstream API drift.
- Rust edition/MSRV considerations: No rust-version is declared in crate metadata; build verification in this workspace is required before acceptance.

### License Compatibility

MIT license, compatible with this project’s Apache-2.0 distribution.

### Security Posture

- Advisory check: No project-known advisory is recorded in this feature evaluation; workspace dependency tree and CI validation remain required before release.
- Unsafe exposure: No unsafe usage is introduced in project code by this wrapper decision.
- Untrusted input handling considerations: Frontmatter content can be user-provided. The project wrapper must keep malformed frontmatter as graceful fallback for skill discovery and must avoid panics or lossy persisted-memory reads.

### Transitive Dependency Footprint

- Direct dependency features: Disable defaults except YAML support.
- Major transitive dependencies: YAML parsing support is the main footprint; JSON/TOML features are intentionally disabled.
- Footprint rationale: A dedicated wrapper consolidates two duplicated parsers and memory metadata parsing while constraining optional parser features.

### Dependency Direction Fit

Only the new leaf helper crate `agent_scope_frontmatter` depends on `gray_matter`. `agent_scope_tool`, `agent_scope_workspace`, and `agent_scope_memory` depend on the helper. This avoids cycles and keeps provider/core protocol crates unaffected.

### Duplicate Responsibility Check

The project currently has at least two duplicated `parse_skill_md` implementations and a separate memory frontmatter parser. This dependency is adopted only through consolidation into the shared helper.

### Compatibility Fit

The wrapper owns all legacy behavior: delimiter recognition, malformed-frontmatter fallback, skill `name`/`description` extraction, block-scalar compatibility, memory field names, and body layout. Callers must not depend directly on `gray_matter` behavior that differs from existing AgentScope Rust parsing semantics.

### Decision Rationale

Approve with wrapper because frontmatter parsing is commodity behavior, duplicated locally, and safe to centralize if compatibility tests prove the wrapper preserves legacy edge cases.

## Dependency Evaluation: globset

**Candidate IDs**: pi-rust-file-discovery  
**Decision**: approve-with-wrapper  
**Version Policy**: Use workspace dependency `globset = "0.4.20"` from `examples/pi-rust` for glob pattern matching only; keep project-owned traversal, hidden-entry, symlink, cap, and output-shape policies.

### Responsibility

Compile and match glob patterns for pi-rust `Glob` tool behavior, replacing the custom `glob_to_regex` matcher while keeping project-specific filesystem policy outside the crate.

### Maintenance Health

- Recent releases: crates.io reports current version 0.4.20.
- Ecosystem usage: Maintained by the ripgrep ecosystem; commonly used for high-quality glob matching.
- API stability: Stable builder/matcher APIs; use is localized to pi-rust tools.
- Rust edition/MSRV considerations: Crate metadata reports rust-version 1.88, acceptable for this Rust 2024 workspace if CI confirms the active toolchain.

### License Compatibility

Dual licensed Unlicense OR MIT, compatible with Apache-2.0 project distribution.

### Security Posture

- Advisory check: No project-known advisory is recorded in this feature evaluation; workspace validation remains required.
- Unsafe exposure: No unsafe usage is introduced in project code.
- Untrusted input handling considerations: User-provided patterns must be compiled with bounded error handling; invalid patterns should return the existing invalid-arguments tool error rather than panic.

### Transitive Dependency Footprint

- Direct dependency features: Default features are acceptable; no serde/arbitrary/simd features are needed.
- Major transitive dependencies: Regex/glob matching stack maintained by the ripgrep/BurntSushi ecosystem.
- Footprint rationale: Replaces custom glob-to-regex code with a mature glob matcher and avoids adding broader ignore semantics.

### Dependency Direction Fit

Used only by the pi-rust example crate. No library crate or provider dependency edge is introduced.

### Duplicate Responsibility Check

The current duplicate responsibility is local custom glob-to-regex behavior in `examples/pi-rust/src/tools.rs`. The wrapper replaces only matching, not workspace path containment.

### Compatibility Fit

The wrapper must preserve relative path matching, `**/` zero-or-more-directory behavior, sorted output, result caps, scan caps, hidden-entry skipping, and symlink skipping.

### Decision Rationale

Approve with wrapper because glob pattern matching is commodity logic and `globset` is mature, but pi-rust must keep project-owned tool semantics around matching.

## Dependency Evaluation: walkdir

**Candidate IDs**: pi-rust-file-discovery  
**Decision**: approve-with-wrapper  
**Version Policy**: Use workspace dependency `walkdir = "2.5.0"` from `examples/pi-rust` for bounded recursive traversal only; keep hidden-entry and symlink policies explicit in project code.

### Responsibility

Provide recursive directory traversal for pi-rust `Glob` and `Grep`, replacing manual DFS stack handling.

### Maintenance Health

- Recent releases: crates.io reports current version 2.5.0.
- Ecosystem usage: Widely used Rust directory traversal crate from the same ecosystem as ripgrep.
- API stability: Long-lived iterator API with stable behavior.
- Rust edition/MSRV considerations: No rust-version is declared in crate metadata; workspace CI validates compatibility.

### License Compatibility

Dual licensed Unlicense/MIT, compatible with Apache-2.0 project distribution.

### Security Posture

- Advisory check: No project-known advisory is recorded in this feature evaluation; workspace validation remains required.
- Unsafe exposure: No unsafe usage is introduced in project code.
- Untrusted input handling considerations: Traversal must not follow symlinks, must retain workspace path resolution checks, and must preserve scan caps for large trees.

### Transitive Dependency Footprint

- Direct dependency features: Default feature set.
- Major transitive dependencies: Minimal traversal-focused footprint.
- Footprint rationale: Small, established dependency that replaces hand-written recursive traversal and integrates cleanly with existing caps.

### Dependency Direction Fit

Used only by `examples/pi-rust`; no core, provider, model, event, message, sandbox, or state crate dependency edge changes.

### Duplicate Responsibility Check

Manual traversal currently exists inside pi-rust tools. `walkdir` centralizes traversal mechanics while project code retains filtering and safety policy.

### Compatibility Fit

`follow_links(false)` and explicit filtering must preserve the existing hidden-entry skip, symlink skip, relative path output, deterministic sorting, and scan/result caps.

### Decision Rationale

Approve with wrapper because recursive directory walking is commodity behavior and the wrapper keeps all compatibility-sensitive tool policies under project control.

## Dependency Evaluation: thiserror

**Candidate IDs**: typed-error-derives  
**Decision**: approve  
**Version Policy**: Use workspace dependency `thiserror = "2"` for typed library error enums; migrate existing direct crate dependencies to `thiserror.workspace = true` where touched.

### Responsibility

Derive `std::error::Error`, `Display`, `source()`, and selected `From` implementations for typed error enums without replacing public error types with untyped errors.

### Maintenance Health

- Recent releases: crates.io reports 2.0.x with latest 2.0.20.
- Ecosystem usage: Standard Rust ecosystem crate for deriving typed errors.
- API stability: Mature derive macros with stable attributes for display and source handling.
- Rust edition/MSRV considerations: Crate metadata reports rust-version 1.71, compatible with this Rust 2024 workspace.

### License Compatibility

Dual licensed MIT OR Apache-2.0, directly compatible with this project’s Apache-2.0 distribution.

### Security Posture

- Advisory check: No project-known advisory is recorded in this feature evaluation; workspace validation remains required.
- Unsafe exposure: No unsafe usage is introduced in project code.
- Untrusted input handling considerations: Error strings may include user/tool/provider data exactly as before; no redaction policy changes are introduced by the derive migration.

### Transitive Dependency Footprint

- Direct dependency features: Default `std` feature.
- Major transitive dependencies: Procedural macro dependencies used at compile time.
- Footprint rationale: Several crates already use direct `thiserror = "2"`; moving to a workspace dependency consolidates existing dependency usage and reduces boilerplate.

### Dependency Direction Fit

`thiserror` is a compile-time derive dependency and does not create cross-crate coupling. It can be used independently by eligible library crates.

### Duplicate Responsibility Check

The project has repeated hand-written `Display`, `Error::source`, and `From` boilerplate for typed errors. `thiserror` replaces that boilerplate while preserving each enum as the public API.

### Compatibility Fit

Migrations must keep enum variants and fields unchanged, keep `Display` text byte-for-byte compatible, and preserve exactly which variants expose `source()`.

### Decision Rationale

Approve because `thiserror` is established, already partially present in the workspace, and directly targets low-risk boilerplate without changing public typed-error design.
