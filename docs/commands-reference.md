# ZeroBuild Commands Reference

Complete CLI reference for ZeroBuild.

Last verified: **March 2026**.

## Top-Level Commands

| Command | Purpose |
|---------|---------|
| `onboard` | Initialize workspace and configuration |
| `agent` | Run interactive chat or single-message mode |
| `gateway` | Start webhook gateway for Telegram, Discord, Slack, etc. |
| `daemon` | Start full runtime (gateway + channels + scheduler) |
| `service` | Manage OS service lifecycle |
| `doctor` | Run diagnostics |
| `status` | Show system status |
| `cron` | Manage scheduled tasks |
| `models` | Refresh provider model catalogs |
| `providers` | List supported AI providers |
| `channel` | Manage channels (Telegram, Discord, Slack) |
| `skills` | Manage skills |
| `memory` | Manage agent memory |
| `config` | Export configuration schema |
| `completions` | Generate shell completions |

## Essential Commands

### `onboard`

Initialize ZeroBuild with your API keys and preferences.

```bash
# Interactive onboarding
zerobuild onboard --interactive

# Non-interactive with all options
zerobuild onboard \
  --api-key "sk-or-v1-..." \
  --provider openrouter \
  --model "anthropic/claude-sonnet-4" \
  --e2b-api-key "e2b_..." \
  --github-client-id "Iv1.xxx" \
  --github-client-secret "xxx"

# Docker instead of E2B
zerobuild onboard \
  --docker-image "node:20-slim"
```

**Options:**
- `--api-key <KEY>` — Provider API key
- `--provider <ID>` — Provider ID (openrouter, anthropic, openai, etc.)
- `--model <MODEL>` — Default model ID
- `--e2b-api-key <KEY>` — E2B API key for cloud sandboxes
- `--docker-image <IMAGE>` — Docker image for local sandboxes
- `--github-client-id <ID>` — GitHub OAuth app client ID (for repo creation, push, issues, PRs)
- `--github-client-secret <SECRET>` — GitHub OAuth app secret (for repo creation, push, issues, PRs)
- `--interactive` — Interactive setup wizard
- `--force` — Overwrite existing config

### `gateway`

Start the webhook gateway to receive messages from Telegram, Discord, Slack, etc.

```bash
# Default: localhost:3000
zerobuild gateway

# Custom host/port
zerobuild gateway --host 0.0.0.0 --port 8080
```

### `daemon`

Start the full autonomous runtime (gateway + channels + scheduler).

```bash
zerobuild daemon
zerobuild daemon --host 0.0.0.0 --port 3000
```

### `agent`

Run the agent in CLI mode.

```bash
# Interactive mode
zerobuild agent

# Single message - web app
zerobuild agent -m "Build a landing page with React"

# Single message - mobile app
zerobuild agent -m "Create a React Native todo app"

# Single message - backend
zerobuild agent -m "Build a Python FastAPI service with user auth"

# Single message - CLI tool
zerobuild agent -m "Write a Node.js CLI for file encryption"

# With specific provider/model
zerobuild agent --provider anthropic --model claude-sonnet-4-5
```

### `channel`

Manage communication channels.

```bash
# List configured channels
zerobuild channel list

# Telegram
zerobuild channel bind-telegram @yourbot

# Discord
zerobuild channel add discord '{"token":"YOUR_BOT_TOKEN"}'

# Slack
zerobuild channel add slack '{"token":"xoxb-YOUR-TOKEN"}'

# Start all channels
zerobuild channel start
```

### `models`

Refresh available models from providers.

```bash
# Refresh all providers
zerobuild models refresh

# Refresh specific provider
zerobuild models refresh --provider openrouter
```

### `status`

Check system status.

```bash
zerobuild status
```

### `doctor`

Run diagnostics.

```bash
# General diagnostics
zerobuild doctor

# Check models
zerobuild doctor models

# View traces
zerobuild doctor traces
```

### `memory`

Manage agent memory.

```bash
# List memories
zerobuild memory list

# Get specific memory
zerobuild memory get <KEY>

# Clear all memories
zerobuild memory clear

# Show memory stats
zerobuild memory stats
```

### `service`

Manage ZeroBuild as a system service.

```bash
# Install service
zerobuild service install

# Start service
zerobuild service start

# Check status
zerobuild service status

# Stop service
zerobuild service stop

# Restart service
zerobuild service restart

# Uninstall service
zerobuild service uninstall
```

### `config`

Export configuration schema.

```bash
# Print JSON schema
zerobuild config schema
```

### `completions`

Generate shell completion scripts.

```bash
zerobuild completions bash >> ~/.bashrc
zerobuild completions zsh >> ~/.zshrc
zerobuild completions fish > ~/.config/fish/completions/zerobuild.fish
```

## Configuration File

ZeroBuild stores configuration at `~/.zerobuild/config.toml`:

```toml
[provider]
provider = "openrouter"
api_key = "sk-or-v1-..."
model = "anthropic/claude-sonnet-4"

[channels.telegram]
enabled = true
identity = "@yourbuildbot"
token = "..."

[zerobuild]
e2b_api_key = "e2b_..."
# GitHub connector — leave empty to use official OAuth Proxy (recommended)
github_client_id = ""
github_client_secret = ""
db_path = "./data/zerobuild.db"
```

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `E2B_API_KEY` | E2B sandbox API key |
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `TELEGRAM_BOT_TOKEN` | Telegram bot token |
| `DISCORD_BOT_TOKEN` | Discord bot token |
| `SLACK_BOT_TOKEN` | Slack bot token |

## Getting Help

```bash
# General help
zerobuild --help

# Command-specific help
zerobuild onboard --help
zerobuild agent --help
zerobuild gateway --help
```
