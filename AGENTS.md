# AGENTS.md — ZeroBuild: Autonomous Software Factory Protocol

> **ZeroBuild lets you build and host your own autonomous software factory.** Through a hierarchical multi-agent system, it automates the entire software development lifecycle — from idea to production-ready code. Built on [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw), the Rust-first autonomous agent runtime.

This file defines the default working protocol for coding agents in this repository.
Scope: entire repository (Rust runtime only — Node.js backend removed).

---

## 1) Project Snapshot (Read First)

**ZeroBuild** is a hierarchical multi-agent system (Autonomous Software Factory) built on ZeroClaw:

- **Orchestrator (CEO)** — Receives user ideas, analyzes feasibility, creates project plans, spawns specialized sub-agents, and coordinates the entire SDLC.
- **Specialized Sub-Agents** — BA (requirements), UI/UX (design), Developer (implementation), Tester (validation), DevOps (deployment) — each with dedicated contexts, permissions, and tools.
- **Single-agent mode** (default) — One agent handles conversation, planning, coding, and deployment for simpler tasks.
- **Factory mode** (enabled by default) — The Orchestrator spawns the full AI team for complex, multi-phase projects. The agent autonomously decides when to use `factory_build` based on task complexity.

ZeroClaw (the upstream base) is a Rust-first autonomous agent runtime optimized for performance, efficiency, stability, extensibility, sustainability, and security. ZeroBuild adds the multi-agent factory and project-building product layer on top.

**Project types the agent can build (non-exhaustive):**
- Web apps / websites (Next.js, React, etc.) — have a dev server → get a preview URL
- APIs / backend services (Node.js, Python, etc.) — no browser preview; use port/log output
- CLI tools, scripts, libraries — no preview URL; output is build artifacts
- Any project where the user's runtime environment provides the necessary toolchain

**Preview URL rules:**
- `sandbox_get_preview_url` and `sandbox_get_public_url` are only meaningful for projects that run an HTTP server on a port.
- For non-web projects, skip the URL step entirely.

### Core ZeroBuild extension points (unchanged)

- `src/providers/traits.rs` (`Provider`)
- `src/channels/traits.rs` (`Channel`)
- `src/tools/traits.rs` (`Tool`)
- `src/memory/traits.rs` (`Memory`)
- `src/observability/traits.rs` (`Observer`)
- `src/runtime/traits.rs` (`RuntimeAdapter`)

### ZeroBuild-specific extension points

- `src/tools/sandbox/` — local process sandbox tools (10 tools: create, run, write, read, list, preview, public-url, snapshot, restore, kill)
- `src/tools/deploy.rs` — `request_deploy` tool (push to GitHub via REST API)
- `src/tools/github_ops.rs` — GitHub ops tools (issue, PR, review, connect)
- `src/gateway/oauth.rs` — GitHub OAuth flow (`/auth/github`, `/auth/github/callback`)
- `src/store/` — SQLite persistence (sandbox session, project snapshot, GitHub token)
- `src/factory/` — multi-agent factory (roles, blackboard, workflow, orchestrator tool)

---

## 2) Architecture and Key Decisions

### Hierarchical multi-agent architecture

```
User provides idea (any channel: Telegram, Discord, Slack, CLI)
    │
    ▼
┌───────────────────────────────────────────────┐
│  Orchestrator (CEO)                           │
│  • Receives idea, analyzes feasibility        │
│  • Creates project plan, spawns sub-agents    │
│  • Coordinates phased execution               │
│  • Reports progress to user                   │
│                                               │
│  ┌─────────────────────────────────────────┐  │
│  │  Phase 1: BA Agent → PRD                │  │
│  │  Phase 2: UI/UX + Dev + Tester (parallel)│ │
│  │  Phase 3: Dev ◄─► Tester (fix loop)     │  │
│  │  Phase 4: DevOps → Deploy               │  │
│  └─────────────────────────────────────────┘  │
│                                               │
│  Each sub-agent has:                          │
│  • Dedicated system prompt & context          │
│  • Scoped tool permissions                    │
│  • Configurable provider/model                │
└───────────────────────────────────────────────┘
    │
    ▼
Local Process Sandbox      ← Isolated build sandbox
  • $TMPDIR/zerobuild-sandbox-{uuid}/
  • Any toolchain available from host PATH (node, python, cargo, etc.)
  • All agents share the same sandbox filesystem
  • Web projects: HTTP server on a port → preview URL available
  • Non-web projects: no preview URL; output via stdout/artifacts
```

### Why this architecture

1. **Autonomous Software Factory**: Mirrors a real dev team — Orchestrator delegates to specialists, each owns their domain.
2. **Autonomous SDLC**: The full lifecycle (requirements → design → code → test → deploy) runs without human intervention at technical steps.
3. **Self-healing loops**: Dev-Tester ping-pong with hard iteration cap prevents infinite loops while ensuring quality.
4. **Security boundary preserved**: OAuth tokens stored in SQLite only, never in logs or agent messages. Sandbox uses `env_clear()`.
5. **Re-hydration pattern**: SQLite snapshots allow session restoration across builds.
6. **Direct GitHub push**: `request_deploy` uses git blobs/tree/commit/ref API — no intermediate service needed.

