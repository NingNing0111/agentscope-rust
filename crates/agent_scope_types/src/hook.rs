//! Agent hook type definitions.

/// Pre-defined agent hook points (6).
pub mod agent_hooks {
    pub const PRE_REPLY: &str = "pre_reply";
    pub const POST_REPLY: &str = "post_reply";
    pub const PRE_PRINT: &str = "pre_print";
    pub const POST_PRINT: &str = "post_print";
    pub const PRE_OBSERVE: &str = "pre_observe";
    pub const POST_OBSERVE: &str = "post_observe";
}

/// ReAct agent hook points (extends agent_hooks with 4 additional hooks).
pub mod react_agent_hooks {
    pub use super::agent_hooks::*;
    pub const PRE_REASONING: &str = "pre_reasoning";
    pub const POST_REASONING: &str = "post_reasoning";
    pub const PRE_ACTING: &str = "pre_acting";
    pub const POST_ACTING: &str = "post_acting";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_hook_constants() {
        assert_eq!(agent_hooks::PRE_REPLY, "pre_reply");
        assert_eq!(agent_hooks::POST_REPLY, "post_reply");
        assert_eq!(agent_hooks::PRE_PRINT, "pre_print");
        assert_eq!(agent_hooks::POST_PRINT, "post_print");
        assert_eq!(agent_hooks::PRE_OBSERVE, "pre_observe");
        assert_eq!(agent_hooks::POST_OBSERVE, "post_observe");
    }

    #[test]
    fn test_react_agent_hook_constants() {
        // Inherits all agent hooks
        assert_eq!(react_agent_hooks::PRE_REPLY, "pre_reply");
        assert_eq!(react_agent_hooks::POST_REPLY, "post_reply");
        assert_eq!(react_agent_hooks::PRE_PRINT, "pre_print");
        assert_eq!(react_agent_hooks::POST_PRINT, "post_print");
        assert_eq!(react_agent_hooks::PRE_OBSERVE, "pre_observe");
        assert_eq!(react_agent_hooks::POST_OBSERVE, "post_observe");
        // Plus 4 ReAct-specific hooks
        assert_eq!(react_agent_hooks::PRE_REASONING, "pre_reasoning");
        assert_eq!(react_agent_hooks::POST_REASONING, "post_reasoning");
        assert_eq!(react_agent_hooks::PRE_ACTING, "pre_acting");
        assert_eq!(react_agent_hooks::POST_ACTING, "post_acting");
    }
}
