# AGENTS.md — ZeroBuild Agent Engineering Protocol

> **Forked from ZeroBuild.** ZeroBuild is a customization of the ZeroBuild agent runtime that builds web applications via Telegram using an isolated local process sandbox. This protocol extends ZeroBuild's base AGENTS.md with ZeroBuild-specific rules.

This file defines the default working protocol for coding agents in this repository.
Scope: entire repository (Rust runtime only — Node.js backend removed).

---

## 1) Project Snapshot (Read First)

**ZeroBuild** is a single-tier AI agent system built on ZeroBuild:

- **ZeroBuild Agent** — ZeroBuild Rust runtime. Handles Telegram conversations, proposes plans, writes code into an isolated local process sandbox, and pushes to GitHub.

ZeroBuild (the upstream base) is a Rust-first autonomous agent runtime optimized for performance, efficiency, stability, extensibility, sustainability, and security. ZeroBuild keeps all of that and adds a web-app-building product layer on top.

### Core ZeroBuild extension points (unchanged)

- `src/providers/traits.rs` (`Provider`)
- `src/channels/traits.rs` (`Channel`)
- `src/tools/traits.rs` (`Tool`)
- `src/memory/traits.rs` (`Memory`)
- `src/observability/traits.rs` (`Observer`)
- `src/runtime/traits.rs` (`RuntimeAdapter`)

### ZeroBuild-specific extension points

- `src/tools/sandbox/` — local process sandbox tools (8 tools: create, run, write, read, list, preview, snapshot, kill)
- `src/tools/deploy.rs` — `request_deploy` tool (push to GitHub via REST API)
- `src/tools/github_ops.rs` — GitHub ops tools (issue, PR, review, connect)
- `src/gateway/oauth.rs` — GitHub OAuth flow (`/auth/github`, `/auth/github/callback`)
- `src/store/` — SQLite persistence (sandbox session, project snapshot, GitHub token)

---

## 2) Architecture and Key Decisions

### Single-tier agent design

```
Telegram User
    │
    ▼
ZeroBuild Runtime (Rust)   ← ZeroBuild Agent
  • Runs the conversation loop
  • Proposes plans, waits for user confirmation
  • Calls sandbox_* tools directly (no external API needed)
  • Calls github_* tools → GitHub REST API directly
  • Calls request_deploy → GitHub git tree/commit/ref API
    │
    ▼
Local Process Sandbox      ← Isolated build sandbox
  • $TMPDIR/zerobuild-sandbox-{uuid}/
  • Node.js, npm, npx available from host PATH
  • scaffold → build → preview (http://localhost:{port})
```

### Why single-tier

1. **Simplicity**: No external sandbox service. The sandbox is a local process with a restricted environment. Fewer moving parts = easier to debug.
2. **Security boundary preserved**: OAuth tokens stored in SQLite only, never in logs or agent messages.
3. **Re-hydration pattern**: SQLite snapshots (`src/store/snapshot.rs`) allow future sessions to restore previous builds.
4. **Direct GitHub push**: `request_deploy` uses git blobs/tree/commit/ref API — no intermediate service needed.

### Identity boundary

- **User-facing name**: `ZeroBuild` — users interact with ZeroBuild via Telegram
- **Runtime engine**: ZeroBuild — internal name, never shown to users
- **`IDENTITY.md`**: loaded by the ZeroBuild Agent to enforce this boundary

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
- OAuth tokens stored in SQLite only, never passed through agent chat or logs.

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
- `src/tools/sandbox/` — local process sandbox tools (8 tools)
- `src/tools/deploy.rs` — request_deploy tool (GitHub REST API)
- `src/tools/github_ops.rs` — GitHub ops tools (direct GitHub API)
- `src/gateway/oauth.rs` — GitHub OAuth handlers
- `src/store/` — SQLite persistence layer
  - `src/store/mod.rs` — DB init (3 tables: sandbox_session, snapshots, tokens)
  - `src/store/session.rs` — sandbox_id tracking
  - `src/store/snapshot.rs` — project files persistence
  - `src/store/tokens.rs` — GitHub token storage