### Identity boundary

- **User-facing name**: `ZeroBuild` — users interact with ZeroBuild as their "AI software company"
- **Runtime engine**: ZeroClaw — internal name, never shown to users
- **`IDENTITY.md`**: loaded by the Orchestrator to enforce this boundary

---

## 3) Engineering Principles (Normative)

Inherited from ZeroBuild — mandatory. These are implementation constraints, not slogans.

### 3.1 KISS — Keep It Simple, Stupid

- Prefer straightforward control flow over clever meta-programming.
- Prefer explicit match branches and typed structs over hidden dynamic behavior.
- Keep error paths obvious and localized.

### 3.2 YAGNI — You Aren't Gonna Need It

- Do not add new config keys, trait methods, feature flags, or workflow branches without a concrete accepted use case.
- Do not introduce speculative "future-proof" abstractions without at least one current caller.
- Keep unsupported paths explicit (error out) rather than adding partial fake support.

### 3.3 DRY + Rule of Three

- Duplicate small, local logic when it preserves clarity.
- Extract shared utilities only after repeated, stable patterns (rule-of-three).

### 3.4 SRP + ISP

- Keep each module focused on one concern.
- Extend behavior by implementing existing narrow traits whenever possible.

### 3.5 Fail Fast + Explicit Errors

- Prefer explicit `bail!`/errors for unsupported or unsafe states.
- Never silently broaden permissions/capabilities.

### 3.6 Secure by Default + Least Privilege

- Deny-by-default for access and exposure boundaries.
- Never log secrets, raw tokens, or sensitive payloads.
- Sandbox uses `env_clear()` — host credentials are never visible to child processes.
- OAuth tokens stored in SQLite only, never passed through channel messages or logs.

### 3.7 Determinism + Reproducibility

- Prefer reproducible commands and locked dependency behavior.
- Keep tests deterministic.

### 3.8 Reversibility + Rollback-First Thinking

- Keep changes easy to revert (small scope, clear blast radius).
- For risky changes, define rollback path before merge.

---

## 4) Repository Map

### Rust (ZeroBuild Agent)

- `src/main.rs` — CLI entrypoint
- `src/agent/` — orchestration loop
- `src/providers/` — LLM providers
- `src/tools/sandbox/` — local process sandbox tools (10 tools)
- `src/tools/deploy.rs` — request_deploy tool (GitHub REST API)
- `src/tools/github_ops.rs` — GitHub ops tools (direct GitHub API)
- `src/gateway/oauth.rs` — GitHub OAuth handlers
- `src/store/` — SQLite persistence layer
  - `src/store/mod.rs` — DB init (3 tables: sandbox_session, snapshots, tokens)
  - `src/store/session.rs` — sandbox_id tracking
  - `src/store/snapshot.rs` — project files persistence
  - `src/store/tokens.rs` — GitHub token storage
- `src/channels/` — channel implementations (Telegram, Discord, Slack, and others)
- `src/security/` — policy, pairing, secret store
- `src/config/` — schema + config loading
- `IDENTITY.md` — ZeroBuild user-facing persona definition

---

## 5) ZeroBuild-Specific Rules

### 5.1 Sandbox tool workflow

The ZeroBuild Agent uses these sandbox tools to build projects:

| Tool | Purpose |
|------|---------|
| `sandbox_create` | Create/resume local sandbox (reset=true to start fresh) |
| `github_read_repo` | Read all text files from an existing GitHub repo into sandbox (for bug-fix workflows) |
| `sandbox_run_command` | Run shell commands (npm, npx, node, cargo, python, etc.) IN SANDBOX |
| `sandbox_write_file` | Write file content to sandbox path |
| `sandbox_read_file` | Read file content from sandbox path |
| `sandbox_list_files` | List directory contents |
| `sandbox_get_preview_url` | Get localhost URL for a running HTTP server (web projects only) |
| `sandbox_get_public_url` | Start Cloudflare Quick Tunnel → public `https://xxx.trycloudflare.com` URL (web projects, VPS/remote only) |
| `sandbox_save_snapshot` | Extract files from sandbox to SQLite (persist project) |
| `sandbox_restore_snapshot` | Restore files from SQLite snapshot into sandbox (use when resuming after kill) |
| `sandbox_kill` | Kill sandbox and tunnel when done |

**⚠️ CRITICAL: Use `sandbox_run_command` for ALL build operations — NEVER use `shell` tool!**
- `shell` runs LOCALLY in workspace (not sandbox)
- `sandbox_run_command` runs in the isolated local sandbox

