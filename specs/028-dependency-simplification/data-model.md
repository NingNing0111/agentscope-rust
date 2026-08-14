# Data Model: Dependency Simplification

**Feature**: Dependency Simplification  
**Date**: 2026-08-11  
**Spec**: [spec.md](./spec.md)

## Overview

This feature does not introduce a runtime database schema. Its data model defines maintainer-facing planning records used to inventory simplification candidates, evaluate external dependencies, and capture behavior-preservation evidence before implementation tasks are generated.

## Entity: Simplification Candidate

A current in-project implementation under review for replacement or consolidation.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Stable candidate identifier, e.g. `frontmatter-skill-parser` |
| `title` | string | yes | Human-readable candidate name |
| `status` | enum | yes | `adopt`, `adopt-cautiously`, `defer`, or `reject` |
| `priority` | enum | yes | `P1`, `P2`, or `P3` for implementation ordering |
| `affected_crates` | list[string] | yes | Crates or examples affected by the candidate |
| `affected_files` | list[path] | yes | Project-relative files/modules that contain the current implementation |
| `current_responsibility` | string | yes | What the current custom implementation does |
| `commodity_rationale` | string | yes | Why this is or is not a commodity/basic responsibility |
| `public_behavior` | list[string] | yes | Observable behavior that must remain stable |
| `risk_level` | enum | yes | `low`, `medium`, `high`, or `security-critical` |
| `replacement_direction` | list[string] | conditional | Candidate crate families or dependency policy changes, required when status is `adopt` or `adopt-cautiously` |
| `dependency_evaluations` | list[DependencyEvaluationRef] | conditional | Required for adopted candidates before implementation |
| `evidence_requirements` | list[BehaviorEvidenceRef] | yes | Validation evidence needed before completion |
| `decision_rationale` | string | yes | Why this status was selected |
| `alternatives_considered` | list[string] | yes | Rejected or deferred alternatives |
| `legacy_compatibility_notes` | string | optional | Persisted format, public API, or migration notes |

### Validation Rules

- `id` must be unique across the candidate inventory.
- `status=adopt` requires at least one concrete dependency evaluation or an explicit “no new dependency, workspace consolidation only” note.
- `status=adopt-cautiously` requires a compatibility wrapper or legacy migration strategy.
- `status=defer` or `status=reject` requires a rationale sufficient for future maintainers.
- Any candidate touching serialized data, event order, errors, cancellation, public APIs, persisted paths, or security boundaries must be `medium` risk or higher.
- Any candidate touching sandbox/path containment must be `security-critical` unless the change is documentation-only.
- Every candidate must map to at least one Functional Requirement from the spec.

### State Transitions

```text
identified
  -> evaluated
  -> adopt | adopt-cautiously | defer | reject

adopt | adopt-cautiously
  -> implementation-ready
  -> implemented
  -> validated

implemented
  -> deferred        # if validation reveals unacceptable behavior drift
  -> rejected        # if dependency review fails
```

A candidate may not move to `implementation-ready` until dependency evaluation and behavior evidence requirements are complete.

## Entity: External Dependency Evaluation

A governance record for a proposed crate or dependency-policy change.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `dependency_name` | string | yes | Crate or dependency family under evaluation |
| `version_policy` | string | yes | Proposed version requirement and workspace dependency policy |
| `used_by_candidates` | list[CandidateRef] | yes | Candidates that would use this dependency |
| `responsibility` | string | yes | Commodity function supplied by the dependency |
| `maintenance_health` | string | yes | Release cadence, ecosystem usage, issue activity, and API stability |
| `license_compatibility` | string | yes | Apache-2.0 compatibility assessment |
| `security_posture` | string | yes | Advisory and unsafe-code review summary |
| `transitive_footprint` | string | yes | Expected dependency tree impact and feature flags |
| `dependency_direction_fit` | string | yes | Confirmation that crate layering remains valid |
| `duplicate_responsibility_check` | string | yes | Whether the project already depends on an equivalent crate |
| `compatibility_fit` | string | yes | Known behavior differences and wrapper requirements |
| `decision` | enum | yes | `approve`, `approve-with-wrapper`, `defer`, or `reject` |
| `decision_rationale` | string | yes | Why the dependency decision is acceptable |

### Validation Rules

