# GitHub Professional Writing Skill

> **ZeroBuild Agent Skill**: Transform user requests into professional, actionable GitHub Issues and Pull Requests

---

## Mandatory Rules (Non-negotiable)

1. **English only** — All issue titles, bodies, PR titles, bodies, review comments, and closing comments MUST be written in English, regardless of the language the user spoke.
2. **Bracketed type prefix** — Issue titles MUST begin with a bracketed type: `[Feature]:`, `[Bug]:`, `[Chore]:`, `[Docs]:`, `[Security]:`, `[Refactor]:`, `[Test]:`, or `[Perf]:`. Never use "Feature Request:", "Bug Report:", or similar verbose forms.
3. **Labels required** — Every issue and PR MUST have at least one label applied at creation time. Never create unlabeled issues or PRs.
4. **Confirm before creating** — Show the user a preview and wait for approval before calling any create/edit tool.

---

## Issue Title Format

| Type | Prefix | Example |
|------|--------|---------|
| New capability | `[Feature]:` | `[Feature]: Add Stripe payment integration` |
| Defect / regression | `[Bug]:` | `[Bug]: Login returns 500 on invalid credentials` |
| Maintenance / tooling | `[Chore]:` | `[Chore]: Upgrade reqwest to v0.12` |
| Documentation | `[Docs]:` | `[Docs]: Add quickstart guide for Docker setup` |
| Security hardening | `[Security]:` | `[Security]: Enforce HTTPS on all API routes` |
| Code restructuring | `[Refactor]:` | `[Refactor]: Extract token resolution into shared helper` |
| Test coverage | `[Test]:` | `[Test]: Add integration tests for GitHub OAuth flow` |
| Speed / efficiency | `[Perf]:` | `[Perf]: Cache GitHub token lookup to reduce DB reads` |

---

## Label Reference

### Type labels (at least one required on every issue and PR)

| Label | When to use |
|-------|-------------|
| `feature` | New functionality |
| `bug` | Defect or regression |
| `chore` | Maintenance, deps, tooling |
| `docs` | Documentation changes |
| `security` | Security hardening or vulnerability |
| `refactor` | Code restructuring, no behavior change |
| `test` | Test coverage or test infra |
| `perf` | Performance improvement |

### Scope labels (optional, add when relevant)

`provider`, `channel`, `tool`, `gateway`, `memory`, `runtime`, `config`, `ci`, `peripheral`

### PR-specific labels (required for PRs)

- **Size**: `size: XS`, `size: S`, `size: M`, `size: L`, `size: XL`
- **Status**: `needs-review`, `blocked` (add when appropriate)
- **Impact**: `breaking-change` (add when the PR breaks a public API, config key, or user-facing behavior)

---

## Issue Template (Required)

Use this structure for every issue. Fill all sections; write "N/A — [reason]" if a section genuinely does not apply.

```markdown
## Summary
[One paragraph: what this issue is about and why it matters.]

## Problem Statement
[Describe the current behavior, gap, or pain point. For bugs: include exact reproduction steps and error messages.]

## Proposed Solution
[For features: what the new behavior should look like. For bugs: what correct behavior looks like.]

## Non-goals / Out of Scope
- [Explicitly list what this issue will NOT address.]

## Alternatives Considered
- [Alternatives evaluated and why they were not chosen.]

## Acceptance Criteria
- [ ] [Concrete, testable condition 1]
- [ ] [Concrete, testable condition 2]

## Architecture Impact
- Affected subsystems: [list modules, traits, tools, or channels impacted]
- New dependencies: [none or list]
- Config/schema changes: [yes/no — if yes, describe]

## Risk and Rollback
- Risk: [low / medium / high — and why]
- Rollback: [how to revert if the fix or feature causes a regression]

## Breaking Change?
- [ ] Yes — describe impact and migration path
- [ ] No

## Data Hygiene Checks
- [ ] I removed personal/sensitive data from examples, payloads, and logs.
- [ ] I used neutral, project-focused wording and placeholders.
```

---

## PR Template (Required)

Use this structure for every pull request.