**Recommended build workflow (new project):**
1. `sandbox_create` (reset=true if user requests fresh start)
2. `sandbox_run_command` to scaffold the project (e.g. `npx create-next-app`, `cargo new`, `npm init`)
3. `sandbox_write_file` to create/edit files
4. `sandbox_read_file` / `sandbox_list_files` to inspect code
5. `sandbox_run_command` to build or start the project
6. **(Web projects only)** Auto-test: run `curl -s -o /dev/null -w "%{http_code}" http://localhost:{port}` to verify server responds 200
7. **(Web projects only)** URL step — choose based on deployment context:
   - **Local dev** (same machine): `sandbox_get_preview_url` (port=3000) → `http://localhost:{port}`
   - **VPS / remote server**: `sandbox_get_public_url` (port=3000) → `https://xxx.trycloudflare.com`
   - **Non-web projects**: skip this step
8. `sandbox_save_snapshot` to persist code to SQLite
9. Send result to user (URL for web projects, build output/artifacts for others)

**Edit workflow (resuming after sandbox was killed):**
1. `sandbox_create` (reset=false — creates fresh sandbox)
2. `sandbox_restore_snapshot` (workdir="project") — writes all files back from SQLite
3. `sandbox_run_command` to re-install deps (e.g. `cd project && npm install`)
4. Apply edits via `sandbox_write_file`
5. `sandbox_run_command` to restart server or rebuild
6. **(Web projects only)** Get new preview URL (step 7 above)
7. `sandbox_save_snapshot` to persist updated code

**Progress reporting (REQUIRED):**

Before every significant tool call, the agent MUST send a short, plain-language status message:

| Tool call | User message |
|---|---|
| `sandbox_create` | "Starting up the build environment..." |
| `sandbox_run_command { scaffold }` | "Creating your project..." |
| `sandbox_run_command { install }` | "Installing dependencies..." |
| `sandbox_run_command { build/start }` | "Building your project..." / "Starting the server..." |
| `sandbox_get_preview_url` | "Getting your preview link..." |
| `sandbox_get_public_url` | "Getting your public URL..." |
| `sandbox_restore_snapshot` | "Restoring your project files..." |
| `github_push` | "Pushing your code to GitHub..." |

**Rules:**
- Never paste raw shell/build output unless there is an error
- Keep messages short (one line)
- Use plain present-tense verbs ("Creating", "Installing", "Building")

### 5.2 Plan enforcement

The ZeroBuild Agent must always propose and confirm a plan before building. This is enforced at system-prompt level.

Never skip the plan step. Plan-before-build is a core product guarantee.

### 5.3 Sandbox workspace path

Agent workspace inside sandbox: `project/` (relative to sandbox root)

- The sandbox root is `$TMPDIR/zerobuild-sandbox-{uuid}/`.
- Project directory: `project/` inside sandbox root.
- All paths passed to sandbox tools are **relative to the sandbox root** — no leading `/` required.
- Build commands **must** be run from the `project/` workdir.
- **NEVER** use `/home/user/project` or any absolute path in tool arguments or inside shell commands — the local sandbox has no `/home/user/` directory. Use relative paths (e.g. `project/`) or `$HOME/project` which resolves to the sandbox root.
- ✅ Correct: `workdir: "project"`, command: `cd project && npm install`
- ❌ Wrong: `workdir: "/home/user/project"`, command: `cd /home/user/project && npm install`

### 5.4 Web project structure (Next.js)

When the project is a Next.js web app, maintain this layout:

```
project/                    ← Next.js project root (package.json here)
  app/                      ← App Router: ROUTES ONLY
    layout.tsx
    page.tsx
    globals.css
  components/               ← ALL reusable UI components
    Navbar.tsx
    Hero.tsx
    Footer.tsx
    ui/                     ← Primitive UI elements
    sections/               ← Page sections
  lib/                      ← Utilities, helpers, constants, types
  public/
```

File placement rules — no exceptions:

| File type | Correct location | Wrong location |
|---|---|---|
| Reusable component | `components/Hero.tsx` | `app/Hero.tsx` |
| Page section | `components/sections/HeroSection.tsx` | `app/HeroSection.tsx` |
| UI primitive | `components/ui/Button.tsx` | `app/Button.tsx` |
| Route/page | `app/about/page.tsx` | `components/about/page.tsx` |

### 5.5 Error fixing rules

When a build fails:

**CORRECT procedure:**
1. Read the exact error message
2. Identify the specific file
3. `sandbox_read_file` that file
4. `sandbox_write_file` only that file with the targeted fix
5. Re-run the build command via `sandbox_run_command`

**FORBIDDEN:**
- `rm -rf` on any source directory (app/, components/, lib/, public/, src/)
- Writing empty content to entry-point files (`app/page.tsx`, `main.rs`, `index.py`, etc.)
- Re-running scaffold commands (`npx create-next-app`, `cargo new`, etc.) after the project is already created
- Deleting and recreating files from scratch when a targeted fix is possible

**Allowed rm targets (build artifacts only):** `node_modules`, `.next`, `target`, `.cache`, `dist`, `out`, `build`

### 5.6 GitHub OAuth deploy flow

1. ZeroBuild Agent calls `github_connect` tool (no args)
2. If GitHub not connected: tool returns full OAuth URL in `error` field — forward it exactly to the user
3. User clicks URL → GitHub OAuth → callback stores token in SQLite
4. User says "done" → agent retries the original operation
5. `github_push` reads token from SQLite, creates/updates repo via GitHub git trees API
6. Returns repo URL + branch + commit SHA to user

