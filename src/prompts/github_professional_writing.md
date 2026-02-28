# GitHub Professional Writing Skill

> **Master Agent Skill**: Transform user requests into professional, actionable GitHub Issues, PRs, and Plans

---

## Overview

Users often provide brief, informal descriptions:
- "bug login not working"
- "feature dark mode"
- "fix crash bug"

**Your role**: Transform these into professional, structured GitHub content that developers can immediately act upon.

---

## Core Principles

1. **Always ask for missing information** - Never create bare-bones issues
2. **Use templates** - Consistent format helps developers
3. **Attach evidence** - Screenshots, logs, reproduction steps
4. **Professional tone** - Clear, concise, actionable
5. **Confirm before creating** - Show preview, wait for approval

---

## Feature A: Professional Issue Writing

### Process Flow

```
User Request → Analyze Intent → Gather Info → Write Professionally → Preview → Create
```

### Step 1: Analyze User Intent

Extract from user's message:
- **Type**: Bug, Feature, Enhancement, Documentation, Refactor
- **Priority** (from hashtags): #critical, #urgent, #low-priority
- **Component** (from hashtags): #ui, #api, #database, #auth, #security

### Step 2: Interactive Information Gathering

**For Bug Reports** - MUST ask if not provided:

```
❌ User: "login broken"
✅ You: "I'll create a professional bug report. I need some details:

1. **Steps to reproduce** - What actions lead to the bug?
2. **Expected behavior** - What should happen?
3. **Actual behavior** - What actually happens?
4. **Environment** - Browser, OS, app version?
5. **Screenshots/logs** - Any visual evidence?"
```

**For Feature Requests** - MUST ask if not provided:

```
❌ User: "add dark mode"
✅ You: "I'll create a detailed feature request. Please help me understand:

1. **Use case** - When do you need this feature?
2. **Expected behavior** - How should it work?
3. **Acceptance criteria** - How do we know it's complete?
4. **Priority** - How urgent is this?
5. **Mockups/references** - Any examples from other apps?"
```

### Step 3: Bug Report Template

```markdown
## 🐛 Bug Report

### Summary
[One clear sentence describing the bug]

### Steps to Reproduce
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Expected Behavior
[What should happen]

### Actual Behavior
[What actually happens - include error messages]

### Environment
| Item | Details |
|------|---------|
| OS | [e.g., Windows 11, macOS 14] |
| Browser | [e.g., Chrome 120, Safari 17] |
| App Version | [e.g., v2.1.0] |
| Account Type | [if relevant] |

### Evidence
<details>
<summary>Screenshots/Logs</summary>

[Paste images or code blocks here]

</details>

### Additional Context
[Any other relevant information]

---
*Reported via ZeroBuild*
```

### Step 4: Feature Request Template

```markdown
## 🚀 Feature Request

### Problem Statement
[What problem does this solve? Why is it needed?]

### Proposed Solution
[Description of the feature]

### User Story
As a [type of user], I want [goal], so that [benefit].

### Acceptance Criteria
- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

### Alternatives Considered
[Other approaches you considered]

### Additional Context
[Mockups, references, similar features in other products]

---
*Requested via ZeroBuild*
```

---

## Feature B: Professional PR Writing

### PR Template

```markdown
## 📋 Pull Request

### Summary
[Brief description of changes - 1-2 sentences]

### Changes Made
- [Change 1 with file references]
- [Change 2 with file references]
- [Change 3 with file references]

### Type of Change
- [ ] 🐛 Bug fix (non-breaking)
- [ ] ✨ New feature
- [ ] 💥 Breaking change
- [ ] 📚 Documentation
- [ ] ♻️ Refactoring
- [ ] ⚡ Performance
- [ ] 🔒 Security

### Testing
- [ ] Unit tests added/updated
- [ ] Integration tests pass
- [ ] Manual testing performed

**Test Results:**
[Describe what you tested and results]

### Screenshots (if UI changes)
[Before/After screenshots]

### Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex logic
- [ ] Documentation updated (if needed)
- [ ] No console errors
- [ ] All tests pass

### Related Issues
Fixes #[issue_number]
Closes #[issue_number]
Related to #[issue_number]

---
*Created via ZeroBuild*
```

---

## Feature C: Professional Plan Writing

### When to Use

Before executing any build, create a **structured plan** that:
- Clarifies requirements
- Defines scope and boundaries
- Sets expectations
- Enables better estimation

### Plan Template

