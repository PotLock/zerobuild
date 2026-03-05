# Fix Plan Step Bugs - Implementation Guide

## Overview

This branch fixes bugs in the mandatory plan-before-execute feature (merged in PR #37). The feature currently has several issues that prevent it from working correctly across all channels.

## Current Implementation (in src/agent/agent.rs)

The plan step is triggered in `turn()` method when:
- First tool iteration (iteration == 0)
- Has write operations detected

Current flow:
1. User sends request: "build a coffee cat website"
2. Agent detects write operations in tool calls
3. Agent calls `generate_plan_with_llm()` to create plan
4. Plan is printed via `println!()` and `tracing::info!()`
5. Agent returns plan message immediately without executing tools
6. On next user message, agent should check for approval

## Bugs to Fix

### Bug 1: Plan Not Displaying in Non-CLI Channels

**Problem:**
When using Signal, WhatsApp, or other non-CLI channels, the plan is generated but not visible to the user. The agent shows "Processing..." then immediately executes tools without showing the plan.

**Current Code:**
```rust
println!("\n📋 Plan:\n{}", plan);
println!("\nReply 'yes' to proceed or tell me what to change.");
```

**Why it fails:**
- `println!()` only outputs to stdout, not to Signal/WhatsApp channels
- The plan is not returned as part of the response message

**Expected Fix:**
- Plan must be returned as assistant message content
- User should see the plan in their Signal/WhatsApp chat
- Format: Clear, emoji-enhanced plan with approval prompt

**Acceptance Criteria:**
- [ ] Plan visible in Signal channel
- [ ] Plan visible in WhatsApp channel
- [ ] Plan visible in Telegram channel
- [ ] Plan visible in CLI mode

### Bug 2: User Confirmation Flow Not Implemented

**Problem:**
After showing the plan, the agent doesn't properly wait for or process user confirmation. Currently it returns the plan but doesn't track approval state.

**Current Code:**
```rust
return Ok(format!("📋 Plan:\n{}\n\nReply 'yes' to proceed or tell me what to change.", plan));
```

**Why it fails:**
- No state tracking for pending plan approval
- On next user message, agent doesn't check if previous was a plan awaiting approval
- No logic to handle "yes", "ok", "build it", "stop", "cancel"

**Expected Fix:**
Need a state machine approach:

1. **Add pending plan state to Agent struct:**
```rust
struct Agent {
    // ... existing fields
    pending_plan: Option<PendingPlan>,  // New field
}

struct PendingPlan {
    original_request: String,
    plan_text: String,
    timestamp: Instant,
}
```

2. **Modify turn() to check for pending plan:**
```rust
pub async fn turn(&mut self, user_message: &str) -> Result<String> {
    // Check if we have a pending plan awaiting approval
    if let Some(pending) = self.pending_plan.take() {
        match user_message.to_lowercase().as_str() {
            "yes" | "y" | "ok" | "sure" | "build it" | "go" => {
                // User approved - continue with execution
                // Store approval in history
                self.history.push(ConversationMessage::Chat(
                    ChatMessage::assistant("Plan approved. Starting execution...".to_string())
                ));
                // Continue with tool execution...
            }
            "no" | "n" | "cancel" | "stop" | "abort" => {
                // User rejected
                self.history.push(ConversationMessage::Chat(
                    ChatMessage::assistant("Plan cancelled. No changes made.".to_string())
                ));
                return Ok("Plan cancelled. You can request something else.".to_string());
            }
            _ => {
                // User wants to modify plan - treat as new request
                // Could regenerate plan with modifications
                let modified_prompt = format!("{original_request}\n\nUser wants to modify: {user_message}", 
                    original_request = pending.original_request
                );
                // Generate new plan...
            }
        }
    }
    
    // Normal flow...
}
```

3. **Handle first-time plan generation:**
```rust
// On first write iteration
if iteration == 0 && self.has_write_operations(&calls) && self.pending_plan.is_none() {
    let plan = self.generate_plan_with_llm(user_message).await?;
    self.pending_plan = Some(PendingPlan {
        original_request: user_message.to_string(),
        plan_text: plan.clone(),
        timestamp: Instant::now(),
    });
    return Ok(format_plan_message(&plan));
}
```

**Acceptance Criteria:**
- [ ] User can approve with: "yes", "y", "ok", "sure", "build it", "go"
- [ ] User can reject with: "no", "n", "cancel", "stop", "abort"
- [ ] User can request modifications (triggers plan regeneration)
- [ ] After approval, agent continues with tool execution
- [ ] After rejection, agent clears state and waits for new request

### Bug 3: Plan Generation Error Handling

**Problem:**
If `generate_plan_with_llm()` fails, the agent doesn't have a fallback. It should gracefully handle errors.

**Current Code:**
```rust
let plan = self.generate_plan_with_llm(user_message).await?;
```

**Why it fails:**
- If LLM call fails, the ? operator returns early with error
- User sees error instead of fallback behavior

**Expected Fix:**
```rust
let plan = match self.generate_plan_with_llm(user_message).await {
    Ok(plan) => plan,
    Err(e) => {
        tracing::warn!("Failed to generate plan: {}, using fallback", e);
        // Fallback: Use tool calls to describe what we'll do
        self.generate_simple_plan_from_tools(&calls)
    }
};
```

Add `generate_simple_plan_from_tools()` method that creates a basic plan from the tool calls without calling LLM.

**Acceptance Criteria:**
- [ ] If LLM fails, fallback to tool-based plan
- [ ] User always sees a plan before execution
- [ ] Error is logged but not shown to user

### Bug 4: Plan Not Added to Conversation History

**Problem:**
The plan is printed but not stored in the conversation history properly.

**Current Code:**
The plan is returned as string but not added to `self.history` before returning.

**Expected Fix:**
```rust
// Add plan to history before returning
let plan_message = format!("📋 Plan:\n{}\n\nReply 'yes' to proceed or tell me what to change.", plan);
self.history.push(ConversationMessage::Chat(
    ChatMessage::assistant(plan_message.clone())
));
self.pending_plan = Some(PendingPlan { /* ... */ });
return Ok(plan_message);
```

**Acceptance Criteria:**
- [ ] Plan appears in conversation history
- [ ] Plan visible when viewing conversation log
- [ ] Plan included in context for subsequent messages

### Bug 5: Test Failures

**Problem:**
Some agent tests may fail because the plan step changes the expected flow.

**Expected Fix:**
Update tests in `src/agent/tests.rs`:

1. **Tests with write operations** should expect plan step:
```rust
#[tokio::test]
async fn test_write_operation_shows_plan() {
    // Setup agent with write tools
    // Send request that triggers write
    // Expect plan message returned (not tools executed yet)
    // Send "yes"
    // Expect tools to execute
}
```

2. **Tests with read-only operations** should skip plan:
```rust
#[tokio::test]
async fn test_read_only_skips_plan() {
    // Send request with only file_read
    // Expect immediate execution (no plan)
}
```

3. **Test user approval flow:**
```rust
#[tokio::test]
async fn test_user_approval_yes() {
    // Send build request
    // Get plan
    // Send "yes"
    // Verify execution continues
}

#[tokio::test]
async fn test_user_rejection() {
    // Send build request
    // Get plan
    // Send "cancel"
    // Verify no execution, state cleared
}
```

**Acceptance Criteria:**
- [ ] All existing tests pass
- [ ] New tests for plan approval flow
- [ ] New tests for rejection flow
- [ ] New tests for modification flow

## Implementation Order

1. **Phase 1: Fix plan display** (Bug 1)
   - Modify return value to include plan as assistant message
   - Ensure plan visible in all channels

2. **Phase 2: Add pending plan state** (Bug 2, 4)
   - Add `pending_plan` field to Agent struct
   - Modify turn() to check for pending approval
   - Handle yes/no/modify responses

3. **Phase 3: Error handling** (Bug 3)
   - Add fallback plan generation
   - Add proper error logging

4. **Phase 4: Tests** (Bug 5)
   - Update existing tests
   - Add new tests for plan flow

## Testing Strategy

### Manual Testing Checklist:

- [ ] **CLI Mode:**
  - Request: "build a landing page"
  - Verify: Plan displayed with emoji
  - Reply: "yes"
  - Verify: Execution starts

- [ ] **Signal Channel:**
  - Request: "build a landing page"
  - Verify: Plan message received
  - Reply: "yes"
  - Verify: Execution starts

- [ ] **Rejection Flow:**
  - Request: "build a landing page"
  - Reply: "cancel"
  - Verify: "Plan cancelled" message
  - Verify: No sandbox created

- [ ] **Read-Only Flow:**
  - Request: "explain this code"
  - Verify: No plan, immediate response

### Automated Tests:

```bash
# Run agent tests
cargo test --lib agent::tests

# Run with output visible
cargo test --lib agent::tests -- --nocapture

# Specific test
cargo test --lib agent::tests::test_plan_approval
```

## Code Structure

### Files to Modify:

1. **src/agent/agent.rs**
   - Add `pending_plan` field to Agent struct
   - Add `PendingPlan` struct
   - Modify `turn()` method
   - Add `generate_simple_plan_from_tools()` method
   - Add helper `format_plan_message()`

2. **src/agent/mod.rs**
   - Export new types if needed

3. **tests/** (if new tests needed)
   - Add plan flow tests

### Key Methods:

```rust
impl Agent {
    /// Generate plan using LLM
    async fn generate_plan_with_llm(&mut self, user_message: &str) -> Result<String>
    
    /// Generate simple plan from tool calls (fallback)
    fn generate_simple_plan_from_tools(&self, calls: &[ParsedToolCall]) -> String
    
    /// Check if pending plan needs approval
    fn has_pending_plan(&self) -> bool
    
    /// Handle user response to plan
    async fn handle_plan_response(&mut self, user_response: &str) -> Result<PlanAction>
    
    /// Format plan for display
    fn format_plan_message(plan: &str) -> String
}

enum PlanAction {
    Approve,      // User said yes
    Reject,       // User said no/cancel
    Modify(String), // User wants changes
}
```

## References

- Original Issue: #38
- Original PR: #37
- Related: AGENTS.md section 5.2

## Notes for Agent

When fixing these bugs:
1. Keep changes minimal and focused
2. Don't break existing functionality
3. Add comprehensive tests
4. Update AGENTS.md if behavior changes
5. Run full test suite before committing
6. Test manually with both CLI and Signal channels
