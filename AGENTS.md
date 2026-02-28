# AGENTS.md — ZeroBuild Agent Engineering Protocol

> **Forked from ZeroBuild.** ZeroBuild is a customization of the ZeroBuild agent runtime that builds web applications via Telegram using E2B MicroVM sandboxes. This protocol extends ZeroBuild's base AGENTS.md with ZeroBuild-specific rules.

This file defines the default working protocol for coding agents in this repository.
Scope: entire repository (Rust runtime only — Node.js backend removed).

---

## 1) Project Snapshot (Read First)

**ZeroBuild** is a single-tier AI agent system built on ZeroBuild:

- **Master Agent** — ZeroBuild Rust runtime. Handles Telegram conversations, proposes plans, writes code directly into E2B sandboxes using built-in E2B tools, and pushes to GitHub.

ZeroBuild (the upstream base) is a Rust-first autonomous agent runtime optimized for performance, efficiency, stability, extensibility, sustainability, and security. ZeroBuild keeps all of that and adds a web-app-building product layer on top.

### Core ZeroBuild extension points (unchanged)

- `src/providers/traits.rs` (`Provider`)
- `src/channels/traits.rs` (`Channel`)
- `src/tools/traits.rs` (`Tool`)
- `src/memory/traits.rs` (`Memory`)
- `src/observability/traits.rs` (`Observer`)
- `src/runtime/traits.rs` (`RuntimeAdapter`)

### ZeroBuild-specific extension points

- `src/tools/e2b/` — E2B sandbox tools (8 tools: create, run, write, read, list, preview, snapshot, kill)
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
ZeroBuild Runtime (Rust)   ← Master Agent
  • Runs the conversation loop
  • Proposes plans, waits for user confirmation
  • Calls E2B tools directly (no backend proxy)
  • Calls github_* tools → GitHub REST API directly
  • Calls request_deploy → GitHub git tree/commit/ref API
    │
    ▼
E2B MicroVM               ← Isolated build sandbox
  • Ubuntu, Node.js 20, npm
  • scaffold → build → preview
```

### Why single-tier

1. **Simplicity**: No HTTP proxy layer between Master Agent and E2B. Fewer moving parts = easier to debug.
2. **Security boundary preserved**: OAuth tokens stored in SQLite only, never in logs or agent messages.
3. **Re-hydration pattern**: SQLite snapshots (`src/store/snapshot.rs`) allow future sessions to restore previous builds.
4. **Direct GitHub push**: `request_deploy` uses git blobs/tree/commit/ref API — no intermediate service needed.

### Identity boundary

- **User-facing name**: `ZeroBuild` — users interact with ZeroBuild via Telegram
- **Runtime engine**: ZeroBuild — internal name, never shown to users
- **`IDENTITY.md`**: loaded by the Master Agent to enforce this boundary

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
- E2B API key read from `E2B_API_KEY` env var first; fallback to config field.
- OAuth tokens stored in SQLite only, never passed through agent chat or logs.

### 3.7 Determinism + Reproducibility

- Prefer reproducible commands and locked dependency behavior.
- Keep tests deterministic.

### 3.8 Reversibility + Rollback-First Thinking

- Keep changes easy to revert (small scope, clear blast radius).
- For risky changes, define rollback path before merge.

---

## 4) Repository Map

### Rust (Master Agent)

- `src/main.rs` — CLI entrypoint
- `src/agent/` — orchestration loop
- `src/providers/` — LLM providers
- `src/tools/e2b/` — E2B sandbox tools (8 tools)
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

### 5.1 E2B tool workflow

The Master Agent uses these 8 E2B tools to build web apps:

| Tool | Purpose |
|------|---------|
| `e2b_create_sandbox` | Create/resume E2B sandbox (reset=true to start fresh) |
| `e2b_run_command` | Run shell commands (npm, npx, node, etc.) |
| `e2b_write_file` | Write file content to sandbox path |
| `e2b_read_file` | Read file content from sandbox path |
| `e2b_list_files` | List directory contents |
| `e2b_get_preview_url` | Get preview URL for running dev server (default port 3000) |
| `e2b_save_snapshot` | Extract files from sandbox to SQLite (persist project) |
| `e2b_kill_sandbox` | Kill sandbox when done |

**Recommended build workflow:**
1. `e2b_create_sandbox` (reset=true if user requests fresh start)
2. `e2b_run_command` to scaffold (`npx create-next-app`, `npm install`)
3. `e2b_write_file` to create/edit files
4. `e2b_read_file` / `e2b_list_files` to inspect code
5. `e2b_run_command` to start dev server (`npm run dev &`)
6. `e2b_get_preview_url` (port=3000) to get preview URL
7. `e2b_save_snapshot` to persist code to SQLite
8. Send preview URL to user

### 5.2 Plan enforcement

The Master Agent must always propose and confirm a plan before building. This is enforced at system-prompt level.

Never skip the plan step. Plan-before-build is a core product guarantee.

### 5.3 E2B workspace path

Agent workspace inside sandbox: `/home/user/project/`

- E2B base template runs as non-root user — workspace **must** be under `/home/user/`.
- App directory: `/home/user/project/` (Next.js project root here).
- `npm run build` **must** be run from `/home/user/project/`.

### 5.4 Next.js project structure

The Master Agent must maintain this layout:

```
/home/user/project/         ← Next.js project root (package.json here)
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
3. `e2b_read_file` that file
4. `e2b_write_file` only that file with the targeted fix
5. Re-run `npm run build` via `e2b_run_command`