- License compatibility must be explicitly recorded before approval.
- Security posture must mention whether known advisories were checked.
- Transitive dependency footprint must identify major new dependency families and enabled features.
- `approve-with-wrapper` requires the wrapper contract to name which behavior is project-owned.
- `reject` must identify at least one blocking reason: license, maintenance, security, footprint, compatibility, or layering.
- Dependencies shared across multiple candidates should be declared in `[workspace.dependencies]` unless there is a documented exception.

## Entity: Behavior Preservation Evidence

Evidence that an adopted replacement did not change externally observable behavior.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `candidate_id` | CandidateRef | yes | Candidate being validated |
| `evidence_type` | enum | yes | `unit-test`, `golden-test`, `integration-test`, `example-run`, `doc-check`, `manual-review`, or `dependency-audit` |
| `command_or_artifact` | string | yes | Test command, artifact path, or review document |
| `behavior_covered` | list[string] | yes | Public behavior covered by this evidence |
| `expected_result` | string | yes | Expected passing condition |
| `actual_result` | string | conditional | Filled during implementation validation |
| `compatibility_exception` | string | optional | Approved exception, if behavior intentionally changes |
| `status` | enum | yes | `required`, `passed`, `failed`, or `waived` |

### Validation Rules

- Every adopted candidate must include evidence for all affected externally observable behaviors.
- A failed evidence item blocks completion unless the candidate is deferred or an approved exception is recorded.
- Waivers require a rationale and must not be used for public API, security, or persisted format regressions.
- Example-facing changes require an example run or equivalent integration test.
- Persistent format changes require round-trip and legacy-read tests.

## Entity: Candidate Inventory

A collection-level record proving the feature meets the minimum review scope.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `feature_id` | string | yes | `028-dependency-simplification` |
| `review_date` | date | yes | Date the inventory was prepared |
| `candidates` | list[SimplificationCandidate] | yes | Reviewed candidates |
| `summary_counts` | object | yes | Count by status and risk level |
| `first_batch` | list[CandidateRef] | yes | Candidates recommended for first implementation tasks |
| `out_of_scope` | list[string] | optional | Explicitly excluded areas |

### Validation Rules

- Inventory must contain at least 10 reviewed candidates to satisfy SC-001.
- Inventory must either identify at least 3 approved low-risk simplifications or document why all candidates are unsuitable to satisfy SC-002.
- Inventory must include reject/defer records for compatibility-sensitive areas that were considered and excluded.

## Relationships

```text
Candidate Inventory
  contains many Simplification Candidates

Simplification Candidate
  references zero or more External Dependency Evaluations
  requires one or more Behavior Preservation Evidence records

External Dependency Evaluation
  may support many Simplification Candidates

Behavior Preservation Evidence
  belongs to exactly one Simplification Candidate
```

## Initial Candidate Set

The planning inventory currently includes these reviewed candidates:

| ID | Title | Status | Risk |
|----|-------|--------|------|
| `skill-frontmatter-parser` | Shared `SKILL.md` frontmatter parser | adopt | medium |
| `memory-frontmatter-parser` | Memory markdown frontmatter parser/serializer | adopt | medium |
| `pi-rust-file-discovery` | pi-rust Glob/Grep/ListDir traversal and matching | adopt | medium |
| `typed-error-derives` | Replace hand-written error impls with `thiserror` | adopt | low |
| `path-component-sanitization` | Consolidate safe filename/tool/path component policies | adopt-cautiously | medium |
| `tool-context-cache` | Replace manual Vec cache helper | adopt-cautiously | medium |
| `dashscope-sse-framing` | Replace streaming SSE byte/line framing | defer | medium |
| `model-retry-backoff` | Use retry/backoff abstraction for model calls | defer | medium |
| `json-repair-and-schema-flatten` | Replace JSON repair/schema flatten helpers | defer | high |
| `json-file-session-store` | Replace atomic JSON file persistence helper | defer | high |
| `event-protocol-types` | Replace AgentScope event protocol models | reject | high |
| `message-content-protocol` | Replace message/content block models | reject | high |
| `sandbox-path-containment` | Replace sandbox path containment policy | reject | security-critical |
| `mcp-internal-model-replacement` | Replace internal tool/workspace abstractions with MCP-native types | defer | high |

This set satisfies the planning-level inventory target of at least 10 reviewed candidates. Implementation tasks should select a smaller first batch and attach concrete dependency evaluations before code changes.