- `src/channels/` — Telegram/Discord/Slack channels
- `src/security/` — policy, pairing, secret store
- `src/config/` — schema + config loading
- `IDENTITY.md` — ZeroBuild user-facing persona definition

---

## 5) ZeroBuild-Specific Rules

### 5.1 Sandbox tool workflow

The ZeroBuild Agent uses these 8 sandbox tools to build web apps:

| Tool | Purpose |
|------|---------|
| `sandbox_create` | Create/resume local sandbox (reset=true to start fresh) |
| `sandbox_run_command` | Run shell commands (npm, npx, node, etc.) IN SANDBOX |
| `sandbox_write_file` | Write file content to sandbox path |
| `sandbox_read_file` | Read file content from sandbox path |
| `sandbox_list_files` | List directory contents |
| `sandbox_get_preview_url` | Get preview URL for running dev server (default port 3000) |
| `sandbox_save_snapshot` | Extract files from sandbox to SQLite (persist project) |
| `sandbox_kill_sandbox` | Kill sandbox when done |

**⚠️ CRITICAL: Use `sandbox_run_command` for ALL build operations — NEVER use `shell` tool!**
- `shell` runs LOCALLY in workspace (not sandbox)
- `sandbox_run_command` runs in the isolated local sandbox

**Recommended build workflow:**
1. `sandbox_create` (reset=true if user requests fresh start)
2. `sandbox_run_command` to scaffold (`npx create-next-app`, `npm install`)
3. `sandbox_write_file` to create/edit files
4. `sandbox_read_file` / `sandbox_list_files` to inspect code
5. `sandbox_run_command` to start dev server (`npm run dev &`)
6. **Auto-test:** Run `curl -s -o /dev/null -w "%{http_code}" http://localhost:3000` to verify the server responds with 200
7. `sandbox_get_preview_url` (port=3000) to get preview URL
8. `sandbox_save_snapshot` to persist code to SQLite
9. Send preview URL to user

**Progress reporting (REQUIRED):**

Before every significant tool call, the agent MUST send a short, plain-language status message:

| Tool call | User message |
|---|---|
| `sandbox_create` | "Starting up the build environment..." |
| `sandbox_run_command { npx create-next-app }` | "Creating your project..." |
| `sandbox_run_command { npm install }` | "Installing dependencies..." |
| `sandbox_run_command { npm run dev }` | "Starting the dev server..." |
| `sandbox_get_preview_url` | "Getting your preview link..." |
| `github_push` | "Pushing your code to GitHub..." |

**Rules:**
- Never paste raw shell/npm output unless there is an error
- Keep messages short (one line)
- Use plain present-tense verbs ("Creating", "Installing", "Starting")

### 5.2 Plan enforcement

The ZeroBuild Agent must always propose and confirm a plan before building. This is enforced at system-prompt level.

Never skip the plan step. Plan-before-build is a core product guarantee.

### 5.3 Sandbox workspace path

Agent workspace inside sandbox: `project/` (relative to sandbox root)

- The sandbox root is `$TMPDIR/zerobuild-sandbox-{uuid}/`.
- App directory: `project/` inside sandbox root (Next.js project root here).
- All paths passed to sandbox tools are relative to the sandbox root — no leading `/` required.
- `npm run build` **must** be run from the `project/` workdir.

### 5.4 Next.js project structure

The ZeroBuild Agent must maintain this layout:

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

When `npm run build` fails:

**CORRECT procedure:**
1. Read the exact error message
2. Identify the specific file
3. `sandbox_read_file` that file
4. `sandbox_write_file` only that file with the targeted fix
5. Re-run `npm run build` via `sandbox_run_command`

**FORBIDDEN:**
- `rm -rf` on any source directory (app/, components/, lib/, public/, src/)
- Writing empty content to `app/page.tsx` or `layout.tsx`
- Re-running `npx create-next-app` after scaffold
- Deleting and recreating components from scratch