OAuth tokens stored in `src/store/tokens.rs` — never in logs or channel messages.

### 5.7 Hashtag Workflow Routing (Required)

When a user message contains one of these hashtags or trigger phrases, you MUST use the corresponding tool immediately:

| Hashtag / Trigger | Workflow | Primary tools | Do NOT use |
|---|---|---|---|
| `#issue` / `#issues` / `#bug` / "create issue" / "file issue" / "report bug" | Create GitHub issue | `github_create_issue` | `glob_search`, `file_read` |
| `#plan` / "plan issue" / "create detailed issue" / "issue with plan" | Create structured issue with implementation plan | `github_read_repo` → [plan] → `github_create_issue` | `github_create_issue` (alone) |
| `#comment` / "comment on issue" / "add comment" | Add comment to issue or PR | `github_comment_issue` or `github_comment_pr` | `file_write` |
| `#pr` / "create PR" / "open PR" / "submit PR" | Create PR | `github_create_pr` | `file_write`, `shell` |
| `#review` / "review PR code" / "code review" / "review this PR" | Deep code review with inline suggestions | `github_get_pr` → `github_get_pr_diff` → `github_read_file` → `github_post_inline_comments` | `file_write`, `shell` |
| `#summarize` / "summarize PR" / "what does this PR do" | PR summary/description (what changed) | `github_get_pr` → `github_get_pr_diff` | `github_post_inline_comments` |
| `#feature` / "new feature" / "feature request" | Create feature issue | `github_create_issue` + `github_push` | `task_plan` (alone) |
| `#deploy` / `#push` / "deploy" / "push to github" | Push code to GitHub | `github_push` | `sandbox_write_file` |
| `#build` / "build" / "compile" | Build in sandbox | Sandbox tool workflow (section 5.1) | `shell` (local) |
| `#repo` / "list repos" / "my repositories" | List repositories | `github_list_repos` | `http_request` |
| `#read` / `#file` / "read file from repo" | Read repo file | `github_read_file` | `file_read` (local) |

**CRITICAL RULES:**
1. When user says "create issue" → call `github_create_issue` (NOT `glob_search` or other tools)
2. When user says "create PR" → call `github_create_pr` (NOT `file_write` or other tools)
3. Before any GitHub operation, call `github_connect` first to verify authentication
4. **NEVER use `shell` tool for build commands** — use `sandbox_run_command` instead
   - `shell` runs LOCALLY (wrong place for builds)
   - `sandbox_run_command` runs in the isolated sandbox (correct place for builds)

**Extracting repo context from the message:**
1. Look for explicit `owner/repo` pattern in the message (e.g. `myorg/myapp`)
2. Fall back to `active_project.github_repo` in memory (if session resumption is active)
3. If both absent, ask: "Which repository should I use?"

**Branch context:**
- Default branch: `main` unless the user specifies otherwise
- For `#feature` workflow: create a branch named after the feature, e.g. `feature/add-login`
- For `#pr` workflow: ask for head + base branch if not stated

### 5.8 Default Workflow (No Hashtag)

When a user message has **no hashtag**, infer intent from content:

| Message content pattern | Inferred workflow |
|---|---|
| Describes a new project, app, tool, script, or service to build | Sandbox build workflow (section 5.1) |
| References an existing GitHub repo, issue number, or PR number | GitHub ops workflow — call the relevant tool |
| Contains a GitHub URL (github.com/...) | Parse context from URL → call the relevant tool |
| Asks a question about an existing project | Answer directly; do not start building |
| Ambiguous — cannot determine intent | Ask ONE clarifying question: "Do you want me to build something new, or work on an existing project?" |

**Do not ask multiple clarifying questions.** One question, wait for answer, then proceed.

### 5.9 Config fields

ZeroBuild-specific fields in `ZerobuildConfig`:

| Field | Default | Purpose |
|-------|---------|---------|
| `github_client_id` | `""` | GitHub OAuth app client ID |
| `github_client_secret` | `""` | GitHub OAuth app client secret |
| `github_oauth_proxy` | official proxy URL | OAuth proxy for GitHub connector |
| `db_path` | `"~/.zerobuild/zerobuild.db"` | SQLite database path |

### 5.10 GitHub Ops Language and Content Rules (Required)

**ALL GitHub issues and pull requests MUST be written in English — no exceptions.**

This applies to:
- Issue title and body
- PR title and body
- Review comments
- Close/edit comments

Even if the user writes their request in another language, the agent MUST translate the content into English before calling any GitHub tool.

**Issue title format:**
Use a bracketed type prefix: `[Feature]: ...`, `[Bug]: ...`, `[Chore]: ...`, `[Docs]: ...`

**Before creating an issue or PR, verify:**
1. The target repo (`owner/repo`) exists and the user's token has write access — call `github_list_repos` or confirm with user if unsure
2. Labels exist in the repo — only use labels that exist, or omit the `labels` field entirely
3. Content is in English