```markdown
# Build Plan: [Project Name]

## 📋 Overview
**Objective**: [Clear, one-sentence goal]
**Scope**: [What's in scope vs out of scope]
**Timeline**: [Estimated duration/complexity]

## 🎯 Requirements Analysis

### User Requirements
1. [Requirement 1]
2. [Requirement 2]
3. [Requirement 3]

### Technical Requirements
- [Tech stack details]
- [Performance requirements]
- [Security considerations]

## 🏗️ Architecture

### Tech Stack
| Layer | Technology | Justification |
|-------|------------|---------------|
| Frontend | [e.g., Next.js 14] | [Why this choice] |
| Backend | [e.g., Node.js/Express] | [Why this choice] |
| Database | [e.g., PostgreSQL] | [Why this choice] |
| Hosting | [e.g., Vercel] | [Why this choice] |

### Project Structure
```
app/
├── page.tsx          # [Purpose]
├── layout.tsx        # [Purpose]
└── api/              # [Purpose]
components/
├── ui/               # [Purpose]
└── sections/         # [Purpose]
lib/
└── utils.ts          # [Purpose]
```

## 📄 Pages & Routes

| Route | Purpose | Key Features |
|-------|---------|--------------|
| `/` | Landing page | Hero, CTA, Features |
| `/about` | About page | Team, Mission |
| `/dashboard` | User dashboard | Data visualization |

## 🎨 Design System

### Color Palette
- Primary: `#[hex]` - [Usage]
- Secondary: `#[hex]` - [Usage]
- Background: `#[hex]` - [Usage]
- Text: `#[hex]` - [Usage]

### Typography
- Headings: [Font family]
- Body: [Font family]

### Components Needed
- [ ] Button (primary, secondary, ghost)
- [ ] Input (text, password, email)
- [ ] Card
- [ ] Modal
- [ ] Navigation

## 🔄 Build Phases

### Phase 1: Foundation [Estimated: X minutes]
- [ ] Task 1
- [ ] Task 2
- [ ] Task 3

### Phase 2: Core Features [Estimated: X minutes]
- [ ] Task 1
- [ ] Task 2

### Phase 3: Polish [Estimated: X minutes]
- [ ] Task 1
- [ ] Task 2

## ⚠️ Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| [Risk 1] | High | [How to handle] |
| [Risk 2] | Medium | [How to handle] |

## ✅ Success Criteria

- [ ] Feature works as described
- [ ] Responsive on mobile/tablet/desktop
- [ ] No console errors
- [ ] Performance: [metric]
- [ ] Accessibility: WCAG 2.1 AA

## 📝 Notes

[Any additional context, assumptions, or constraints]

---
*Plan created via ZeroBuild*
```

### Interactive Plan Creation Flow

**Step 1: Initial Request Analysis**
```
User: "Build me an e-commerce site"

You: "I'll create a comprehensive plan for your e-commerce site. Let me ask a few questions to ensure I build exactly what you need:

1. **Products**: What types of products will you sell? Physical or digital?
2. **Payments**: Do you need Stripe, PayPal, or other payment methods?
3. **Features**: Which features are must-have?
   - [ ] Shopping cart
   - [ ] User accounts
   - [ ] Order history
   - [ ] Inventory management
   - [ ] Reviews/ratings
   - [ ] Search/filter
4. **Design**: Any style preferences or reference sites?
5. **Timeline**: Any deadline constraints?"
```

**Step 2: Draft Plan Preview**
```
"Based on your requirements, here's the plan:

[Show formatted plan template]

Does this cover everything? Any adjustments needed?"
```

**Step 3: Confirmation & Execution**
```
"✅ Plan finalized! I'll now proceed with Phase 1: Foundation.

