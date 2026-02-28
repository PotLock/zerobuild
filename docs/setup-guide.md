# ZeroBuild Setup Guide

Complete setup instructions for running your own ZeroBuild instance.

## Prerequisites

- Rust 1.80+ (`rustc --version`)
- Git
- Either:
  - **E2B API Key** (recommended for cloud sandboxes) — get from [e2b.dev](https://e2b.dev)
  - **Docker** (for local sandboxes)

## Quick Setup

```bash
# 1. Clone the repository
git clone https://github.com/potlock/zerobuild.git
cd zerobuild

# 2. Run bootstrap (installs Rust if needed)
./bootstrap.sh

# 3. Build release binary
cargo build --release

# 4. Run onboarding (interactive setup)
./target/release/zerobuild onboard --interactive

# 5. Start the gateway
./target/release/zerobuild gateway
```

## Step-by-Step Configuration

### 1. Choose Your Channel

ZeroBuild supports multiple channels. Choose the one that fits your workflow:

#### Option A: Telegram (Recommended for Mobile)

1. Message [@BotFather](https://t.me/botfather) on Telegram
2. Create a new bot with `/newbot`
3. Save the bot token
4. Set a username for your bot (e.g., `@yourbuildbot`)

During onboarding, you'll be prompted for the bot token.

#### Option B: Discord

1. Go to [Discord Developer Portal](https://discord.com/developers/applications)
2. Create a new application
3. Go to "Bot" section and create a bot
4. Copy the bot token
5. Enable necessary intents (Message Content, Server Members)
6. Generate OAuth2 URL with `bot` scope and add to your server

#### Option C: Slack

1. Go to [Slack API](https://api.slack.com/apps)
2. Create a new app
3. Add Bot Token Scopes: `chat:write`, `im:history`, `im:read`
4. Install to workspace and copy Bot User OAuth Token

#### Option D: CLI Only

Skip channel setup and use the agent directly:

```bash
zerobuild agent
```

### 2. Sandbox Provider (Choose One)

#### Option A: E2B Cloud Sandboxes (Recommended)

E2B provides ephemeral Linux MicroVMs in the cloud — fast, isolated, with public preview URLs for web apps.

```bash
# Get your API key from https://e2b.dev/dashboard
export E2B_API_KEY="e2b_..."

# Or add to config during onboarding
./target/release/zerobuild onboard --e2b-api-key "e2b_..."
```

#### Option B: Local Docker Sandboxes

If you prefer local sandboxes or don't have an E2B key:

```bash
# Ensure Docker is running
./target/release/zerobuild onboard --docker-image "node:20-slim"
```

ZeroBuild auto-detects: if `E2B_API_KEY` is set, it uses E2B; otherwise tries Docker.

### 3. LLM Provider Setup

ZeroBuild supports multiple LLM providers. Choose one:

#### OpenRouter (Recommended - Access to all models)

```bash
# Get key from https://openrouter.ai/keys
./target/release/zerobuild onboard \
  --provider openrouter \
  --api-key "sk-or-v1-..." \
  --model "anthropic/claude-sonnet-4"
```

#### Anthropic Direct

```bash
./target/release/zerobuild onboard \
  --provider anthropic \
  --api-key "sk-ant-..." \
  --model "claude-sonnet-4-5"
```

#### OpenAI

```bash
./target/release/zerobuild onboard \
  --provider openai \
  --api-key "sk-..." \
  --model "gpt-4.1"
```

See [providers-reference.md](providers-reference.md) for all supported providers.

### 4. Connectors (GitHub, etc.) — Zero Setup Required ✅

**Good news:** ZeroBuild now includes **seamless connectors** with zero configuration required!

When you or your users want to connect GitHub:

1. Simply say in chat: **"Connect GitHub"**
2. Click the link provided by the bot
3. Authorize on GitHub
4. Done! ✅

The connection allows you to:
- Create new repositories and push code
- Open issues on existing repos
- Create and review pull requests
- Commit and push updates

**How it works:** ZeroBuild uses an official OAuth Proxy service that securely handles the GitHub OAuth flow. No need to create your own GitHub OAuth App or configure client IDs.

#### Advanced: Use Your Own GitHub OAuth App (Optional for GitHub Connector)

If you prefer to use your own GitHub OAuth App instead of the default proxy:

1. Go to GitHub → Settings → Developer Settings → OAuth Apps
2. Click "New OAuth App"
3. Fill in:
   - **Application name**: Your ZeroBuild Instance
   - **Homepage URL**: `http://127.0.0.1:3000` (or your preferred port)
   - **Authorization callback URL**: `http://127.0.0.1:3000/auth/github/callback`
4. Save and note the **Client ID** and **Client Secret**
5. Configure ZeroBuild:

```bash
./target/release/zerobuild onboard \
  --github-client-id "Iv1.xxx" \
  --github-client-secret "xxx"
```

## Running ZeroBuild

### Development Mode

```bash
# Start the gateway
./target/release/zerobuild gateway --port 3000

# The actual port may be dynamic (e.g., http://127.0.0.1:42617)
# Check the output for the exact URL

# For Telegram - set up webhook (for local dev, use ngrok)
curl -F "url=https://your-ngrok-url/webhook" \
  https://api.telegram.org/bot<TOKEN>/setWebhook
```

### Production Mode

```bash
# Run as daemon (includes gateway + channels)
./target/release/zerobuild daemon

# Or use service management
./target/release/zerobuild service install
./target/release/zerobuild service start
```

## Configuration File

After onboarding, config is stored at `~/.zerobuild/config.toml`:

```toml
[provider]
provider = "openrouter"
api_key = "sk-or-v1-..."
model = "anthropic/claude-sonnet-4"

[channels.telegram]
enabled = true
identity = "@yourbuildbot"
token = "..."

[channels.discord]
enabled = false
token = "..."

[channels.slack]
enabled = false
token = "xoxb-..."

[zerobuild]
e2b_api_key = "e2b_..."  # or docker_image = "node:20-slim"
# GitHub OAuth - leave empty to use official proxy (recommended)
github_client_id = ""
github_client_secret = ""
github_oauth_proxy = "https://zerobuild-oauth-proxy.githubz.workers.dev"
db_path = "./data/zerobuild.db"
```

## Testing Your Setup

Try these example requests to test different build types:

### Web App
*"Build me a landing page for my coffee shop"*

### Mobile App
*"Create a React Native app for tracking daily expenses"*

### Backend API
*"Build a REST API with Express and MongoDB for a todo app"*

### CLI Tool
*"Write a Python CLI tool that converts CSV to JSON with filtering options"*

### Script/Automation
*"Create a Node.js script that fetches weather data and sends email alerts"*

**Workflow:**
1. Send your request via your configured channel
2. The agent proposes a plan — confirm it
3. Watch the build progress
4. For web apps: receive a preview URL
5. Request changes or iterate
6. Say "Push this to GitHub" to create a repo, or ask "Create an issue for this bug" — full GitHub ops from chat

**GitHub Integration Test:**
1. Say: "Connect GitHub"
2. Click the link and authorize
3. Say: "Push this project to GitHub"
4. Or: "Create an issue for adding user authentication"

## Troubleshooting

### Sandbox not starting

- Check `E2B_API_KEY` is set correctly
- Or ensure Docker is running for local mode

### Channel not responding

- **Telegram**: Verify webhook with `curl https://api.telegram.org/bot<TOKEN>/getWebhookInfo`
- **Discord**: Check bot has proper permissions and intents enabled
- **Slack**: Verify bot is installed to workspace and token is correct

### Build errors

- Check sandbox logs in messages
- Ensure the correct runtime is available in your sandbox (Node.js, Python, etc.)

### GitHub connection issues

- Make sure you're using the latest version of ZeroBuild
- If using custom OAuth App, verify the callback URL matches your gateway URL
- Check logs with `zerobuild doctor`

## Next Steps

- [Commands Reference](commands-reference.md) — Full CLI documentation
- [Config Reference](config-reference.md) — All configuration options
- [Architecture Overview](../CLAUDE.md) — How ZeroBuild works internally
