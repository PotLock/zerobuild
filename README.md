<p align="center">
  <img src="zerobuild.png" alt="ZeroBuild" width="200" />
</p>

<h1 align="center">ZeroBuild — Autonomous Software Factory 🏭</h1>

<p align="center">
  <strong>Build and host your own autonomous software factory.</strong><br>
  ⚡️ <strong>From idea to production — a hierarchical multi-agent team of AI specialists (Orchestrator, BA, UI/UX, Dev, Tester, DevOps) auto-builds your software. Zero coding. Zero management. Deploy-ready.</strong>
</p>

<p align="center">
  <a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-MIT%20OR%20Apache%202.0-blue.svg" alt="License: MIT OR Apache-2.0" /></a>
  <a href="NOTICE"><img src="https://img.shields.io/github/contributors/zerobuild/zerobuild?color=green" alt="Contributors" /></a>
  <a href="https://t.me/zerobuild_bot"><img src="https://img.shields.io/badge/Telegram-Bot-26A5E4?style=flat&logo=telegram&logoColor=white" alt="Telegram Bot" /></a>
</p>

<p align="center">
Built on <a href="https://github.com/zeroclaw-labs/zeroclaw">ZeroClaw</a> — the Rust-first autonomous agent runtime.
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> |
  <a href="bootstrap.sh">One-Click Setup</a> |
  <a href="docs/commands-reference.md">Commands</a> |
  <a href="docs/setup-guide.md">Setup Guide</a>
</p>

<p align="center">
  <strong>Describe your idea. The AI team takes over — analyzing, designing, coding, testing, and deploying.</strong><br />
  No coding skills. No team management. Just results.
</p>

<p align="center"><code>Autonomous Software Factory · Hierarchical Multi-Agent · Zero Management · Isolated Sandboxes · Deploy-Ready</code></p>

---

## ✨ What is ZeroBuild?

ZeroBuild is an **Autonomous Software Factory** that lets you build and host your own AI-powered software development system. Through a **Hierarchical Multi-Agent System**, you provide a raw idea in natural language, and ZeroBuild automatically assembles a team of AI specialists — Orchestrator (CEO), Business Analyst, UI/UX Designer, Developer, Tester, and DevOps Engineer — that coordinate to automate the entire software development lifecycle and deliver a production-ready product.

**Think of it as hiring an entire software team, but it's all AI — and it costs pennies.**

**What you can build:**
- 🌐 **Web applications** — Next.js, React, Vue, static sites
- 📱 **Mobile apps** — React Native, Flutter, Ionic
- ⚙️ **Backend services** — APIs, microservices, serverless functions
- 🛠️ **CLI tools & scripts** — Python, Node.js, Rust utilities
- 🎮 **Games & interactive apps** — WebGL, Canvas, game prototypes
- 🤖 **Automation & bots** — Scrapers, workflows, integrations
- And anything else you can describe...

**Core values:**

- 🚀 **Idea to Code in minutes** — Shrink development time from months to hours
- 🤖 **Zero Management** — No coding skills, no team management; the Orchestrator (CEO) handles all task delegation and supervision
- 💰 **Ultra-low cost** — Replace expensive engineering teams with API token costs
- 🏭 **Full SDLC automation** — Requirements → Design → Code → Test → Deploy, all automated

**Key capabilities:**

- 🏢 **Hierarchical multi-agent factory** — Orchestrator (CEO) spawns specialized sub-agents (BA, UI/UX, Dev, Tester, DevOps) with dedicated contexts and permissions
- 🔄 **Cross-agent collaboration** — BA writes PRD → UI/UX creates design spec → Dev implements → Tester validates → automatic fix loops until perfect → DevOps deploys
- 🏗️ **Plan-before-build workflow** — Agent proposes a structured plan; you confirm before any code is written
- 🔒 **Isolated sandboxes** — Every build runs in an isolated local process sandbox; host credentials and filesystem stay untouched
- 🌐 **Live preview URLs** — Get public HTTPS links to running web apps
- 🚀 **GitHub connector** — Connect your GitHub account via OAuth to create repos, push code, open/comment on issues, manage PRs, post inline code reviews, and reply to discussions — all from chat
- 🧠 **Intelligent model routing** — Automatic model recommendations based on task type
- 💬 **Multi-channel support** — Use Telegram, Discord, Slack, or CLI — your choice

