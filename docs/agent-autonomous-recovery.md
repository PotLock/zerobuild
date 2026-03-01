# Agent Autonomous Recovery

ZeroBuild is designed to run **without human intervention**. This document
describes exactly what happens at every failure point, which layers recover
automatically, and which failure modes have no current recovery path.

---

## 1. The recovery stack (four independent layers)

```
┌──────────────────────────────────────────────────┐
│  Layer 4 — Agent loop (loop_.rs)                 │
│  Tool errors → history → LLM decides next step  │
├──────────────────────────────────────────────────┤
│  Layer 3 — Provider wrapper (reliable.rs)        │
│  LLM call failures → retry + model failover      │
├──────────────────────────────────────────────────┤
│  Layer 2 — Channel transport (telegram.rs)       │
│  Send failures → format fallback + retry         │
├──────────────────────────────────────────────────┤
│  Layer 1 — Sandbox client (local.rs)             │
│  Process errors → structured output → LLM hint  │
└──────────────────────────────────────────────────┘
```

Each layer operates independently. A failure at Layer 1 does not require Layer 3
to activate. Understanding which layer handles which failure is the key to
reasoning about autonomous behavior.

---

## 2. Layer 1 — Sandbox failures

### What the sandbox does when a command fails

`LocalProcessSandboxClient::run_command` **always** returns `Ok(CommandOutput)`.
It never propagates a hard error upward unless the OS could not spawn the process
at all (extremely rare).

| Scenario | `exit_code` | `stderr` | What the tool returns |
|---|---|---|---|
| Command succeeds | 0 | empty or warnings | `success: true` |
| Command fails (non-zero) | e.g. 1 | npm/node error text | `success: false`, stderr in output |
| Command times out | -1 | "Command timed out after Nms" | `success: false`, timeout message |
| Process cannot be spawned | — | — | `Err(anyhow)` → tool catches it |

**The LLM always receives the stderr output.** A failed `npm install` produces
the full npm error text in the tool result. The LLM uses this to decide whether
to retry, change the command, or ask for input.

### Path validation errors

If the agent calls `sandbox_write_file` or `sandbox_read_file` with a path
containing `..`, `safe_join` rejects it with an explicit error message.
The tool returns `success: false` with the error. The agent loop continues — the
LLM sees the rejection and must use a valid path.

### Sandbox not created yet

If any sandbox tool is called before `sandbox_create`, `require_id()` returns
`Err("No active sandbox. Call sandbox_create first.")`.

The tool (`sandbox_run_command`, `sandbox_write_file`, etc.) catches this and
returns:

```
success: false
error: "No active sandbox. Call sandbox_create first."
error_hint: "🚨 SANDBOX NOT AVAILABLE — call sandbox_create with reset=true"
```

The `error_hint` is explicitly written to instruct the LLM on the exact recovery
step. The agent loop continues; the LLM's next action will typically be to call
`sandbox_create`.

---

## 3. Layer 2 — Channel transport failures

### Telegram send failures

Every outgoing message goes through a two-attempt sequence
(`telegram.rs:1450–1505`):

```
Attempt 1: Send with Markdown/HTML formatting
  ↓ status 2xx → done
  ↓ status 4xx/5xx
Attempt 2: Send as plain text (no parse_mode)
  ↓ status 2xx → done
  ↓ still fails → bail with combined error message
```

If the Telegram API rejects a formatted message (e.g., unmatched bold tags in
generated code), the agent automatically retries as plain text. From the user's
perspective, the message arrives without formatting rather than being lost.

### Media URL fallback

If the agent sends a preview URL or image that Telegram cannot fetch:

```
Attempt 1: Send as media attachment
  ↓ fails (Telegram can't reach the URL)
Fallback: Send the URL as a clickable text link
```

The reply is never silently dropped.

### Telegram polling retry loop

The poll loop (`telegram.rs:2312–2380`) runs forever:

```rust
loop {
    match client.post(...).send().await {
        Err(e) => {
            tracing::warn!("poll error: {e}");
            sleep(5s).await;
            continue;  // ← never exits on network error
        }
        Ok(resp) => { /* process updates */ }
    }
}
```

