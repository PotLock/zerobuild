# Factory Module — Implementation Status

> Last updated: 2026-03-02

## Overview

ZeroBuild's Autonomous Software Factory is a hierarchical multi-agent system where an Orchestrator (CEO) spawns specialized sub-agents (BA, UI/UX, Developer, Tester, DevOps) that collaborate through phased execution to automate the entire software development lifecycle.

This document tracks the current implementation status and architecture.

---

## Architecture

```
User provides idea (any channel: Telegram / Discord / Slack / CLI)
    │
    ▼
┌───────────────────────────────────────────────┐
│  Orchestrator (CEO)            │
│  • Receives idea, analyzes via LLM            │
│  • Creates project plan                       │
│  • Spawns specialized sub-agents dynamically  │
│  • Reports progress to user                   │
│                                               │
│  Phase 0: Intent Analysis (LLM-powered)       │
│  └── BA Agent analyzes complexity → decides   │
│      which agents to spawn (dynamic)          │
│                                               │
│  Phase 1: Analysis                            │
│  └── BA Agent → PRD                           │
│                                               │
│  Phase 2: Parallel Build                      │
│  ├── UI/UX Agent (if needed) → Design Spec    │
│  ├── Developer → Source Code                  │
│  └── Tester → Test Cases                      │
│                                               │
│  Phase 3: Integration Loop                    │
│  └── Developer ↔ Tester (until tests pass)    │
│                                               │
│  Phase 4: Deployment                          │
│  └── DevOps → GitHub Push                     │
└───────────────────────────────────────────────┘
    │
    ▼
Local Process Sandbox
```

---

## Implementation Status: Complete

### Core Components

| Component | File | Description | Status |
|-----------|------|-------------|--------|
| **Workflow Engine** | `src/factory/workflow.rs` | Phased execution with dynamic agent spawning | Complete |
| **Blackboard** | `src/factory/blackboard.rs` | Shared artifact storage (PRD, Code, Tests) | Complete |
| **Agent Roles** | `src/factory/roles.rs` | 6 roles with system prompts & tool allowlists | Complete |
| **Progress Streaming** | `src/factory/progress.rs` | Real-time progress events | Complete |
| **Learning Engine** | `src/factory/learning.rs` | Cross-project pattern learning | Complete |
| **Sub-Agent Spawning** | `src/factory/subagent.rs` | Hierarchical agent spawning | Complete |
| **Orchestrator Tool** | `src/factory/orchestrator_tool.rs` | `factory_build` tool | Complete |

### Configuration

```toml
[factory]
enabled = true                 # Default: true. Agent autonomously decides when to use factory
max_ping_pong_iterations = 5

# Feature flags
enable_streaming = true        # Real-time progress updates
enable_learning = false        # Learn from past builds
enable_sub_agents = false      # Spawn sub-agents for complex tasks

# Per-role provider overrides (optional)
[factory.provider_overrides.developer]
provider = "anthropic"
model = "claude-sonnet-4-6"
temperature = 0.3
```

---

## Key Features

### 1. Dynamic Agent Spawning (LLM-Powered)

No more hardcoded logic! The Business Analyst Agent uses LLM to analyze:

```rust
// BA Agent analyzes intent via LLM
let analysis = ba_agent.chat(
    "Analyze this request and decide which agents are needed..."
);

// Parse results to decide
let spawn_ui_ux = analysis.needs_frontend();
let spawn_devops = analysis.needs_backend();
```

**Rules determined by LLM:**
- Simple: Developer only (script, small tool)
- Medium: BA + Dev + Tester (API, backend)
- Complex: Full team + UI/UX (web app, mobile)

### 2. Progress Streaming

Real-time visibility into each phase:

```rust
pub enum FactoryProgressEvent {
    PhaseStarted { phase, message },
    AgentSpawned { role, agent_id },
    AgentCompleted { role, duration, status },
    ArtifactPublished { artifact, by },
    DevTesterIteration { iteration, max, status },
    DeploymentCompleted { target, url },
}
```

### 3. Cross-Phase Memory Sharing

Agents receive context from previous phases:

- **Phase 2 Developer**: Receives PRD + Design Spec
- **Phase 3 Developer**: Receives PRD + Code + Test Cases + Test Results
- **Phase 3 Tester**: Receives PRD + Code + Test Cases

### 4. Learning Engine (Optional)

Save and reuse patterns from successful builds:

```rust
pub trait LearningEngine {
    async fn store_pattern(&self, pattern: BuildPattern);
    async fn find_similar(&self, project_type: &str) -> Vec<SimilarPattern>;
    async fn get_best_practices(&self, project_type: &str) -> Vec<BestPractice>;
}
```

### 5. Sub-Agent Spawning (Optional)

Developer can spawn specialists for complex tasks:

