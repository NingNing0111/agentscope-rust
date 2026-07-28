//! Integration tests for hook type constants.

use agent_scope_types::hook::agent_hooks;
use agent_scope_types::hook::react_agent_hooks;

#[test]
fn test_all_agent_hook_constants_present_and_correct() {
    assert_eq!(agent_hooks::PRE_REPLY, "pre_reply");
    assert_eq!(agent_hooks::POST_REPLY, "post_reply");
    assert_eq!(agent_hooks::PRE_PRINT, "pre_print");
    assert_eq!(agent_hooks::POST_PRINT, "post_print");
    assert_eq!(agent_hooks::PRE_OBSERVE, "pre_observe");
    assert_eq!(agent_hooks::POST_OBSERVE, "post_observe");
}

#[test]
fn test_react_agent_hooks_inherits_and_extends() {
    // Inherited (same values as agent_hooks)
    assert_eq!(react_agent_hooks::PRE_REPLY, agent_hooks::PRE_REPLY);
    assert_eq!(react_agent_hooks::POST_REPLY, agent_hooks::POST_REPLY);
    assert_eq!(react_agent_hooks::PRE_PRINT, agent_hooks::PRE_PRINT);
    assert_eq!(react_agent_hooks::POST_PRINT, agent_hooks::POST_PRINT);
    assert_eq!(react_agent_hooks::PRE_OBSERVE, agent_hooks::PRE_OBSERVE);
    assert_eq!(react_agent_hooks::POST_OBSERVE, agent_hooks::POST_OBSERVE);

    // Extended
    assert_eq!(react_agent_hooks::PRE_REASONING, "pre_reasoning");
    assert_eq!(react_agent_hooks::POST_REASONING, "post_reasoning");
    assert_eq!(react_agent_hooks::PRE_ACTING, "pre_acting");
    assert_eq!(react_agent_hooks::POST_ACTING, "post_acting");
}

#[test]
fn test_total_hook_count() {
    // Agent hooks: 6, ReAct hooks: 6 + 4 = 10
    let agent_hook_names = [
        agent_hooks::PRE_REPLY,
        agent_hooks::POST_REPLY,
        agent_hooks::PRE_PRINT,
        agent_hooks::POST_PRINT,
        agent_hooks::PRE_OBSERVE,
        agent_hooks::POST_OBSERVE,
    ];
    assert_eq!(agent_hook_names.len(), 6);

    let react_hook_names = [
        react_agent_hooks::PRE_REPLY,
        react_agent_hooks::POST_REPLY,
        react_agent_hooks::PRE_PRINT,
        react_agent_hooks::POST_PRINT,
        react_agent_hooks::PRE_OBSERVE,
        react_agent_hooks::POST_OBSERVE,
        react_agent_hooks::PRE_REASONING,
        react_agent_hooks::POST_REASONING,
        react_agent_hooks::PRE_ACTING,
        react_agent_hooks::POST_ACTING,
    ];
    assert_eq!(react_hook_names.len(), 10);
}

#[test]
fn test_hook_constants_are_unique() {
    use std::collections::HashSet;
    let all_hooks: HashSet<&&str> = HashSet::from_iter([
        &agent_hooks::PRE_REPLY,
        &agent_hooks::POST_REPLY,
        &agent_hooks::PRE_PRINT,
        &agent_hooks::POST_PRINT,
        &agent_hooks::PRE_OBSERVE,
        &agent_hooks::POST_OBSERVE,
    ]);
    assert_eq!(all_hooks.len(), 6, "all agent hook names must be unique");

    let react_hooks: HashSet<&&str> = HashSet::from_iter([
        &react_agent_hooks::PRE_REPLY,
        &react_agent_hooks::POST_REPLY,
        &react_agent_hooks::PRE_PRINT,
        &react_agent_hooks::POST_PRINT,
        &react_agent_hooks::PRE_OBSERVE,
        &react_agent_hooks::POST_OBSERVE,
        &react_agent_hooks::PRE_REASONING,
        &react_agent_hooks::POST_REASONING,
        &react_agent_hooks::PRE_ACTING,
        &react_agent_hooks::POST_ACTING,
    ]);
    assert_eq!(react_hooks.len(), 10, "all react hook names must be unique");
}