A network outage, DNS failure, or Telegram service interruption will pause
message delivery for 5-second intervals but will not crash the agent. When the
network recovers, polling resumes automatically.

---

## 4. Layer 3 — LLM provider failures

This is the most critical recovery layer because every agent action depends on
the LLM responding correctly.

### Three-level failover (`reliable.rs:215–782`)

When a provider call fails, the wrapper iterates three nested loops:

```
for each model in [requested_model, fallback_model_1, ...]:
  for each provider in [primary, secondary, ...]:
    for attempt in 0..max_retries:
      try provider.chat(model)
      on retryable error: exponential backoff, continue
      on non-retryable error: break to next provider
```

**Exponential backoff:** starts at `base_backoff_ms` (default ~100 ms), doubles
on each retry, caps at 10 seconds. Respects HTTP `Retry-After` headers.

### Error classification

The wrapper classifies every error before deciding to retry or bail:

| Error type | Action |
|---|---|
| 5xx server error | Retry same provider |
| 429 rate-limit (temporary) | Retry with backoff + API key rotation |
| 408 timeout | Retry same provider |
| Network error | Retry same provider |
| 4xx client error (not 429/408) | Skip to next provider |
| 429 with "insufficient balance" | Skip to next provider (quota exhausted) |
| "invalid api key" / auth failure | Skip to next provider |
| Context window exceeded | **Bail immediately** — no retry at any level |
| Model not found | Skip to next model in fallback chain |

### Circuit breaker (`health.rs:1–150`)

After 5 consecutive failures, a provider's circuit opens:

```
provider fails 5 times
  → circuit OPEN: all new requests to this provider are rejected instantly
  → after cooldown TTL expires: circuit HALF-OPEN (try once)
  → on success: circuit CLOSED, failure count reset
```

This prevents a degraded provider from consuming retry budget and increasing
latency when a healthier provider is available.

### What happens when all providers fail

If every model and every provider have been exhausted:

```rust
anyhow::bail!(
    "All provider attempts failed:\n{failures}"
);
```

The error propagates to the agent loop, which returns it to the channel handler.
The channel (Telegram) catches this and sends the user a message such as:

> ❌ I encountered an error and could not complete your request. Please try again.

This is the only point where the user sees a hard failure.

---

## 5. Layer 4 — The agent tool-call loop

### Tool errors are never fatal

The loop (`loop_.rs:1998–2676`) treats tool results as information, not
exceptions. Whether a tool returns `success: true` or `success: false`, the
result is added to the conversation history and the loop continues.

```
iteration N:
  LLM call → tool calls listed
  for each tool call:
    execute tool
    if success:  add result to history
    if failure:  add error + error_hint to history
  loop → iteration N+1: LLM sees full error context, decides next step
```

### Consecutive failure escalation

The loop tracks how many times the same tool fails in a row
(`loop_.rs:2032–2033`). After 3 consecutive failures:

```
"[ESCALATION] I'm having trouble with this step.
Would you like me to try a different approach?"
```

This message is appended to the tool result in history. On the next LLM call,
the model sees it and typically switches strategy: different command, different
workdir, different approach entirely.

### Max iterations guard

```rust
const DEFAULT_MAX_TOOL_ITERATIONS: usize = 10; // (configurable)
```

If the agent does not produce a final answer within `max_iterations` rounds, the
loop bails with a diagnostic message. This prevents infinite loops where the LLM
keeps calling the same failing tool.

The Telegram channel handler catches this and informs the user.

### Global turn timeout (`channels/mod.rs:1744–1772`)

Every agent turn is wrapped by an **outer** `tokio::time::timeout` at the
channel level, independent of any per-tool timeout:

```rust
let timeout_budget_secs =
    channel_message_timeout_budget_secs(ctx.message_timeout_secs, ctx.max_tool_iterations);

let llm_result = tokio::select! {
    () = cancellation_token.cancelled() => LlmExecutionResult::Cancelled,
    result = tokio::time::timeout(
        Duration::from_secs(timeout_budget_secs),
        run_tool_call_loop(...)
    ) => LlmExecutionResult::Completed(result),
};
```