**Allowed rm targets (build artifacts only):** `node_modules`, `.next`, `.cache`, `dist`, `out`, `build`

### 5.6 GitHub OAuth deploy flow

1. ZeroBuild Agent calls `github_connect` tool (no args)
2. If GitHub not connected: tool returns full OAuth URL in `error` field — forward it exactly to the user
3. User clicks URL → GitHub OAuth → callback stores token in SQLite
4. User says "done" → agent retries the original operation
5. `github_push` reads token from SQLite, creates/updates repo via GitHub git trees API
6. Returns repo URL + branch + commit SHA to user

OAuth tokens stored in `src/store/tokens.rs` — never in logs or Telegram messages.

### 5.7 Hashtag Workflow Routing (Required)

When a user message contains one of these hashtags or trigger phrases, you MUST use the corresponding tool immediately:

| Hashtag / Trigger | Workflow | Primary tools | Do NOT use |
|---|---|---|---|
| `#issue` / `#issues` / `#bug` / "create issue" / "file issue" / "report bug" | Create GitHub issue | `github_create_issue` | `glob_search`, `file_read` |
| `#pr` / `#review` / "create PR" / "open PR" / "submit PR" | Create or review PR | `github_create_pr`, `github_review_pr` | `file_write`, `shell` |
| `#feature` / "new feature" / "feature request" | Create feature issue | `github_create_issue` + `github_push` | `task_plan` (alone) |
| `#deploy` / `#push` / "deploy" / "push to github" | Push code to GitHub | `github_push` | `sandbox_write_file` |
| `#build` / "build" / "compile" | Build in sandbox | Sandbox tool workflow (section 5.1) | `shell` (local) |
| `#repo` / "list repos" / "my repositories" | List repositories | `github_list_repos` | `http_request` |
| `#read` / `#file` / "read file from repo" | Read repo file | `github_read_file` | `file_read` (local) |

**CRITICAL RULES:**
1. When user says "create issue" → call `github_create_issue` (NOT `glob_search` or other tools)
2. When user says "create PR" → call `github_create_pr` (NOT `file_write` or other tools)
3. Before any GitHub operation, call `github_connect` first to verify authentication
4. **NEVER use `shell` tool for npm/npx/node commands** — use `sandbox_run_command` instead
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
| Describes a new app, website, tool, or page to build | Sandbox build workflow (section 5.1) |
| References an existing GitHub repo, issue number, or PR number | GitHub ops workflow — call the relevant tool |
| Contains a GitHub URL (github.com/...) | Parse context from URL → call the relevant tool |
| Asks a question about an existing project | Answer directly; do not start building |
| Ambiguous — cannot determine intent | Ask ONE clarifying question: "Do you want me to build something new, or work on an existing project?" |

**Do not ask multiple clarifying questions.** One question, wait for answer, then proceed.

### 5.7 Config fields

ZeroBuild-specific fields in `ZerobuildConfig`:

| Field | Default | Purpose |
|-------|---------|---------|
| `github_client_id` | `""` | GitHub OAuth app client ID |
| `github_client_secret` | `""` | GitHub OAuth app client secret |
| `github_oauth_proxy` | official proxy URL | OAuth proxy for GitHub connector |
| `db_path` | `"~/.zerobuild/zerobuild.db"` | SQLite database path |

### 5.9 GitHub Ops Language and Content Rules (Required)

**ALL GitHub issues and pull requests MUST be written in English — no exceptions.**

This applies to:
- Issue title and body
- PR title and body
- Review comments
- Close/edit comments

Even if the user writes their request in another language, the agent MUST translate the content into English before calling any GitHub tool. Do not pass Vietnamese, Chinese, or any non-English content to `github_create_issue`, `github_create_pr`, `github_edit_issue`, or any review tool.

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

### 5.10 Auto-Invoke Product Advisor After Deploy

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

### 5.11 Error Recovery and Failure Escalation

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