**If GitHub API returns an error:**
- `403` / `404` → token does not have write access to that repo or the repo does not exist
- `422` → labels do not exist in the repo (remove labels and retry without them)
- `503` → transient GitHub error or org-level access control block — retry once, then report the error URL to the user

### 5.11 Auto-Invoke Product Advisor After Deploy

After every successful `github_push`, the agent MUST automatically call `product_advisor` with the active project context to generate improvement suggestions.

**Procedure:**
1. Push completes successfully
2. Agent calls `product_advisor` with:
   - `project_name`: from active project context
   - `description`: from active project context
   - `current_features`: derived from the built project
   - `focus`: "all" (default)
3. Agent presents suggestions to the user in this format:
   ```
   💡 IMPROVEMENT SUGGESTIONS — [Project Name]
   ═══════════════════════════════════════════

   🔴 HIGH PRIORITY:
      • [recommendation 1]
      • [recommendation 2]

   🟡 MEDIUM PRIORITY:
      • [recommendation 3]

   🔵 LONG-TERM:
      • [recommendation 4]

   Which improvement would you like to start with?
   ```

This closes the loop — every completed deploy ends with actionable next steps.

### 5.12 Error Recovery and Failure Escalation

**Error classification:** When a tool fails, classify the error from `ToolResult`:

| Category | Detection pattern |
|---|---|
| Dependency error | contains `"not found"`, `"cannot find module"`, `"missing"` |
| Build error | contains `"SyntaxError"`, `"TypeError"`, `"compilation failed"` |
| Runtime error | contains `"ECONNREFUSED"`, `"port already in use"`, `"SIGKILL"` |
| Config error | contains `"invalid config"`, `"missing env"` |

**Consecutive failure escalation:** Track consecutive failures per tool in the agent loop. After 3 consecutive failures on the same tool, inject a clarification prompt:

> "I'm having trouble with this step. Would you like me to try a different approach?"

This prevents silent infinite retry loops and gives users a way to intervene.

### 5.13 Bug Fix Workflow (existing repo)

Use this workflow when the user asks to fix a bug in an existing GitHub repository.

**Required tools in order:**

1. `github_connect` — confirm GitHub authentication
2. `sandbox_create` (reset=false — clean sandbox, no existing files)
3. `github_read_repo` (owner, repo, branch="main", workdir="project") — fetch all repo files into sandbox
4. `sandbox_read_file` / `sandbox_list_files` — inspect the file(s) related to the bug
5. `sandbox_write_file` — apply the fix
6. `sandbox_run_command` — verify the fix (build, tests, lint)
7. `sandbox_save_snapshot` — persist the fixed state
8. `github_push` (branch="fix/<short-description>") — push to a new branch
9. `github_create_pr` — open a PR describing the bug and fix

**Progress messages (REQUIRED):**

| Tool call | User message |
|---|---|
| `github_read_repo` | "Reading repository files..." |
| `sandbox_run_command { verify }` | "Verifying the fix..." |
| `github_push` | "Pushing fix branch to GitHub..." |
| `github_create_pr` | "Opening pull request..." |

**Rules:**
- Always push to a new branch (never directly to main/master).
- PR title must follow `[Bug]: <description>` format per section 5.10.
- If `github_read_repo` skips more files than it writes, warn the user and confirm the fix scope before pushing.

---

## 6) Multi-Agent Factory Workflow

ZeroBuild supports a **factory mode** where the Orchestrator spawns specialized AI agents that collaborate through phased execution. See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design.

### 6.1 Agent Roles and Responsibilities

| Role | Module | Responsibility |
|------|--------|----------------|
| **Orchestrator** | `src/factory/orchestrator_tool.rs` | Workflow coordination, phase management, user communication |
| **Business Analyst** | `src/factory/roles.rs` | Requirements analysis, PRD generation |
| **UI/UX Designer** | `src/factory/roles.rs` | Design specifications, component structure |
| **Developer** | `src/factory/roles.rs` | Code generation using sandbox tools |
| **Tester** | `src/factory/roles.rs` | Test case generation and execution |
| **DevOps** | `src/factory/roles.rs` | Deployment configuration, GitHub push |

### 6.2 Workflow Phases

1. **Analysis** (sequential) — BA agent produces PRD from user idea
2. **Parallel Build** — UI/UX + Developer + Tester run concurrently via `tokio::join!`
3. **Integration Loop** — Developer-Tester ping-pong until tests pass (max iterations configurable, default 5)
4. **Deployment** — DevOps agent deploys when tests pass

### 6.3 Blackboard Protocol

Agents communicate through a shared `Blackboard` (typed `Arc<Mutex<HashMap>>`) with versioned artifact entries:

| Artifact | Producer | Consumers |
|----------|----------|-----------|
| `Prd` | Business Analyst | UI/UX, Developer, Tester |
| `DesignSpec` | UI/UX Designer | Developer |
| `SourceCode` | Developer | Tester, DevOps |
| `TestCases` | Tester | Developer (integration loop) |
| `TestResults` | Tester | Orchestrator (phase control) |
| `DeployConfig` | DevOps | Orchestrator (final result) |