---

## 🚀 Quick Start

```bash
# 1. Clone and bootstrap
git clone https://github.com/potlock/zerobuild.git
cd zerobuild
./bootstrap.sh

# 2. Build the release binary
cargo build --release

# 3. Onboard with your API keys
./target/release/zerobuild onboard --interactive

# 4. Start the gateway
./target/release/zerobuild gateway
```

Then message your bot: *"Build me a REST API for a todo app"* or *"Create a mobile app for tracking expenses"*

See the full [Setup Guide](docs/setup-guide.md) for detailed instructions.

---

## 🏗️ Architecture — The Autonomous Software Factory

```
User provides idea (natural language)
    │
    ▼
┌───────────────────────────────────────────────────────┐
│  🏢 Orchestrator Agent (CEO)                          │
│  • Receives idea, analyzes feasibility                │
│  • Creates project plan                               │
│  • Spawns specialized sub-agents                      │
│  • Supervises & coordinates all phases                │
│  • Reports progress to user                           │
│                                                       │
│  Phase 1: Analysis (Sequential)                       │
│  ┌──────────────────────────────────┐                 │
│  │  📋 BA Agent                     │                 │
│  │  Writes PRD & requirements       │─────┐           │
│  └──────────────────────────────────┘     │           │
│                                           ▼           │
│  Phase 2: Parallel Build (Concurrent)                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐              │
│  │ 🎨 UI/UX │ │ 💻 Dev   │ │ 🧪 Test  │              │
│  │  Agent   │ │  Agent   │ │  Agent   │              │
│  └──────────┘ └──────────┘ └──────────┘              │
│                     │             │                    │
│  Phase 3: Integration Loop  ◄─────┘                   │
│  ┌──────────────────────────────────┐                 │
│  │  💻 Dev ◄──► 🧪 Tester           │                 │
│  │  (auto-fix loop until perfect)   │                 │
│  └──────────────────────────────────┘                 │
│                     │                                 │
│  Phase 4: Deployment                                  │
│  ┌──────────────────────────────────┐                 │
│  │  🚀 DevOps Agent                 │                 │
│  │  Deploy to GitHub / live URL     │                 │
│  └──────────────────────────────────┘                 │
└───────────────────────────────────────────────────────┘
    │
    ▼
Local Process Sandbox             ← Isolated Build Environment
  • Temp directory with cleared environment (no credential leaks)
  • Scaffolds projects, installs dependencies
  • Runs dev servers on localhost with live preview URLs
```

**Dual-mode architecture:** ZeroBuild operates in two modes:
- **Single-agent mode** (default) — One unified agent handles conversation, planning, coding, and deployment
- **Factory mode** (enabled by default, `factory.enabled = true`) — The Orchestrator (CEO) automatically spawns specialized sub-agents for complex tasks. For simple requests, the agent handles directly without invoking factory.

### 📋 Blackboard — Shared Agent State

The **Blackboard** is the shared state layer enabling cross-agent collaboration. Built on `InMemoryMessageBus` and `SharedContextEntry`, it allows agents to publish and read artifacts:

| Artifact | Producer | Consumers |
|----------|----------|-----------|
| `artifact:prd` | Business Analyst | All agents |
| `artifact:design_spec` | UI/UX Designer | Developer, Tester |
| `artifact:source_code` | Developer | Tester, DevOps |
| `artifact:test_cases` | Tester | Developer (integration loop) |
| `artifact:test_results` | Tester | Orchestrator (phase control) |
| `artifact:deploy_config` | DevOps | Orchestrator (final result) |

Agents use versioned writes (`ContextPatch`) with optimistic-locking to prevent conflicts. All factory agents share the same sandbox filesystem, enabling collaborative file access.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full multi-agent design.

