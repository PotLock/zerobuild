# ZeroBuild 

![ZeroBuild](zerobuild.png)

> **Forked from [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)** — a Rust-first autonomous agent runtime.
> ZeroBuild extends ZeroBuild into a Telegram-native web application builder powered by E2B MicroVM sandboxes.

---

## What is ZeroBuild?

ZeroBuild lets users build production-quality web applications through natural conversation on Telegram. You describe what you want, the agent plans it, you confirm, and a live preview link appears in minutes — no setup required.

**Built on top of ZeroBuild's runtime infrastructure**, ZeroBuild adds:

- **Telegram-native UX** — converse directly in Telegram, receive progress updates and preview links without leaving the app
- **E2B MicroVM sandboxes** — each build runs inside an isolated, ephemeral Linux MicroVM; no host exposure
- **Master Agent + Builder Agent architecture** — two-tier separation: Master Agent (ZeroBuild runtime, Rust) handles conversation and planning; Builder Agent (Node.js, E2B) handles code generation and preview
- **Re-hydration pattern** — sandbox state is preserved via SQLite snapshots; future sessions restore previous builds (Manus-style)
- **Plan-before-build workflow** — agent proposes a structured plan (tech stack, pages, components, build phases), waits for user confirmation, then builds
- **GitHub OAuth deploy** — users connect their GitHub account via OAuth; agent pushes code to a new repo on request
- **Multi-user concurrency** — atomic job creation, SQLite WAL mode, per-user rate limiting

---

## Architecture

```
User (Telegram)
    │
    ▼
ZeroBuild Runtime (Rust)          ← Master Agent
  • Receives Telegram messages
  • Runs agent loop with tools
  • create_job tool → HTTP POST → Orchestrator
  • github_create_issue / github_create_pr / github_review_pr tools
    │
    ▼
ZeroBuild Orchestrator (Node.js / Express)   ← Backend
  • POST /jobs — creates job in SQLite (atomic check+insert), enqueues build
  • GET /auth/github — GitHub OAuth flow (stores token per user)
  • POST /github/* — GitHub operations (issue, PR, review)
  • Spawns E2B MicroVM sandbox per job
  • Runs Builder Agent (OpenAI-compatible agentic loop)
  • Sends progress and preview URL back via Telegram Bot API
    │
    ▼
E2B MicroVM Sandbox              ← Builder Agent
  • Ubuntu Linux, Node.js 20, npm pre-installed
  • Scaffolds Next.js app with npx create-next-app
  • Writes components, runs npm run build
  • Starts dev server on 0.0.0.0 → public preview URL via E2B port forwarding
```

---

## Key Improvements Over Base ZeroBuild

| Feature | ZeroBuild (base) | ZeroBuild |
|---|---|---|
| Runtime sandbox | Docker (local) | E2B MicroVM (cloud, ephemeral) |
| Build interface | CLI | Telegram conversation |
| Preview URLs | None | E2B public port forwarding (HTTPS) |
| Agent identity | ZeroBuild | ZeroBuild (user-facing name) |
| Plan enforcement | None | Required before every build |
| Builder LLM | Any provider | Kimi-for-coding (`kimi-code` endpoint) |
| Progress reporting | Log only | Telegram messages (component-level milestones) |
| System prompt | Inline | External `src/prompts/builder.md` |
| Frontend design | None | Anthropic Frontend Design skill embedded |
| GitHub deploy | None | OAuth flow → push snapshot to GitHub repo |
| Concurrency | Single-user | Multi-user with atomic job slots + rate limiting |

---

## Repository Layout

```
zerobuild/                  ← ZeroBuild Rust runtime (master agent)
  src/
    agent/                 ← orchestration loop
    providers/             ← LLM providers
    tools/
      create_job.rs
      deploy.rs
      github_ops.rs  ← GitHub ops tools
    gateway/               ← /internal/notify endpoint
    config/
  IDENTITY.md              ← ZeroBuild user-facing persona

backend/                   ← ZeroBuild orchestrator (Node.js)
  src/
    services/
      agent.ts             ← Builder Agent agentic loop
      builder.ts           ← job runner, Telegram notifications, reset guard
      e2bService.ts        ← E2B sandbox lifecycle
      githubService.ts     ← GitHub REST API (push, issue, PR, review)
    prompts/
      builder.md           ← Builder Agent system prompt (external markdown)
    db/
      index.ts             ← SQLite init (WAL mode, busy_timeout)
      jobRepository.ts     ← atomic job CRUD
      tokenRepository.ts   ← GitHub OAuth tokens
    routes/
      jobs.ts              ← POST /jobs, GET /jobs/:id, POST /jobs/:id/deploy
      auth.ts              ← GET /auth/github (public OAuth endpoints)
      github.ts            ← POST /github/* (protected GitHub ops)
    types/
  package.json
```

---

## Credits

ZeroBuild is built on top of [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) by zeroclaw-labs.
ZeroBuild is licensed under MIT. All ZeroBuild-origin code retains its original license.