### 6.4 Factory Configuration

```toml
[factory]
enabled = true                     # Default: true. Agent autonomously decides when to use factory
max_ping_pong_iterations = 5       # Dev-Tester loop cap

# Per-role provider/model overrides (optional)
[factory.provider_overrides.developer]
provider = "anthropic"
model = "claude-sonnet-4-6"
temperature = 0.3

[factory.provider_overrides.tester]
provider = "openrouter"
model = "anthropic/claude-sonnet-4-6"
```

When `factory.enabled = true`, the `factory_build` tool is registered and available to the agent.

### 6.5 Extension Points

- New roles: add variant to `AgentRole` enum in `src/factory/roles.rs`, define system prompt
- New artifacts: add variant to `Artifact` enum in `src/factory/blackboard.rs`
- Phase customization: modify `FactoryWorkflow::run()` in `src/factory/workflow.rs`

---

## 7) Risk Tiers by Path

- **Low risk**: docs, test changes
- **Medium risk**: `src/tools/sandbox/`, `src/store/`, `src/factory/`, most `src/**` Rust changes
- **High risk**: `src/security/**`, `src/runtime/**`, `src/gateway/**`, `src/tools/deploy.rs`, `src/tools/github_ops.rs`, `src/gateway/oauth.rs`, `.github/workflows/**`, access-control boundaries

---

## 8) Agent Workflow (Required)

1. **Read before write** — inspect existing module and adjacent tests before editing.
2. **Define scope boundary** — one concern per PR; avoid mixed feature+refactor+infra patches.
3. **Implement minimal patch** — apply KISS/YAGNI/DRY rule-of-three.
4. **Validate by risk tier** — docs-only: lightweight; code/risky: full checks.
5. **Document impact** — update docs/PR notes for behavior, risk, side effects, rollback.
6. **Respect queue hygiene** — declare `Depends on #...` for stacked PRs.

### 8.1 Branch / Commit / PR Flow (Required)

- Create and work from a non-`main` branch.
- Commit changes to that branch with clear, scoped commit messages.
- Open a PR to `main`; do not push directly to `main`.
- Wait for required checks and review outcomes before merging.

### 8.2 Code Naming Contract (Required)

- Rust: modules/files `snake_case`, types/traits `PascalCase`, functions/variables `snake_case`, constants `SCREAMING_SNAKE_CASE`.
- Test identifiers: use project-scoped neutral labels (`zerobuild_user`, `zerobuild_node`).

### 8.3 Architecture Boundary Contract (Required)

- Sandbox runs as a local process with `env_clear()` — no host credentials leak into builds.
- OAuth tokens must never appear in logs, channel messages, or agent tool results.
- GitHub API calls must use token loaded from `src/store/tokens.rs` — never hardcoded.

---

## 9) Validation Matrix

### Rust (ZeroBuild Agent)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 10) Collaboration and PR Discipline

- Follow `.github/pull_request_template.md` fully.
- Keep PR descriptions concrete: problem, change, non-goals, risk, rollback.
- Use conventional commit titles.
- Prefer small PRs when possible.
- Agent-assisted PRs are welcome, but contributors remain accountable for understanding what their code will do.

### 10.1 Privacy/Sensitive Data (Required)

- Never commit API keys, bot tokens, OAuth secrets, or user IDs.
- Never log user messages, channel user IDs, prompt content, or OAuth tokens in production.
- Use neutral project-scoped placeholders in tests and examples.

---

## 11) Anti-Patterns (Do Not)

- Do not add heavy dependencies for minor convenience.
- Do not silently weaken security policy or access constraints.
- Do not add speculative config/feature flags "just in case".
- Do not mix formatting-only changes with functional changes.
- Do not modify unrelated modules "while here".
- Do not bypass failing checks without explicit explanation.
- Do not hide behavior-changing side effects in refactor commits.
- Do not include personal identity or sensitive information in any commit.
- **ZeroBuild-specific**: Do not skip plan confirmation before building.
- **ZeroBuild-specific**: Do not expose OAuth tokens in tool results or channel messages.
- **ZeroBuild-specific**: Do not allow the agent to delete source files or directories when fixing build errors.
- **ZeroBuild-specific**: Do not re-scaffold a project (e.g. `npx create-next-app`, `cargo new`) after it is already created.
- **ZeroBuild-specific**: Do not call preview URL tools (`sandbox_get_preview_url`, `sandbox_get_public_url`) for non-web projects.

---

## 12) Handoff Template (Agent → Agent / Maintainer)

When handing off work, include:

1. What changed
2. What did not change
3. Validation run and results
4. Remaining risks / unknowns
5. Next recommended action

### 5.14 PR Code Review Workflow

Use this workflow when the user asks to **review code** in a pull request. This is NOT a summary - it is a deep code review AI that analyzes code quality, patterns, and suggests improvements.

**Difference from Summary:**
- **PR Summary** = What changed (high-level description)
- **Code Review** = Deep analysis of HOW the code was written + suggestions for improvement

**Required tools in order:**