The budget is proportional to `max_tool_iterations` (default 300 s / 5 min per
message). If the entire turn — including all LLM calls and all tool executions —
has not finished within that budget, the future is dropped, the sandbox process
is killed via `kill_on_drop`, and the user receives an error message.

This ensures no agent turn can hang indefinitely regardless of what individual
tools do.

### History trimming and auto-compaction (`loop_/history.rs:17–33`)

Before every LLM call, the history is trimmed to `max_history_messages`
(default: 50, configurable in `config.toml`):

```rust
// 1. Auto-compact: LLM summarises oldest messages into a compact summary
if let Ok(compacted) = auto_compact_history(..., config.agent.max_history_messages).await {
    ...
}
// 2. Hard trim: drop oldest non-system messages if still over limit
trim_history(&mut history, config.agent.max_history_messages);
```

Auto-compaction runs first — it calls the LLM to summarise the oldest portion of
the history into a single compact message, preserving intent without burning
tokens. If the history is still too long after compaction, the hard trim removes
the oldest entries.

This means the context window exhaustion scenario described in prior analysis
is **prevented proactively**, not just handled reactively.

### Cancellation

Each iteration checks the cancellation token before calling the LLM:

```rust
if cancellation_token.is_some_and(CancellationToken::is_cancelled) {
    return Err(ToolLoopCancelled.into());
}
```

