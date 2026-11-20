# ADR-001 — Code Mode tool calling

Status: **accepted as design direction** · Date: 2026-11-20 ·
Reference: [Cloudflare, “Code Mode: the better way to use MCP”](https://blog.cloudflare.com/code-mode/)

## What Code Mode establishes

Most agents expose each MCP tool directly as an LLM function call. Code
Mode instead compiles the tool schemas into a **typed, documented code
API**, loads that API into context, and gives the model **one tool**:
execute code. The model writes a single program that chains calls; only
the final printed output returns to the model. Intermediate results never
re-enter the context window.

Why this wins, per the article and our reading:

1. Models handle many complex tools better as a typed API (TypeScript in
   the article) than as synthetic JSON tool-call syntax — the former
   dominates their training data, the latter does not.
2. Chained calls stop wasting tokens: values flow program-to-program
   instead of being copied through the model's context between turns.
3. The sandbox holds **bindings, not keys**: execution is isolated with no
   general network access, and authorization is attached by the supervisor
   outside the model-written code, so secrets cannot leak through it.

## Mapping onto StainlessSteel

| Code Mode element | StainlessSteel equivalent |
|---|---|
| MCP tools → TypeScript API + doc comments | Native tool descriptors + JSON Schemas served by `stainless tool list` (§4 registry); MCP schema import when rmcp lands (Phase 3) |
| One `execute` tool | A single batch-execution entry in the tool broker: one model turn carries one program, the broker runs it and returns final outputs only |
| V8 isolate per snippet, no internet | Broker-side sandbox policy: capability-scoped execution, no ambient network/credentials; Wasmtime WASI components where the blueprint requires a runtime (Phase 6), OS sandbox backends per platform (Phase 5) |
| RPC bindings back to the supervisor | Capability handles from the credential broker (vault workstream): the program names a grant, the broker attaches the secret in transport |
| `console.log` as the result channel | Bounded final-output capture with content-addressed references to full outputs (blueprint §4 `ToolOutcome`) |

## Decision

All future MCP and tool-calling work follows Code Mode: **no per-call
JSON tool loop as the primary interface**. The model programs against the
generated API; the broker executes, redacts, and accounts. Direct
single-shot invocation remains only as a debug/operator path, never as
the agent's main loop.

## Constraints carried over

- Rust core stays free of a JS runtime: the program surface is the typed
  registry (today) and generated API bindings (Phase 3), not an embedded
  TypeScript engine. Cloudflare's Workers/isolates are their sandbox; ours
  is the broker + platform sandbox backends.
- No fake execution: until MCP/WASI backends exist, batch programs run
  only over natively implemented tools. The broker refuses unknown tool
  ids instead of pretending.
- `stainless tool list` is the first slice: uniform API + documentation.
  Next slices: batch program format and broker executor (Phase 2/3),
  MCP schema importer (Phase 3), vault-bound capability handles (vault
  workstream).

## Consequences

- Token cost of chained tool use drops by construction (no intermediate
  re-feeds); this becomes a measured benchmark in the quality scorecard.
- Prompt-cache layout must treat the generated API surface as stable
  hot-prefix content (ties into `CACHE002` tool-definition-drift lint).
- Security review burden shifts from per-call schemas to the executor
  sandbox and the capability-grant boundary — reflected in the failure
  matrix (S05, S06, S12, S13).
