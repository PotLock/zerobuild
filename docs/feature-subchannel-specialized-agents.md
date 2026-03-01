# Feature Design: Subchannel-Specialized Agents

## Summary

Enable each sub-channel (Telegram forum topic, Discord channel, Slack channel) to be assigned a specialized agent with its own system prompt, tool allowlist, and model configuration. A Telegram supergroup, Discord server, or Slack workspace can host multiple sub-channel agents running in parallel, each operating independently.

---

## Sub-Channel Mapping by Platform

| Platform | Sub-channel Definition | Current Routing Key | Thread Support |
|----------|------------------------|---------------------|----------------|
| Telegram | Forum Topic (`message_thread_id`) | `reply_target = "chat_id:thread_id"` | `thread_id` extracted from `reply_target` |
| Discord | Server Channel (`channel_id`) | `reply_target = channel_id` | `thread_ts = None` (not yet implemented) |
| Slack | Workspace Channel (`channel_id`) | `reply_target = channel_id` | `thread_ts` already functional |

### Key Observations:
- **Discord & Slack**: The routing key (`channel_id`) is already the `reply_target` → conversation history is naturally isolated. Only need to add per-channel agent config.
- **Telegram**: `reply_target = "chat_id:thread_id"` but `conversation_history_key` uses `"channel_sender"` → does NOT include `thread_id` → all topics within the same group share contaminated history.

---

## Current Bug to Fix (Telegram)

**File**: `src/channels/mod.rs:269-271`

```rust
// Current — incorrect for Telegram forum topics
fn conversation_history_key(msg: &traits::ChannelMessage) -> String {
    format!("{}_{}", msg.channel, msg.sender)
}
```

Telegram sends `reply_target = "-100200300:789"` (chat:thread), but the key only uses `msg.sender` → users messaging in topic #coder and topic #reviewer share the same history.

**Fix**: Include `reply_target` in the key (already contains sufficient encoded information):

```rust
fn conversation_history_key(msg: &traits::ChannelMessage) -> String {
    format!("{}_{}_{}", msg.channel, msg.reply_target, msg.sender)
}
```

---

## Configuration Design (Generic for All Channels)

Add a shared `SubchannelAgentConfig` struct:

### Example `config.toml`

```toml
# ── Telegram ──
[channels_config.telegram]
bot_token = "..."
allowed_users = ["*"]

[[channels_config.telegram.subchannel_agents]]
# Telegram Forum Topic's message_thread_id
subchannel_id = "123"
name = "coder"
system_prompt = "You are a coding agent. Build Next.js apps in isolated sandboxes."
model = "anthropic/claude-opus-4-20250514"
allowed_tools = ["shell", "file_read", "file_write", "sandbox_create", "sandbox_run_command"]

[[channels_config.telegram.subchannel_agents]]
subchannel_id = "456"
name = "reviewer"
system_prompt = "You review code for bugs, security issues, and quality."
model = "anthropic/claude-sonnet-4-20250514"
allowed_tools = ["file_read", "github_ops"]

[[channels_config.telegram.subchannel_agents]]
subchannel_id = "789"
name = "research"
system_prompt = "You research topics and summarize findings."
model = "perplexity/llama-3.1-sonar-large-128k-online"
provider = "openrouter"
allowed_tools = ["browser", "http_request"]

# ── Discord ──
[channels_config.discord]
bot_token = "..."
guild_id = "MY_SERVER_ID"
allowed_users = ["*"]

[[channels_config.discord.subchannel_agents]]
# Discord text channel's channel_id
subchannel_id = "1234567890"
name = "coder"
system_prompt = "You are a coding agent..."
model = "anthropic/claude-opus-4-20250514"
allowed_tools = ["shell", "file_read", "file_write"]

# ── Slack ──
[channels_config.slack]
bot_token = "xoxb-..."

[[channels_config.slack.subchannel_agents]]
# Slack channel_id (C...)
subchannel_id = "C1234567890"
name = "reviewer"
system_prompt = "You review code..."
model = "anthropic/claude-sonnet-4-20250514"
allowed_tools = ["file_read", "github_ops"]
```