A user can interrupt a running task mid-execution. The agent stops cleanly
without leaving orphaned processes (the sandbox's `kill_on_drop` handles that).

---

## 6. Failure scenario walkthrough

### Scenario A — npm install fails inside sandbox

```
1. Agent calls sandbox_run_command("npm install")
2. npm exits with code 1, stderr: "ERESOLVE could not resolve..."
3. Tool returns success=false, exit_code=1, stderr captured
4. Loop adds result to history, continues
5. LLM sees error, calls sandbox_run_command("npm install --legacy-peer-deps")
6. npm succeeds → loop continues
```
**Outcome:** Fully automatic. No human needed.

### Scenario B — Sandbox not created before command

```
1. Agent calls sandbox_run_command without calling sandbox_create first
2. require_id() returns Err("No active sandbox")
3. Tool returns success=false, error_hint="call sandbox_create with reset=true"
4. LLM sees hint, calls sandbox_create(reset=false)
5. Sandbox created, loop continues
```
**Outcome:** Fully automatic. The explicit error_hint guides the LLM.

### Scenario C — Rate limit from LLM provider

```
1. Agent calls LLM → 429 Too Many Requests
2. reliable.rs extracts Retry-After header (e.g. 30s)
3. sleep(30s)
4. Retry same provider → success
5. Loop continues without any visible interruption to the agent
```
**Outcome:** Fully automatic. Agent resumes after backoff.

### Scenario D — Primary LLM provider goes down

```
1. Agent calls primary provider → 503 five times
2. Circuit breaker opens for primary provider
3. Wrapper tries secondary provider → success
4. Agent continues with secondary provider
5. Primary provider recovers → circuit half-opens → next success resets it
```
**Outcome:** Fully automatic. Seamless failover.

### Scenario E — All LLM providers down simultaneously

```
1. All providers and all fallback models exhausted
2. reliable.rs bails with "All provider attempts failed: ..."
3. agent loop catches Err, channel handler catches it
4. Telegram: "❌ I encountered an error. Please try again."
5. Agent is ready for the next user message immediately
```
**Outcome:** User sees one error message. Agent recovers for next request.
No restart needed.

### Scenario F — History grows large over a long build session

```
1. Agent accumulates many tool results across 30+ rounds
2. Before next LLM call, trim_history() detects history > max_history_messages (50)
3. auto_compact_history() summarises oldest messages via LLM call
4. Hard trim removes any remaining excess
5. LLM call proceeds with a history that fits within context limits
```
**Outcome:** Fully automatic. The user never sees a context error under normal
operation. If the per-message history trimming were somehow bypassed and the
model still returned `context_length_exceeded`, `reliable.rs` classifies it as
NON-RETRYABLE and the channel sends one error message.

---

## 7. Known gaps (failure modes with no automatic recovery)

These are failure modes where the current system has no autonomous recovery path.
Gap 1 (global watchdog) and Gap 2 (context window) from earlier analysis are
**already handled** in code — see §5 above.

### Gap 1 — Orphaned grandchild processes after timeout

When `tokio::time::timeout` fires, the direct `sh -c ...` child is killed via
`kill_on_drop`. However, if that shell spawned background children that called
`setsid()` to detach from the process group (e.g., `npm run dev` forking a
Node.js server), those grandchildren are **not killed**.

- **Code:** `src/sandbox/local.rs` — `kill_on_drop(true)` only on the direct
  child; no `killpg()` or negative-PID kill.
- **Impact:** Leaked processes on the host that continue running after the
  sandbox is killed. Port 3000 may remain occupied.
- **Mitigation today:** `kill_sandbox()` removes the sandbox directory, which
  cleans artefacts. The orphaned process eventually dies when it can no longer
  write to its working directory.

### Gap 2 — Vague error hint on sandbox_create failure

If `sandbox_create` fails (e.g., `$TMPDIR` is full or the OS rejects the
`mkdir`), the tool returns `success: false` with:

```
error_hint: "Sandbox creation failed. STOP: Do not proceed with file_write
             or shell. Fix the sandbox issue first."
```

- **Code:** `src/tools/sandbox/create.rs:99–101`
- **Impact:** The hint gives no actionable recovery step. The LLM may retry
  immediately and fail the same way, burning iteration budget.
- **Mitigation today:** The LLM typically calls `sandbox_create(reset=true)` on
  the second attempt. If the disk is genuinely full, all retries will fail and
  `max_tool_iterations` eventually terminates the turn.

---

## 8. Configuration levers for autonomous operation

| Config key | Default | Source | Purpose |
|---|---|---|---|
| `agent.max_tool_iterations` | 10 | `config/schema.rs` | Max LLM + tool rounds per turn |
| `agent.max_history_messages` | 50 | `config/schema.rs` | Trim + compact history before context overflow |
| `channel.message_timeout_secs` | 300 | `channels/mod.rs` | Global wall-clock limit for the entire turn |
| `providers.max_retries` | 3 | `providers/reliable.rs` | Retry attempts per (provider, model) pair |
| `providers.base_backoff_ms` | 100 | `providers/reliable.rs` | Initial backoff; doubles per retry, caps at 10 s |
| `runtime.reasoning_enabled` | false | `config/schema.rs` | Enable extended thinking for hard problems |

For a fully unattended deployment handling complex multi-file builds:

```toml
[agent]
max_tool_iterations = 30      # long builds may need 20+ rounds
max_history_messages = 60     # keep enough context for the full build

[providers]
max_retries = 5
base_backoff_ms = 200
```

---

## 9. Summary

| Failure | Recovers? | Who handles it | User sees it? |
|---|---|---|---|
| Command non-zero exit | ✅ Auto | LLM retries via history | No |
| Command timeout | ✅ Auto | Tool returns -1, LLM adapts | No |
| Sandbox not created | ✅ Auto | error_hint guides LLM | No |
| Path traversal attempt | ✅ Auto | safe_join rejects, LLM corrects | No |
| Telegram format rejected | ✅ Auto | Retry as plain text | No (gets plain msg) |
| Telegram network down | ✅ Auto | Poll retries every 5s | Delayed delivery |
| LLM rate-limited | ✅ Auto | Backoff + retry | Delayed response |
| LLM provider down | ✅ Auto | Circuit breaker + failover | Slower response |
| History too long | ✅ Auto | Auto-compact + hard trim | No |
| Turn hangs forever | ✅ Auto | Global turn timeout kills it | Yes — 1 error msg |
| All providers down | ❌ Hard stop | Channel sends error | Yes — 1 error msg |
| Orphaned grandchild process | ⚠️ Partial | Directory cleaned, process may linger | No |
| sandbox_create fails (disk full) | ⚠️ Partial | LLM retries blindly until max_iterations | Yes — if all fail |

ZeroBuild recovers from virtually all transient failures autonomously. The two
remaining partial gaps (orphaned grandchildren, vague sandbox_create hint) have
no user-visible impact under normal operating conditions. The only failure that
reliably surfaces to the user is total provider unavailability — all configured
LLM providers and fallback models exhausted simultaneously.