---

## 6) Risk Tiers by Path

- **Low risk**: docs, test changes
- **Medium risk**: `src/tools/sandbox/`, `src/store/`, most `src/**` Rust changes
- **High risk**: `src/security/**`, `src/runtime/**`, `src/gateway/**`, `src/tools/deploy.rs`, `src/tools/github_ops.rs`, `src/gateway/oauth.rs`, `.github/workflows/**`, access-control boundaries

---

## 7) Agent Workflow (Required)

1. **Read before write** — inspect existing module and adjacent tests before editing.
2. **Define scope boundary** — one concern per PR; avoid mixed feature+refactor+infra patches.
3. **Implement minimal patch** — apply KISS/YAGNI/DRY rule-of-three.
4. **Validate by risk tier** — docs-only: lightweight; code/risky: full checks.
5. **Document impact** — update docs/PR notes for behavior, risk, side effects, rollback.
6. **Respect queue hygiene** — declare `Depends on #...` for stacked PRs.

### 7.1 Branch / Commit / PR Flow (Required)

- Create and work from a non-`main` branch.
- Commit changes to that branch with clear, scoped commit messages.
- Open a PR to `main`; do not push directly to `main`.
- Wait for required checks and review outcomes before merging.

### 7.2 Code Naming Contract (Required)

- Rust: modules/files `snake_case`, types/traits `PascalCase`, functions/variables `snake_case`, constants `SCREAMING_SNAKE_CASE`.
- Test identifiers: use project-scoped neutral labels (`zerobuild_user`, `zerobuild_node`).

### 7.3 Architecture Boundary Contract (Required)

- Sandbox runs as a local process with `env_clear()` — no host credentials leak into builds.
- OAuth tokens must never appear in logs, Telegram messages, or agent tool results.
- GitHub API calls must use token loaded from `src/store/tokens.rs` — never hardcoded.

---

## 8) Validation Matrix

### Rust (ZeroBuild Agent)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 9) Collaboration and PR Discipline

- Follow `.github/pull_request_template.md` fully.
- Keep PR descriptions concrete: problem, change, non-goals, risk, rollback.
- Use conventional commit titles.
- Prefer small PRs when possible.
- Agent-assisted PRs are welcome, but contributors remain accountable for understanding what their code will do.

### 9.1 Privacy/Sensitive Data (Required)

- Never commit API keys, bot tokens, OAuth secrets, or user IDs.
- Never log user messages, Telegram IDs, prompt content, or OAuth tokens in production.
- Use neutral project-scoped placeholders in tests and examples.

---

## 10) Anti-Patterns (Do Not)

- Do not add heavy dependencies for minor convenience.
- Do not silently weaken security policy or access constraints.
- Do not add speculative config/feature flags "just in case".
- Do not mix formatting-only changes with functional changes.
- Do not modify unrelated modules "while here".
- Do not bypass failing checks without explicit explanation.
- Do not hide behavior-changing side effects in refactor commits.
- Do not include personal identity or sensitive information in any commit.
- **ZeroBuild-specific**: Do not skip plan confirmation before building.
- **ZeroBuild-specific**: Do not expose OAuth tokens in tool results or Telegram messages.
- **ZeroBuild-specific**: Do not allow the agent to delete source files or directories when fixing build errors.
- **ZeroBuild-specific**: Do not re-scaffold (`npx create-next-app`) after the project is already built.

---

## 11) Handoff Template (Agent → Agent / Maintainer)

When handing off work, include:

1. What changed
2. What did not change
3. Validation run and results
4. Remaining risks / unknowns
5. Next recommended action

---

## 12) Vibe Coding Guardrails

When working in fast iterative mode:

- Keep each iteration reversible (small commits, clear rollback).
- Validate assumptions with code search before implementing.
- Prefer deterministic behavior over clever shortcuts.
- Do not "ship and hope" on security-sensitive paths.
- If uncertain, leave a concrete TODO with verification context, not a hidden guess.