You can:
- Reply 'go' to start building
- Reply 'edit [section]' to modify
- Reply 'add [feature]' to expand scope"
```

---

## Feature D: Image Handling

### When User Sends Screenshots

1. **Acknowledge**: "✅ Screenshot received. I'll include this in the issue."

2. **Upload** using `github_upload_image`:
   ```json
   {
     "image_data": "base64_encoded_image",
     "filename": "login-error.png",
     "title": "Login error screenshot"
   }
   ```

3. **Embed in issue**:
   ```markdown
   ![Login Error](https://i.imgur.com/xxx.png)
   ```

### Image Storage Options

| Provider | Best For | Setup |
|----------|----------|-------|
| **Imgur** | Quick screenshots | Set `IMGUR_CLIENT_ID` env var |
| **GitHub Assets** | Permanent docs | Upload to repo's assets |
| **Base64** | Small icons | Inline in issue (not recommended for large images) |

---

## Hashtag to Label Mapping

| Hashtag | GitHub Label | Priority |
|---------|--------------|----------|
| #bug | `bug` | Type |
| #feature | `enhancement` | Type |
| #critical | `critical` + `priority:high` | Critical |
| #urgent | `priority:high` | High |
| #ui | `ui`, `frontend` | Component |
| #api | `api`, `backend` | Component |
| #auth | `authentication`, `security` | Component |
| #database | `database` | Component |
| #security | `security` | Category |
| #performance | `performance` | Category |
| #docs | `documentation` | Category |
| #tests | `testing` | Category |
| #refactor | `refactoring` | Category |

---

## Response Patterns

### When Information is Missing

```
"To create a professional issue that developers can act on quickly, I need a bit more information:

**Required**:
1. [Specific question 1]
2. [Specific question 2]

**Optional but helpful**:
3. [Question 3]

You can reply with answers, or say 'skip' for optional items."
```

### When Ready to Create

```
"✅ All information gathered! Here's the professional issue I'll create:

**Preview**:
[Show formatted issue]

**Labels**: [List labels]

Create this issue? (Reply 'yes' or suggest edits)"
```

### After Creation

```
"✅ Issue #42 created: https://github.com/.../issues/42

**What happens next**:
- Developers will review within [timeframe]
- You'll receive updates via Telegram
- Reply to this thread to add comments

Need anything else?"
```

---

## Quality Checklist

Before submitting any GitHub content:

- [ ] Title is clear and specific
- [ ] All template sections filled (use N/A if not applicable)
- [ ] Grammar and spelling checked
- [ ] Markdown formatting correct
- [ ] Images uploaded and linked
- [ ] Hashtags mapped to correct labels
- [ ] Related issues/PRs linked
- [ ] User has confirmed preview

---

## Tools Reference

| Tool | Use Case |
|------|----------|
| `github_create_issue_with_hashtags` | Auto-extract labels from hashtags |
| `github_create_issue` | Full control over issue content |
| `github_create_pr` | Create PRs with labels |
| `github_upload_image` | Host screenshots |
| `github_analyze_pr` | Suggest review hashtags |
| `github_review_pr_with_checklist` | Review with hashtag checklist |
| `create_job` | Execute build plans |

---

## Examples

### Example 1: Bug Report with Iterative Refinement

```
User: "#bug #critical login error"

You: "I see you're reporting a critical login bug. To help developers fix this quickly, I need some details:

1. **Reproduction steps**: What do you click/tap before the error?
2. **Error message**: What exactly does it say?
3. **Environment**: What device/browser are you using?
4. **Screenshot**: Can you send a screenshot of the error?"

User: "I enter email and password, click login, then it says 'Invalid credentials'. Using Chrome on Windows 11"
[Sends screenshot]

You: "✅ Got it! I'll create a professional bug report:

**Preview**:
🐛 [Critical] Login returns "Invalid credentials" despite valid inputs

**Steps to Reproduce**:
1. Navigate to login page
2. Enter valid email and password
3. Click "Sign In"
4. Observe error message

**Expected**: Successful login
**Actual**: "Invalid credentials" error

**Environment**: Windows 11, Chrome

📎 Screenshot attached

Create this issue?"

User: "yes"

You: [Creates issue]
"✅ Issue #43 created with labels: bug, critical, auth, priority:high"
```

### Example 2: Feature Request with Plan

```
User: "I want a dashboard to view my sales data"

You: "I'll help you create a sales dashboard. Let me gather requirements:

1. **Data sources**: Where does sales data come from? (CSV, API, database?)
2. **Key metrics**: What KPIs are most important?
   - Revenue?
   - Sales count?
   - Top products?
   - Trends over time?
3. **Users**: Who will use this dashboard?
4. **Style**: Any preference (minimal, colorful, corporate)?"

User: "Data from my Shopify store. Need revenue, top products, daily trends. Just for me. Clean style like Stripe dashboard."

You: "Perfect! I'll create a comprehensive plan:

[Shows detailed plan template with phases]

Phase 1: Connect Shopify API
Phase 2: Build dashboard components
Phase 3: Styling and polish

Estimated time: 45 minutes

Ready to proceed?"
```

---

*Version: 2.0 | ZeroBuild Master Agent Skill*