```rust
pub enum ComplexTask {
    FrontendDevelopment { specs },
    BackendDevelopment { api_specs },
    SecurityReview { code },
    PerformanceOptimization { metrics },
}

let sub_agent = dev.spawn_sub_agent(ComplexTask::SecurityReview { ... })?;
```

---

## Agent Roles

| Role | Agentic | Tools | Responsibility |
|------|---------|-------|----------------|
| **Orchestrator** | No | `factory_build` | Coordinates workflow |
| **Business Analyst** | No | None | Analyzes + writes PRD + decides which agents |
| **UI/UX Designer** | No | None | Design spec (dynamically spawned) |
| **Developer** | Yes | `sandbox_*`, `github_read_repo` | Writes code |
| **Tester** | Yes | `sandbox_*` | Writes & runs tests |
| **DevOps** | Yes | `sandbox_*`, `github_push` | Deploy (dynamically spawned) |

---

## Sandbox Tools (10 tools)

| Tool | Purpose |
|------|---------|
| `sandbox_create` | Create/resume sandbox |
| `sandbox_run_command` | Run shell commands |
| `sandbox_write_file` | Write files |
| `sandbox_read_file` | Read files |
| `sandbox_list_files` | List directory |
| `sandbox_get_preview_url` | Get localhost URL |
| `sandbox_get_public_url` | Cloudflare tunnel |
| `sandbox_save_snapshot` | Persist to SQLite |
| `sandbox_restore_snapshot` | Restore from SQLite |
| `sandbox_kill` | Kill sandbox |

---

## Store Layer (SQLite)

| Table | Purpose |
|-------|---------|
| `sandbox_session` | Track active sandbox ID |
| `snapshots` | Persist project files |
| `tokens` | GitHub OAuth tokens |

---

## Workflow Execution Flow

```
1. User calls factory_build(idea)
   
2. Phase 0: Intent Analysis
   └── BA Agent + LLM analyzes
       ├── Decides complexity
       ├── Decides to spawn UI/UX?
       └── Decides to spawn DevOps?

3. Phase 1: Analysis  
   └── BA Agent → PRD (publish to Blackboard)

4. Phase 2: Parallel Build
   ├── UI/UX Agent (if needed) → DesignSpec
   ├── Dev Agent → SourceCode
   └── Tester Agent → TestCases

5. Phase 3: Integration Loop
   ├── Tester runs tests
   ├── If pass → go to Phase 4
   └── If fail → Dev fix → repeat (max 5)

6. Phase 4: Deployment
   └── DevOps Agent → GitHub Push

7. Return summary + URLs
```

---

## Design Principles

### 1. Agent-Driven Decisions

All important decisions are made by **LLM/Agent**:

- Intent classification → BA Agent + LLM
- Tool selection → Agent decides in loop
- Test strategy → Tester Agent decides

### 2. Zero Hardcoded Logic

No hardcoded keywords, rules, or thresholds. Wrong example:

```rust
// WRONG - Removed
if input.contains("web app") { spawn_ui_ux = true; }
```

Đúng:

```rust
// CORRECT - Use LLM
let decision = llm.chat("Should we spawn UI/UX for this request?");
```

### 3. Opt-in Features

All advanced features are opt-in:

```toml
[factory]
enabled = true               # Required
enable_learning = false      # Optional
enable_sub_agents = false    # Optional
```

### 4. Backward Compatible

Runs stable by default, no external services required.

---

## Module Structure

```
src/factory/
├── mod.rs                  # Exports
├── workflow.rs             # Core workflow engine
├── blackboard.rs           # Shared state
├── roles.rs                # Agent roles & prompts
├── progress.rs             # Streaming events
├── learning.rs             # Pattern learning
├── subagent.rs             # Hierarchical spawning
└── orchestrator_tool.rs    # factory_build tool
```

---

## Testing

```bash
# Run factory module tests
cargo test factory

# Run specific component
cargo test workflow
cargo test progress
cargo test learning
```

---

## Future Enhancements

| Feature | Status | Notes |
|---------|--------|-------|
| Vector DB integration | Planned | For semantic search in learning |
| Streaming UI | Planned | WebSocket/SSE frontend |
| Agent performance metrics | Planned | Track token usage, time |
| Custom role definitions | Planned | User-defined agents |

---

## Quick Reference

**Enable factory:**
```toml
[factory]
enabled = true
```

**Use in chat:**
```
Build me a REST API for task management
```

**Expected output:**
```
🏭 Factory workflow started
📋 Phase: Analysis - Business Analyst creating PRD
👤 Spawned Developer, Tester
✓ Phase Analysis completed
🔨 Phase: Parallel Build
...
✅ Factory build completed!
```

---

## Related Docs

- [ARCHITECTURE.md](../ARCHITECTURE.md) — System architecture
- [AGENTS.md](../AGENTS.md) — Agent protocol
- [config-reference.md](./config-reference.md) — Configuration options