```markdown
## Summary
[One paragraph: what this PR does and why.]

## Problem
[What broken/missing behavior or gap does this PR address?]

## Root Cause
[For bug fixes: what was the underlying cause? For features: what need or gap drove this?]

## Changes
- [Concrete change 1 — module / file / behavior]
- [Concrete change 2]

## Validation
- [ ] `cargo fmt --all -- --check` passed
- [ ] `cargo clippy --all-targets -- -D warnings` passed
- [ ] `cargo test` passed
- [ ] Manual test / scenario: [describe]
- [ ] Docker CI (`./dev/ci.sh all`) run: [yes / no / skipped — if skipped, explain]

## Automation / Workflow Notes
[Any CI/CD, workflow, label-routing, or automation changes introduced or affected by this PR. Write "None" if not applicable.]

## Scope
- Affected subsystems: [list]
- Files changed: [count or list key files]

## Non-goals
- [Explicitly list what this PR does NOT change.]

## Risk
- Risk tier: [low / medium / high]
- Blast radius: [which subsystems or users could be affected by a regression]

## Rollback
- Revert strategy: [`git revert <commit>` or specific steps]
- Migration needed on rollback: [yes / no — if yes, describe]
```

---

## Information Gathering

### For bug issues — ask if not provided

```
I'll create a professional bug report. To help developers reproduce and fix this quickly, I need:

1. **Steps to reproduce** — What exact actions lead to the bug?
2. **Expected behavior** — What should happen?
3. **Actual behavior** — What actually happens? Include error messages verbatim.
4. **Environment** — OS, browser/client version, app version?
5. **Evidence** — Screenshots, logs, or stack traces?
```

### For feature issues — ask if not provided

```
I'll create a detailed feature issue. Please help me understand:

1. **Use case** — When and why do you need this feature?
2. **Proposed behavior** — How should it work?
3. **Acceptance criteria** — How do we know it's complete?
4. **Priority** — How urgent is this?
5. **References** — Examples from other tools or prior discussions?
```

---

## Tool Reference

| Tool | Use case |
|------|----------|
| `github_create_issue` | Create a new issue with full control over content |
| `github_create_issue_with_hashtags` | Create issue and auto-extract labels from hashtags |
| `github_edit_issue` | Update an existing issue's title, body, labels, or state — do not close and recreate |
| `github_close_issue` | Close an issue with a required English resolution comment |
| `github_create_pr` | Create a pull request with labels |
| `github_review_pr` | Submit a review (approve / request changes / comment) |
| `github_review_pr_with_checklist` | Review using hashtag checklist format |
| `github_analyze_pr` | Suggest review categories for a PR |
| `github_list_issues` | List open or closed issues |
| `github_list_prs` | List open or closed pull requests |
| `github_get_issue` | Get full details for a specific issue |
| `github_get_pr` | Get full details for a specific PR |
| `github_upload_image` | Upload a screenshot to Imgur for embedding in issues/PRs |
| `create_job` | Execute build plans |

---

## Closing Issues

When closing an issue always:
1. Call `github_close_issue` with a clear English `comment` explaining the outcome.
2. Set `reason` to `"completed"` (fixed / done) or `"not_planned"` (won't fix / out of scope / duplicate).

Resolution comment examples:
- Fixed: `"Fixed in #42 — the token validation logic now rejects expired tokens correctly."`
- Won't fix: `"Out of scope for this milestone. The current architecture does not support dynamic provider loading. Tracked in the roadmap as a future enhancement."`
- Duplicate: `"Duplicate of #10, which tracks the same authentication regression. Please follow #10 for updates."`

---

## Hashtag to Label Mapping

| Hashtag | GitHub label |
|---------|--------------|
| `#bug` | `bug` |
| `#feature` | `feature` |
| `#chore` | `chore` |
| `#docs` | `docs` |
| `#security` | `security` |
| `#refactor` | `refactor` |
| `#test` | `test` |
| `#perf` | `perf` |
| `#critical` | `priority: critical` |
| `#urgent` | `priority: high` |
| `#ui` | `frontend` |
| `#api` | `backend` |
| `#auth` | `authentication` |
| `#database` | `database` |

---

## Quality Checklist

Before submitting any GitHub content:

- [ ] Title uses the correct bracketed prefix (`[Feature]:`, `[Bug]:`, etc.)
- [ ] Title and all body sections are written in English
- [ ] At least one type label applied
- [ ] All template sections filled (or explicitly marked N/A)
- [ ] Data hygiene checks completed
- [ ] User has confirmed the preview

---

## Response Patterns

### When ready to create

```
Here is the issue I will create:

**Title**: [Feature]: Add Stripe payment integration
**Labels**: feature, backend, size: M

**Preview**:
[formatted issue body]

Create this issue? (Reply yes or suggest edits)
```

### After creation

```
Issue #42 created: https://github.com/.../issues/42
Labels applied: feature, backend

What happens next:
- Developers will review and triage
- Reply to this thread to add comments or screenshots
```

### After closing

```
Issue #17 closed: https://github.com/.../issues/17
Reason: completed
Resolution comment posted.
```

---

*Version: 3.0 | ZeroBuild Agent Skill*
