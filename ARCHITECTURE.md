# ARCHITECTURE.md — ZeroBuild: The Autonomous Software Factory

## Vision

ZeroBuild is a **Virtual Software Company** powered entirely by AI. Through a **Hierarchical Multi-Agent System**, a user provides a raw idea in natural language, and ZeroBuild automatically assembles a team of AI specialists — Orchestrator (CEO), Business Analyst, UI/UX Designer, Developer, Tester, and DevOps Engineer — that coordinate autonomously to automate the entire software development lifecycle and deliver a production-ready product.

**Core promise:** From idea to working software in minutes, not months. Zero coding. Zero management. Ultra-low cost.

---

## Multi-Agent Factory Architecture

### Agent Role Hierarchy — The Virtual Software Company

```
User provides idea (any channel: Telegram / Discord / Slack / CLI)
    │
    ▼
┌───────────────────────────────────────┐
│  🏢 Orchestrator (CEO) │
│  • Receives idea, analyzes feasibility│
│  • Creates project plan               │
│  • Spawns specialized sub-agents      │
│  • Delegates tasks & supervises       │
│  • Reports progress to user           │
└───────┬───────────────────────────────┘
        │
        ├── Phase 1: Analysis
        │   └── Business Analyst Agent
        │       → Produces PRD (Product Requirements Document)
        │
        ├── Phase 2: Parallel Build
        │   ├── UI/UX Designer Agent  → Design spec + wireframes
        │   ├── Developer Agent       → Source code
        │   └── Tester Agent          → Test cases
        │
        ├── Phase 3: Integration Loop
        │   ├── Developer Agent  ←→  Tester Agent
        │   │   (ping-pong until tests pass, max N iterations)
        │   └── Blackboard: test results, code patches
        │
        └── Phase 4: Deployment
            └── DevOps Agent → Deploy config, push to GitHub
```

### Agent Roles

| Role | Responsibility | Inputs | Outputs |
|------|---------------|--------|---------|
| **Orchestrator** | Workflow coordination, phase management, user communication | User idea | Final deliverable |
| **Business Analyst** | Requirements analysis, PRD generation | User idea | PRD artifact |
| **UI/UX Designer** | Design specifications, component structure | PRD | Design spec artifact |
| **Developer** | Code generation, implementation | PRD + Design spec | Source code artifact |
| **Tester** | Test case generation, test execution | PRD + Source code | Test cases + results |
| **DevOps** | Deployment configuration, GitHub push | Source code + passing tests | Deploy config |

---

## Workflow Phases

### Phase 1: Analysis (Sequential)

```
User Idea → [BA Agent] → PRD → Blackboard
```

The Business Analyst agent analyzes the user's idea and produces a structured Product Requirements Document. This is the foundation for all subsequent work.

### Phase 2: Parallel Build

```
PRD → [UI/UX Agent]  → Design Spec  → Blackboard
    → [Developer]     → Source Code  → Blackboard
    → [Tester]        → Test Cases   → Blackboard
```

Three agents run concurrently via `tokio::join!`. Each reads the PRD from the blackboard and produces their respective artifacts.

### Phase 3: Integration Loop

```
Source Code + Test Cases → [Developer] ←→ [Tester]
                           (max N iterations, configurable)
```

The Developer and Tester agents enter a ping-pong loop:
1. Tester runs test cases against source code
2. If tests fail → Developer reads test results, fixes code
3. Repeat until all tests pass or iteration cap reached

Default cap: 5 iterations (configurable via `factory.max_ping_pong_iterations`).

### Phase 4: Deployment (Sequential)

```
Passing Source Code → [DevOps Agent] → Deploy Config → GitHub Push
```

Only triggers when Phase 3 produces passing test results.

---

## Blackboard Data Flow

The **Blackboard** is the shared state layer for inter-agent communication. It is built on top of the existing `InMemoryMessageBus` and `SharedContextEntry` coordination primitives.

```
┌──────────────────────────────────────────┐
│              Blackboard                  │
│                                          │
│  artifact:prd          → PRD JSON        │
│  artifact:design_spec  → Design Spec     │
│  artifact:source_code  → Code Manifest   │
│  artifact:test_cases   → Test Suite      │
│  artifact:test_results → Pass/Fail       │
│  artifact:deploy_config→ Deploy Config   │
│                                          │
│  Uses ContextPatch envelopes with        │
│  optimistic-locking versioning           │
└──────────────────────────────────────────┘
```

Agents publish artifacts via `publish_artifact()` and read them via `read_artifact()`. Version conflicts are handled by the existing `ContextVersionMismatch` error in the coordination protocol.

---

## Implementation Strategy

### Reuse Existing Primitives

The factory module builds on existing infrastructure rather than rewriting:

| Existing Primitive | Factory Usage |
|---|---|
| `DelegateTool` | Pattern for spawning sub-agents with filtered tool access |
| `DelegateAgentConfig` | Per-role provider/model/prompt configuration |
| `InMemoryMessageBus` | Blackboard transport layer |
| `SharedContextEntry` + `ContextPatch` | Artifact storage with versioned writes |
| `run_tool_call_loop` | Agent execution engine for each role |

### Module Structure

```
src/factory/
├── mod.rs                  # Module exports
├── roles.rs                # AgentRole enum, RoleConfig, system prompts
├── blackboard.rs           # Blackboard struct wrapping InMemoryMessageBus
├── workflow.rs             # WorkflowPhase state machine, FactoryWorkflow
└── orchestrator_tool.rs    # FactoryOrchestratorTool (Tool trait impl)
```

### Configuration

```toml
[factory]
enabled = true                     # Default: true. Agent autonomously decides when to use factory
max_ping_pong_iterations = 5       # Dev-Tester loop cap

[factory.provider_overrides.business_analyst]
provider = "openrouter"
model = "anthropic/claude-sonnet-4-6"
# ... per-role DelegateAgentConfig overrides
```

---

## Design Decisions

1. **Enabled by default** — `factory.enabled = true`. The `factory_build` tool is always available; the agent autonomously decides when to use it based on task complexity.
2. **Same sandbox** — All factory agents share the same sandbox filesystem, enabling collaborative file access.
3. **Hard iteration cap** — Prevents infinite dev-test loops. Configurable, default 5.
4. **No new traits** — Factory uses existing `Tool`, `Provider`, and coordination traits.
5. **Backward compatible** — No breaking changes to any existing interface.

---

## Development Roadmap

### Phase A: Foundation (Current)
- Factory module structure (`src/factory/`)
- Role definitions with system prompts
- Blackboard on top of `InMemoryMessageBus`
- Workflow state machine
- `factory_build` tool registration

### Phase B: Enhancement (Future)
- Agent memory sharing across phases
- Streaming progress updates to user
- Role-specific tool allowlists refinement
- Parallel agent health monitoring

### Phase C: Advanced (Future)
- Dynamic agent spawning based on project complexity
- Cross-project learning from previous builds
- Custom role definitions via config
- Agent performance metrics and optimization

---

## Cleanup Notes

The following areas may benefit from refocusing as the factory matures:

- **Single-agent assumptions** in `IDENTITY.md` and system prompts — update to acknowledge factory mode
- **Tool registry** — factory agents need filtered tool access per role
- **Progress reporting** — factory phases should emit structured progress events
- **Error escalation** — factory should aggregate errors across agents for user-facing reports