**FORBIDDEN:**
- `rm -rf` on any source directory (app/, components/, lib/, public/, src/)
- Writing empty content to `app/page.tsx` or `layout.tsx`
- Re-running `npx create-next-app` after scaffold
- Deleting and recreating components from scratch

**Allowed rm targets (build artifacts only):** `node_modules`, `.next`, `.cache`, `dist`, `out`, `build`

### 5.6 GitHub OAuth deploy flow

1. Master agent calls `request_deploy` tool
2. If GitHub not connected: error with `/auth/github` URL
3. User visits `/auth/github` → GitHub OAuth → callback stores token in SQLite
4. User tells agent "done" → agent retries `request_deploy`
5. Tool reads token from SQLite, creates repo + pushes code via GitHub git trees API
6. Returns repo URL to user

OAuth tokens stored in `src/store/tokens.rs` — never in logs or Telegram messages.

### 5.7 Config fields

ZeroBuild-specific fields in `ZerobuildConfig`:

| Field | Default | Purpose |
|-------|---------|---------|
| `e2b_api_key` | `""` | E2B API key (prefer `E2B_API_KEY` env var) |
| `e2b_template` | `"code-interpreter-v1"` | E2B sandbox template |
| `e2b_timeout_ms` | `600000` | Sandbox timeout (10 minutes) |
| `github_client_id` | `""` | GitHub OAuth app client ID |
| `github_client_secret` | `""` | GitHub OAuth app client secret |
| `db_path` | `"./data/zerobuild.db"` | SQLite database path |

---

## 6) Risk Tiers by Path

- **Low risk**: docs, test changes
- **Medium risk**: `src/tools/e2b/`, `src/store/`, most `src/**` Rust changes
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

- Master Agent communicates with E2B directly via HTTP — no proxy layer.
- E2B API key must not appear in logs, Telegram messages, or tool results.
- OAuth tokens must never appear in logs, Telegram messages, or agent tool results.
- GitHub API calls must use token loaded from `src/store/tokens.rs` — never hardcoded.

---

## 8) Validation Matrix

### Rust (Master Agent)

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

- Never commit API keys, bot tokens, E2B API keys, OAuth secrets, or user IDs.
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
- **ZeroBuild-specific**: Do not expose OAuth tokens or E2B API keys in tool results or Telegram messages.
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