---

## Data Structure (Rust Schema)

```rust
/// Shared across all channel types — one entry per sub-channel agent
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubchannelAgentConfig {
    /// Platform-specific sub-channel identifier:
    /// - Telegram: Forum topic `message_thread_id` (e.g. "123")
    /// - Discord:  Text channel `channel_id` (e.g. "1234567890")
    /// - Slack:    Channel `channel_id` (e.g. "C1234567890")
    pub subchannel_id: String,

    /// Human-readable name for logs/debugging
    pub name: String,

    /// System prompt override for this sub-channel agent
    pub system_prompt: Option<String>,

    /// Model override (falls back to global default if None)
    pub model: Option<String>,

    /// Provider override (falls back to global default if None)
    pub provider: Option<String>,

    /// Tool allowlist — only these tools are available in this sub-channel.
    /// Empty = inherit all tools from global config (no restriction).
    #[serde(default)]
    pub allowed_tools: Vec<String>,

    /// Temperature override for this agent
    pub temperature: Option<f64>,
}

// Add to TelegramConfig:
pub struct TelegramConfig {
    // ... existing fields ...
    #[serde(default)]
    pub subchannel_agents: Vec<SubchannelAgentConfig>,
}

// Add to DiscordConfig:
pub struct DiscordConfig {
    // ... existing fields ...
    #[serde(default)]
    pub subchannel_agents: Vec<SubchannelAgentConfig>,
}

// Add to SlackConfig:
pub struct SlackConfig {
    // ... existing fields ...
    #[serde(default)]
    pub subchannel_agents: Vec<SubchannelAgentConfig>,
}
```

---

## Routing Logic (in `src/channels/mod.rs`)

Need a function to extract subchannel ID from `ChannelMessage` per platform:

| Platform | Extraction Logic |
|----------|------------------|
| Telegram | `reply_target = "chat_id:thread_id"` → `subchannel_id = "thread_id"` (part after ":") |
| Discord | `reply_target = "channel_id"` → `subchannel_id = reply_target` |
| Slack | `reply_target = "channel_id"` → `subchannel_id = reply_target` |

### Message Processing Flow

```
Receive ChannelMessage
        │
        ▼
Extract subchannel_id by platform
        │
        ▼
Lookup SubchannelAgentConfig by subchannel_id
        │
   ┌────┴────┐
Found?       Not found
   │              │
   ▼              ▼
Override:      Use default
- system_prompt   agent config
- model           (current behavior)
- allowed_tools
- temperature
   │
   ▼
Filter tool registry by allowed_tools
   │
   ▼
Run tool call loop with overridden config
```

---

## Impact Scope & Risk Assessment

| File | Changes | Risk |
|------|---------|------|
| `src/config/schema.rs` | Add `SubchannelAgentConfig`, extend 3 config structs | Low — additive, backward compatible |
| `src/channels/mod.rs` | Fix `conversation_history_key`, add routing lookup | Medium — logic change in hot path |
| `src/channels/telegram.rs` | No changes (routing key already sufficient in `reply_target`) | None |
| `src/channels/discord.rs` | No changes | None |
| `src/channels/slack.rs` | No changes | None |
| `docs/config-reference.md` | Document `subchannel_agents` | Low |

---

## Implementation Order

### Step 1 — Fix History Isolation (independent, safe, do first):
- Fix `conversation_history_key` to include `reply_target`

### Step 2 — Config Schema:
- Add `SubchannelAgentConfig` and wire into `TelegramConfig`, `DiscordConfig`, `SlackConfig`

### Step 3 — Routing in `mod.rs`:
- Extract subchannel ID from message by platform
- Lookup + override agent config
- Filter tool registry by `allowed_tools`

### Step 4 — Docs & Tests:
- Update `config-reference.md`
- Unit tests for routing logic and tool filtering

---

## Notes

- This feature enables multi-agent deployment within a single workspace/group
- Each sub-channel agent operates with complete isolation (history, tools, model)
- Backward compatible: without `subchannel_agents` config, behavior falls back to current default