1. `github_connect` → verify GitHub authentication first
2. `github_get_pr` → obtain `head.sha` (commit_id), title, PR metadata
3. `github_get_pr_diff` → read the file-by-file diff (filename, status, patch text)
4. **CRITICAL: Read full source files** that have changes → use `github_read_file` to get context around the diff (not just the patch!)
5. [Agent analyzes - see Analysis Checklist below]
6. `github_post_inline_comments` → post detailed review with inline comments

**Analysis Checklist:**

For each changed file, analyze:

1. **Code Quality Issues**
   - Unused imports/variables
   - Missing error handling
   - Hardcoded values that should be configurable
   - Code duplication (DRY violations)
   - Overly complex functions that need refactoring

2. **Logic & Correctness**
   - Potential bugs or edge cases
   - Race conditions
   - Off-by-one errors
   - Incorrect error propagation
   - Missing validation/sanitization

3. **Performance & Efficiency**
   - Unnecessary allocations
   - Inefficient algorithms (O(n²) when could be O(n))
   - Missing caching opportunities
   - Blocking operations in async contexts
   - String concatenation in loops

4. **Security**
   - SQL injection risks
   - XSS vulnerabilities
   - Hardcoded secrets/tokens
   - Unsafe deserialization
   - Path traversal risks

5. **Maintainability**
   - Functions too long (>50 lines)
   - Missing documentation for public APIs
   - Inconsistent naming conventions
   - Magic numbers without constants
   - Deep nesting that needs early returns

6. **Idiomatic Patterns**
   - Language-specific best practices
   - Common anti-patterns
   - Better standard library usage
   - Proper error types vs strings

**Review Comment Format:**

```
🔴 **Issue**: [Brief description of the problem]

**Why**: [Explanation of why this is a problem]

**Suggestion**: 
```rust
// Show the improved code here
```

**Alternative**: [If there's more than one way to fix it]
```

**Rules:**
- **ALWAYS** read the full source file via `github_read_file`, not just the diff patch
- Comment on specific lines that were ADDED or MODIFIED (not deleted lines)
- Use `event = REQUEST_CHANGES` if there are issues to fix; `COMMENT` for minor observations; `APPROVE` only if code is clean
- Skip binary files and files without a `patch` field
- `commit_id` MUST be from `github_get_pr` (`head.sha`)
- Maximum 20 inline comments per review (prioritize critical issues)
- Focus on **actionable** suggestions - don't just point out problems, suggest the fix

**Example Review Flow:**
```
User: "review PR #4"
→ "Fetching PR metadata..."
→ github_get_pr → get commit_id
→ "Reading PR diff..."
→ github_get_pr_diff → see files changed
→ "Analyzing source code..."
→ github_read_file for each changed file → get full context
→ [checklist above]
→ "Posting review comments..."
→ github_post_inline_comments with detailed suggestions
```

**Progress messages (REQUIRED):**

| Step | User message |
|---|---|
| `github_get_pr` | "Fetching PR metadata..." |
| `github_get_pr_diff` | "Reading PR diff..." |
| `github_read_file` | "Reading source files for context..." |
| Analysis | "Analyzing code quality..." |
| `github_post_inline_comments` | "Posting review comments..." |

---

### 5.15 Issue Planner Workflow

Use this workflow when the user wants to create a **well-planned, actionable issue** - Issue Planner. Instead of just a basic description, the issue should include implementation planning, task breakdown, and considerations.

**Difference from Basic Issue:**
- **Basic Issue** = What needs to be done (simple description)
- **Issue Planner** = What + How + Task breakdown + Considerations + Acceptance criteria

**Required tools in order:**

1. `github_connect` → verify GitHub authentication
2. **Context gathering (if needed):**
   - `github_read_repo` → read codebase to understand current implementation
   - `github_list_files` → explore project structure
   - `glob_search` / `file_read` → find relevant code patterns
3. [Agent analyzes and plans - see Planning Checklist below]
4. `github_create_issue` → create structured issue with plan

**Issue Planner Analysis Checklist:**

Structure the issue with these sections:

1. **Overview** (Executive Summary)
   - One-paragraph description of the problem/feature
   - Why this matters (business/technical value)
   - Priority level (P0/P1/P2)

2. **Background & Context**
   - Current state of the system
   - Related previous issues/PRs (if known)
   - Technical constraints or dependencies

3. **Proposed Solution(s)**
   - Option 1: Recommended approach
   - Option 2: Alternative approach (if applicable)
   - Pros/cons of each option

4. **Implementation Plan** (Task Breakdown)
   ```markdown
   ### Phase 1: Preparation
   - [ ] Research current implementation
   - [ ] Define interfaces/APIs
   - [ ] Write test cases

   ### Phase 2: Core Implementation
   - [ ] Implement feature/fix
   - [ ] Add tests
   - [ ] Update documentation

   ### Phase 3: Validation
   - [ ] Run test suite
   - [ ] Performance testing (if applicable)
   - [ ] Code review
   ```

5. **Technical Considerations**
   - Potential risks and mitigations
   - Breaking changes (if any)
   - Migration strategy
   - Performance implications
   - Security considerations

