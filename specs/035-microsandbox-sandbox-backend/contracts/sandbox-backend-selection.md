# Contract: Sandbox Backend Selection

**Feature**: `035-microsandbox-sandbox-backend`

## Backend choices

Supported choices for this feature:

- `LocalSandboxSession`: local-process reference backend, no microVM hard isolation.
- `MicrosandboxSession`: feature-gated microsandbox microVM backend.

Future Cloud support is out of scope for this feature and must be designed separately.

## Selection rules

- The caller chooses the backend by constructing the corresponding session type.
- `SandboxWorkspaceBackend::new(LocalSandboxSession)` preserves existing local-process usage.
- `SandboxWorkspaceBackend::from_session(MicrosandboxSession)` or `from_boxed_session(Box<dyn SandboxSession>)` selects microsandbox when the feature is enabled.
- Runtime/platform/SDK failures in microsandbox must not fall back to local-process.

## Cloud rules

Cloud execution is an external service boundary and requires explicit user configuration in future work. The following environment variables do not select Cloud by themselves:

- `MSB_API_KEY`
- `MSB_API_URL`

This feature must not infer Cloud from credentials, URLs, active environment, or profiles unless a future explicit backend-kind setting is added.

## Security rules

- Do not automatically install or manage `msb` runtime.
- Do not automatically mount host credential paths.
- Do not inject real secrets into sandbox env or logs.
- Treat all sandbox output as untrusted data.
