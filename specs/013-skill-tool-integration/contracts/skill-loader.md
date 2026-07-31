# Contract: SkillLoader Trait

**Feature**: 013-skill-tool-integration | **Status**: Draft

## Interface

```rust
/// Abstract loader that can provide [`Skill`] instances from any source
/// (local filesystem, remote service, MCP, etc.).
///
/// # Thread Safety
/// `Send + Sync` — safe to share via `Arc<dyn SkillLoader>`.
#[async_trait::async_trait]
pub trait SkillLoader: Send + Sync {
    /// Return all skills this loader can provide.
    ///
    /// # Errors
    /// Implementations SHOULD return an empty `Vec` rather than error
    /// when the source is temporarily unavailable, logging a warning.
    async fn list_skills(&self) -> Vec<Skill>;
}
```

## Where Used

- Implemented by: `LocalSkillLoader`
- Consumed by: `ToolGroup.skills_or_loaders` → `ToolGroup::list_skills()`
- Dependency: `agent_scope_workspace::Skill`

## Contract Guarantees

| Guarantee | Detail |
|-----------|--------|
| Thread safety | `Send + Sync` |
| Graceful degradation | Implementations return `[]` on I/O errors, never panic |
| No mutable state | `&self` — caches behind `RwLock` if needed |
| No unsafe | All implementations MUST be safe Rust |

## Cross-reference

- Python: `SkillLoaderBase` in `agentscope/src/agentscope/skill/_base.py:23`
- Spec: `spec.md` FR-008
