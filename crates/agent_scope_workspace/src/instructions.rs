//! Default workspace instructions template.

/// Default workspace system instructions (translated from Python AgentScope).
pub const DEFAULT_WORKSPACE_INSTRUCTIONS: &str = r#"You are working in a workspace environment. The workspace is a directory on the filesystem where you can read, write, create, and delete files.

## Workspace Information
- Working directory: {workdir}
- Backend type: {backend}

## Available Capabilities
You have access to the following built-in tools within this workspace:
- **Read**: Read file contents from the workspace
- **Write**: Write content to files in the workspace
- **Edit**: Make precise string replacements in files
- **Bash**: Execute shell commands within the workspace directory
- **Glob**: Find files matching patterns in the workspace
- **Grep**: Search for patterns in workspace files

## Important Rules
1. All file operations are scoped to the workspace directory ({workdir})
2. File paths outside the workspace cannot be accessed
3. When using Bash, the working directory is {workdir}
4. Be careful with destructive operations — deleted files cannot be recovered
5. You can organize your work using subdirectories within the workspace
"#;