6. **Acceptance Criteria**
   - Specific, testable conditions for completion
   - Definition of "done"
   - Edge cases to handle

7. **References & Resources**
   - Related documentation
   - External links/specs
   - Similar implementations elsewhere

**Issue Format Template:**

```markdown
## 🎯 Overview
[Brief description of what needs to be done and why]

## 📋 Background
[Current state, context, and any relevant history]

## 💡 Proposed Solution

### Recommended Approach
[Primary solution with technical details]

**Pros:**
- [Benefit 1]
- [Benefit 2]

**Cons:**
- [Trade-off 1]
- [Trade-off 2]

### Alternative Approach (Optional)
[Alternative if primary isn't feasible]

## 📊 Implementation Plan

### Phase 1: [Name]
- [ ] Task 1
- [ ] Task 2
- [ ] Task 3

### Phase 2: [Name]
- [ ] Task 1
- [ ] Task 2

## ⚠️ Technical Considerations
- **Risk:** [Risk description] → **Mitigation:** [How to handle]
- **Breaking Change:** [Description] → **Migration:** [Steps]
- **Performance:** [Impact and monitoring]

## ✅ Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## 📚 References
- [Link 1]
- [Link 2]
```

**Special Issue Types:**

**Bug Issue Planner:**
- Add "Reproduction Steps" section
- Add "Root Cause Analysis" section
- Include "Environment" (versions, OS, etc.)
- Stack traces or error logs

**Feature Issue Planner:**
- Add "User Stories" section
- Include "UI/UX Mockups" description (if applicable)
- API design considerations
- Backward compatibility plan

**Refactor Issue Planner:**
- Current architecture diagram/description
- Target architecture
- Migration strategy
- Risk assessment for regression

**Progress Messages:**

| Step | User message |
|---|---|
| Context gathering | "Analyzing codebase for context..." |
| Planning | "Creating implementation plan..." |
| `github_create_issue` | "Creating structured issue..." |

**Example Flow:**
```
User: "plan issue for adding OAuth authentication"
→ "Analyzing codebase for context..."
→ github_read_repo / file_read → understand auth system
→ "Creating implementation plan..."
→ [Generate structured issue with phases, tasks, considerations]
→ "Creating structured issue..."
→ github_create_issue with full plan
```

---

### 5.16 GitHub Comment Workflow

Use these tools to add comments to existing issues and PRs.

**Available Tools:**

| Tool | Purpose | Use When |
|---|---|---|
| `github_comment_issue` | Add comment to an issue | User wants to comment on issue #N |
| `github_comment_pr` | Add general comment to PR | User wants to comment on PR (not inline review) |
| `github_reply_comment` | Reply to existing comment | User wants to reply to a specific comment |

**Difference from Review:**
- **PR Review** (`github_post_inline_comments`) = Inline code comments on specific lines
- **PR Comment** (`github_comment_pr`) = General comment on the PR (like "LGTM" or questions)

**Required tools in order:**

1. `github_connect` → verify GitHub authentication
2. `github_comment_issue` / `github_comment_pr` / `github_reply_comment` → post comment

**Parameters:**

```json
// github_comment_issue
type: "object",
properties: {
    "repo": "Repository name",
    "owner": "Repository owner (optional, defaults to auth user)",
    "issue_number": "Issue number",
    "body": "Comment text (Markdown supported)"
}

// github_comment_pr
type: "object",
properties: {
    "repo": "Repository name",
    "owner": "Repository owner (optional)",
    "pr_number": "PR number",
    "body": "Comment text (Markdown supported)"
}

// github_reply_comment
type: "object",
properties: {
    "repo": "Repository name",
    "owner": "Repository owner (optional)",
    "comment_id": "ID of comment to reply to",
    "body": "Reply text (Markdown supported)"
}
```

**Progress Messages:**

| Step | User message |
|---|---|
| `github_comment_issue` | "Adding comment to issue..." |
| `github_comment_pr` | "Adding comment to PR..." |
| `github_reply_comment` | "Replying to comment..." |

**Example Flows:**

```
User: "comment on issue #42 in my-app: 'I can reproduce this'"
→ github_connect
→ github_comment_issue(repo="my-app", issue_number=42, body="I can reproduce this")
→ "Comment added to issue #42"

User: "comment on PR #5: 'Please add tests'"
→ github_connect
→ github_comment_pr(repo="my-app", pr_number=5, body="Please add tests")
→ "Comment added to PR #5"

User: "reply to comment #123456: 'Fixed in latest commit'"
→ github_connect
→ github_reply_comment(repo="my-app", comment_id=123456, body="Fixed in latest commit")
→ "Reply posted"
```

---

## 13) Vibe Coding Guardrails

When working in fast iterative mode:

- Keep each iteration reversible (small commits, clear rollback).
- Validate assumptions with code search before implementing.
- Prefer deterministic behavior over clever shortcuts.
- Do not "ship and hope" on security-sensitive paths.
- If uncertain, leave a concrete TODO with verification context, not a hidden guess.