---

## 🛠️ How It Works — The Execution Flow

1. **Describe** — Provide your idea in natural language (any language, any channel)
2. **Plan** — The Orchestrator (CEO) analyzes feasibility and proposes a plan
3. **Confirm** — You approve the plan (or request changes)
4. **Spawn** — The Orchestrator creates specialized sub-agents (BA, UI/UX, Dev, Tester, DevOps) with dedicated contexts
5. **Build** — Agents collaborate autonomously:
   - BA writes requirements (PRD) → shared with all agents
   - UI/UX, Dev, and Tester work in parallel
   - Dev-Tester auto-fix loop runs until all tests pass (max 5 iterations by default)
6. **Deploy** — DevOps agent deploys the finished product (live URL + GitHub repo)
7. **Iterate** — Request changes; the team re-engages and rebuilds

---

## 🌟 Features

| Feature | Description |
|---------|-------------|
| **Autonomous Software Factory** | A full AI team (PM, BA, UI/UX, Dev, Tester, DevOps) that builds your software autonomously |
| **Hierarchical Multi-Agent** | Orchestrator (CEO) spawns, delegates, and supervises specialized sub-agents with cross-agent collaboration |
| **Auto Dev-Test Loops** | Developer and Tester agents iterate automatically until all tests pass — no human intervention |
| **Full SDLC Automation** | Requirements → Design → Code → Test → Deploy, entirely automated |
| **Build Anything** | Web, mobile, backend, CLI tools, scripts, games — anything you can describe |
| **Multi-Channel** | Telegram, Discord, Slack, Matrix, or CLI — use what you prefer |
| **Zero-dependency Sandbox** | Isolated local process sandbox — no API key, no Docker daemon required |
| **Ultra-low Cost** | Replace entire dev teams with API token costs |
| **Multi-Provider LLM** | OpenAI, Anthropic, OpenRouter, DeepSeek, Gemini, and more |
| **Secure by Default** | OAuth tokens stored in SQLite only; never in logs or messages |
| **GitHub Connector** | Create/comment on issues & PRs, code reviews, push code — all via chat |

---

## 📊 ZeroBuild vs Alternatives

| | ZeroBuild | Bolt.new | Lovable | V0 | OpenClaw |
|---|:---:|:---:|:---:|:---:|:---:|
| **What you can build** | Anything | Web only | Web only | Web only | Anything |
| **Interface** | Any channel | Web | Web | Web | CLI only |
| **Sandbox** | Local process (no API key, no Docker) | StackBlitz | Own cloud | Vercel | Docker |
| **Open Source** | ✅ Yes | ❌ No | ❌ No | ❌ No | ✅ Yes |
| **Self-Hostable** | ✅ Yes | ❌ No | ❌ No | ❌ No | ✅ Yes |
| **Runtime** | Rust (<10MB) | Cloud | Cloud | Cloud | Node.js |
| **Multi-Agent Team** | ✅ Full SDLC (Orchestrator, BA, UI/UX, Dev, Tester, DevOps) | ❌ Single agent | ❌ Single agent | ❌ Single agent | ❌ Single agent |
| **GitHub Connector** | ✅ Full (repos, issues, PRs, comments, inline review, push) | ❌ No | ❌ No | ❌ No | Manual |

---

## 🙏 Credits

ZeroBuild is built on top of **[ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)** by zeroclaw-labs — the Rust-first autonomous agent runtime optimized for performance, security, and portability.

---

## 📄 License

ZeroBuild is dual-licensed under:

| License | Use case |
|---|---|
| [MIT](LICENSE-MIT) | Open-source, research, academic, personal use |
| [Apache 2.0](LICENSE-APACHE) | Patent protection, institutional, commercial deployment |

You may choose either license.

---

## 🔗 Links

- [Setup Guide](docs/setup-guide.md) — Full installation and configuration
- [Commands Reference](docs/commands-reference.md) — CLI documentation
- [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) — The runtime that powers ZeroBuild

---

**ZeroBuild** — Your AI Software Company. Idea in, product out. 🏭
