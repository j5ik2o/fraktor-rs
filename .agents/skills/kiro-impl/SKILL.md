---
name: kiro-impl
description: Implement approved tasks using TDD with subagent dispatch. Runs all pending tasks autonomously or selected tasks manually.
---


# kiro-impl Skill

<background_information>
You operate in two modes:
- **Autonomous mode** (no task numbers): Dispatch a fresh sub-agent per task, with independent review after each
- **Manual mode** (task numbers provided): Execute selected tasks directly in the main context

- **Success Criteria**:
  - All tests written before implementation code
  - Code passes all tests with no regressions
  - Tasks marked as completed in tasks.md
  - Implementation aligns with design and requirements
  - Independent reviewer approves each task before completion
</background_information>

<instructions>

## Step 1: Gather Context

If steering/spec context is already available from conversation, skip redundant file reads.
Otherwise, load all necessary context:
- `.kiro/specs/{feature}/spec.json`, `requirements.md`, `design.md`, `tasks.md`
- Core steering context: `product.md`, `tech.md`, `structure.md`
- Additional steering files only when directly relevant to the selected task's boundary, runtime prerequisites, integrations, domain rules, security/performance constraints, or team conventions that affect implementation or validation
- Relevant local agent skills or playbooks only when they clearly match the task's host environment or use case; read the specific artifact(s) you need, not entire directories

### Parallel Research

The following research areas are independent and can be executed in parallel:
1. **Spec context loading**: spec.json, requirements.md, design.md, tasks.md
2. **Steering, playbooks, & patterns**: Core steering, task-relevant extra steering, matching local agent skills/playbooks, and existing code patterns

After all parallel research completes, synthesize implementation brief before starting.

### Preflight

**Validate approvals**:
- Verify `.kiro/specs/$1/spec.json`, `requirements.md`, `design.md`, and `tasks.md` all exist before reading approvals or building the task queue. If any are missing, stop and report the missing files.
- Verify tasks are approved in spec.json (stop if not, see Safety & Fallback)

**Discover validation commands**:
- Inspect repository-local sources of truth in this order: project scripts/manifests (`package.json`, `pyproject.toml`, `go.mod`, `Cargo.toml`, app manifests), task runners (`Makefile`, `justfile`), CI/workflow files, existing e2e/integration configs, then `README*`
- Derive a canonical validation set for this repo: `TEST_COMMANDS`, `BUILD_COMMANDS`, and `SMOKE_COMMANDS`
- Prefer commands already used by repo automation over ad hoc shell pipelines
- For `SMOKE_COMMANDS`, choose the lightest trustworthy runtime-liveness check for the app shape (for example: root URL load, Electron launch, CLI `--help`, service health endpoint, mobile simulator/e2e harness if one already exists)
- Keep the full command set in the parent context, and pass only the task-relevant subset to implementer and reviewer sub-agents

**Establish repo baseline**:
- Run `git status --porcelain` and note any pre-existing uncommitted changes

## Step 2: Select Tasks & Determine Mode

**Parse arguments**:
- Extract feature name from `$1`
- If task numbers provided in `$2` (e.g., "1.1" or "1,2,3"): **manual mode**
- If no task numbers: **autonomous mode** (all pending tasks)

**Build task queue**:
- Read tasks.md, identify actionable sub-tasks (X.Y numbering like 1.1, 2.3)
- Major tasks (1., 2.) are grouping headers, not execution units
- Treat `- [ ]` as a required pending task and `- [ ]*` as a deferred optional task. Autonomous mode selects required pending tasks only; optional tasks run only when explicitly selected by task number.
- Skip tasks with `_Blocked:_` annotation
- For each selected task, check `_Depends:_` annotations -- a task is actionable only when every referenced task is currently `[x]`
- Preserve document-order dependencies: in autonomous mode, a task is actionable only when every earlier required sub-task in `tasks.md` is already `[x]`. Pick the earliest actionable required task; do not skip ahead just because `_Depends:_` is absent.
- If prerequisites are incomplete, execute prerequisite tasks first when they are in scope; otherwise leave the downstream task pending and report it as blocked
- Use `_Boundary:_` annotations to understand the task's component scope

## Step 3: Execute Implementation

### Autonomous Mode (sub-agent dispatch)

**Iteration discipline**: Process exactly ONE sub-task (e.g., 1.1) per iteration. Do NOT batch multiple sub-tasks into a single sub-agent dispatch. Each iteration follows the full cycle: dispatch implementer → review → verify → record notes → commit → re-read tasks.md → next.

**Context management**: At the start of each iteration, re-read `tasks.md` to determine the next actionable sub-task. A task is eligible only if it is unchecked, required (`- [ ]`), has no `_Blocked:_` annotation, every earlier required sub-task in document order is `[x]`, and every `_Depends:_` reference is currently `[x]`. Do NOT rely on accumulated memory of previous iterations. If no eligible required task remains but required unchecked or blocked tasks still exist, stop and report those tasks instead of continuing to final validation. Ignore deferred optional `- [ ]*` tasks for autonomous eligibility unless the user explicitly selected them. After completing each iteration, retain only a one-line summary (e.g., "1.1: READY_FOR_REVIEW, 3 files changed") and discard the full status report and reviewer details.

If multi-agent capability is available, for each task (one at a time):

**a) Dispatch implementer**:
- Initialize `review_rejection_count = 0` only when starting a different task ID. On retries, re-dispatches, and debug loops for the same task ID, preserve the existing counter. Do not carry the counter into the next task.
- Read `templates/implementer-prompt.md` from this skill's directory
- Construct a prompt by combining the template with task-specific context:
  - Task description and boundary scope
  - Paths to spec files: requirements.md, design.md, tasks.md
  - Exact requirement and design section numbers this task must satisfy (using source numbering, NOT invented `REQ-*` aliases)
  - Task-relevant steering context and parent-discovered validation commands (tests/build/smoke as relevant)
  - Whether the task is behavioral (Feature Flag Protocol) or non-behavioral
  - **Previous learnings**: Include any `## Implementation Notes` entries from tasks.md that are relevant to this task's boundary or dependencies (e.g., "better-sqlite3 requires separate rebuild for Electron"). This prevents the same mistakes from recurring.
- The implementer sub-agent will read the spec files and build its own Task Brief (acceptance criteria, completion definition, design constraints, verification method) before implementation
- Spawn a fresh sub-agent with this prompt

**b) Handle implementer status**:
- Parse implementer status only from the exact `## Status Report` block and `- STATUS:` field.
- If `STATUS` is missing, ambiguous, or replaced with prose, re-dispatch the implementer once requesting the exact structured status block only. If the second response is still unparseable, dispatch the debug subagent with root cause `HANDOFF_PARSE_FAILURE`; do not proceed to review without a parseable `READY_FOR_REVIEW | BLOCKED | NEEDS_CONTEXT` value.
- **READY_FOR_REVIEW** → proceed to review
- **BLOCKED** → dispatch debug subagent (see section below); do NOT immediately skip
- **NEEDS_CONTEXT** → re-dispatch once with the requested additional context; if still unresolved → dispatch debug subagent

**c) Dispatch reviewer**:
- Read `templates/reviewer-prompt.md` from this skill's directory
- Construct a review prompt with:
  - The task description and relevant spec section numbers
  - Paths to spec files (requirements.md, design.md) so the reviewer can read them directly
  - The implementer's status report (for reference only — reviewer must verify independently)
- The reviewer must apply the `kiro-review` protocol to this task-local review.
- Preserve the existing task-specific context: task text, spec refs, `_Boundary:_` scope, validation commands, implementer report, and the actual `git diff` as the primary source of truth.
- The reviewer sub-agent will run `git diff` itself to read the actual code changes and verify against the spec
- Spawn a fresh sub-agent with this prompt

**d) Handle reviewer verdict**:
- Parse reviewer verdict only from the exact `## Review Verdict` block and `- VERDICT:` field.
- If `VERDICT` is missing, ambiguous, or replaced with prose, re-dispatch the reviewer once requesting the exact structured verdict only. If the second response is still unparseable, dispatch the debug subagent with root cause `HANDOFF_PARSE_FAILURE`. Do NOT mark the task complete, commit, or continue to the next task without a parseable `APPROVED | REJECTED` value.
- **APPROVED** → before marking the task `[x]` or making any success claim, apply `kiro-verify-completion` using fresh evidence from the current code state.
  - If completion verification returns `VERIFIED`, mark the task `[x]` in tasks.md and proceed to record learnings and commit.
  - If it returns `NOT_VERIFIED`, do not mark complete; increment this task's `review_rejection_count` with the verification findings. If `review_rejection_count <= 2`, re-dispatch the implementer with those findings and skip record/commit. If `review_rejection_count >= 3`, jump directly to the debug subagent and skip record/commit.
  - If it returns `MANUAL_VERIFY_REQUIRED`, stop without marking complete and report the missing verification step.
- **REJECTED** → increment this task's `review_rejection_count`. If `review_rejection_count <= 2`, re-dispatch the implementer with review feedback and skip record/commit. If `review_rejection_count >= 3`, jump directly to the debug subagent and skip record/commit.

**e) Debug subagent** (triggered by BLOCKED, NEEDS_CONTEXT unresolved, HANDOFF_PARSE_FAILURE, NOT_VERIFIED after retries, or REJECTED after 2 remediation rounds):

The debug subagent runs in a **fresh context** — it receives only the error information, not the failed implementation history. This avoids the context pollution that causes infinite retry loops.

- Read `templates/debugger-prompt.md` from this skill's directory
- Construct a debug prompt with:
  - The error description / blocker reason / reviewer rejection findings
  - `git diff` of the current uncommitted changes
  - The task description and relevant spec section numbers
  - Paths to spec files so the debugger can read them
- The debugger must apply the `kiro-debug` protocol to this failure investigation.
- Preserve rich failure context: error output, reviewer findings, current `git diff`, task/spec refs, and any relevant Implementation Notes.
- When available, the debugger should inspect runtime/config state and use web or official documentation research to validate root-cause hypotheses before proposing a fix plan.
- Spawn a fresh sub-agent with this prompt

**Handle debug report**:
- Parse `NEXT_ACTION` from the debug report's exact structured field.
- If `NEXT_ACTION: STOP_FOR_HUMAN` → append `_Blocked: <ROOT_CAUSE>_` to tasks.md, stop the feature run, and report that human review is required before continuing
- If `NEXT_ACTION: BLOCK_TASK` → append `_Blocked: <ROOT_CAUSE>_` to tasks.md, then inspect `git status --porcelain`. Do not continue to the next task while failed-task edits are mixed into the worktree. Stash or revert only changes known to have been made for the blocked task after preserving the blocker evidence; if task-local changes cannot be isolated confidently, stop and report the dirty paths for human decision.
- If `NEXT_ACTION: RETRY_TASK` → preserve the current worktree; do NOT reset or discard unrelated changes. Spawn a **new** implementer sub-agent with the debug report's `FIX_PLAN`, `NOTES`, and the current `git diff`, and require it to repair the task with explicit edits only
  - If the new implementer succeeds (READY_FOR_REVIEW → reviewer APPROVED) → normal flow
  - If the new implementer also fails → repeat debug cycle (max 2 debug rounds total). After 2 failed debug rounds → append `_Blocked: debug attempted twice, still failing — <ROOT_CAUSE>_` to tasks.md, isolate failed-task dirty changes using the same rule as `BLOCK_TASK`, then skip only if the worktree is clean or contains only unrelated pre-existing changes
- Maintain `debug_round_count` per task ID and increment it before each debug subagent dispatch. **Max 2 debug rounds per task** applies to every debug trigger, including parse failures, `BLOCKED`, `REJECTED`, and `NOT_VERIFIED` after a debug retry. If any path would dispatch a third debug round, block the task instead of dispatching.
- Record debug findings in `## Implementation Notes` (this helps subsequent tasks avoid the same issue)

**f) Record learnings**:
- Run this step only after the task is verified and marked `[x]`. If this task revealed cross-cutting insights, append a one-line note to the `## Implementation Notes` section at the bottom of tasks.md before committing so the note is included with the task completion commit.

**g) Commit** (parent-only, selective staging):
- Run this step only after successful verification, task completion marking, and learning-note recording.
- Stage only the files actually changed for this task, plus tasks.md
- **NEVER** use `git add -A` or `git add .`
- Use `git add <file1> <file2> ...` with explicit file paths
- Commit message format: `feat(<feature-name>): <task description>`

**`(P)` markers**: Tasks marked `(P)` in tasks.md indicate they have no inter-dependencies and could theoretically run in parallel. However, kiro-impl processes them sequentially (one at a time) to avoid git conflicts and simplify review. The `(P)` marker is informational for task planning, not an execution directive.

**Fallback**: If multi-agent is not available, fall back to manual mode execution for all tasks.

### Manual Mode (main context)

For each selected task:

**1. Build Task Brief**:
Before writing any code, read the relevant sections of requirements.md and design.md for this task and clarify:
- What observable behaviors must be true when done (acceptance criteria)
- What files/functions/tests must exist (completion definition)
- What technical decisions to follow from design.md (design constraints)
- How to confirm the task works (verification method)

**2. Execute TDD cycle** (Kent Beck's RED → GREEN → REFACTOR):
- **RED**: Write test for the next small piece of functionality based on the acceptance criteria. Test should fail.
- **GREEN**: Implement simplest solution to make test pass, following the design constraints.
- **REFACTOR**: Improve code structure, remove duplication. All tests must still pass.
- **VERIFY**: All tests pass (new and existing), no regressions. Confirm verification method passes.
- **REVIEW**: Apply `kiro-review` before marking the task complete. If the host supports fresh sub-agents in manual mode, use a fresh reviewer; otherwise perform the review in the main context using the `kiro-review` protocol. Do NOT continue until the verdict is parseably `APPROVED`.
- **MARK COMPLETE**: Only after review returns `APPROVED`, apply `kiro-verify-completion`, then update the checkbox from `- [ ]` to `- [x]` in tasks.md.

## Step 4: Final Validation

**Autonomous mode**:
- Before final validation, re-read `tasks.md` and verify every selected required task is `[x]`. Deferred optional `- [ ]*` tasks do not block validation unless the user explicitly selected them for this run. If required unchecked or `_Blocked:_` tasks remain, stop and report them; do not run feature-level validation.
- After every selected task is `[x]`, run `$kiro-validate-impl $1` as a GO/NO-GO gate
- If validation returns GO → before reporting feature success, apply `kiro-verify-completion` to the feature-level claim using the validation result and fresh supporting evidence
- If validation returns NO-GO:
  - Fix only concrete findings from the validation report
  - Cap remediation at 3 rounds; if still NO-GO, stop and report remaining findings
- If validation returns MANUAL_VERIFY_REQUIRED → stop and report the missing verification step

**Manual mode**:
- Suggest running `$kiro-validate-impl $1` but do not auto-execute

## Feature Flag Protocol

For tasks that add or change behavior, enforce RED → GREEN with a feature flag:

1. **Add flag** (OFF by default): Introduce a toggle appropriate to the codebase (env var, config constant, boolean, conditional)
2. **RED -- flag OFF**: Write tests for the new behavior. Run tests → must FAIL. If tests pass with flag OFF, the tests are not testing the right thing. Rewrite.
3. **GREEN -- flag ON + implement**: Enable the flag, write implementation. Run tests → must PASS.
4. **Remove flag**: Make the code unconditional. Run tests → must still PASS.

**Skip this protocol for**: refactoring, configuration, documentation, or tasks with no behavioral change.

</instructions>

## Critical Constraints
- **Strict Handoff Parsing**: Never infer implementer `STATUS` or reviewer `VERDICT` from surrounding prose; only the exact structured fields count
- **No Destructive Reset**: Never use `git checkout .`, `git reset --hard`, or similar destructive rollback inside the implementation loop
- **Selective Staging**: NEVER use `git add -A` or `git add .`; always stage explicit file paths
- **Bounded Review Rounds**: Maintain a per-task `review_rejection_count`; after 2 implementer re-dispatches for reviewer rejection, route the third rejection to debug
- **Bounded Debug**: Max 2 debug rounds per task (debug + re-implementation per round); if still failing → BLOCKED
- **Bounded Remediation**: Cap final-validation remediation at 3 rounds

## Output Description

**Autonomous mode**: For each task, report: task ID, implementer status, reviewer verdict, files changed, commit hash. After all tasks: final validation result.

**Manual mode**: Tasks executed with test results. Status of completed/remaining tasks.

**Format**: Concise, in the language specified in spec.json.

## Safety & Fallback

### Error Scenarios

**Tasks Not Approved or Missing Spec Files**:
- **Stop Execution**: All spec files must exist and tasks must be approved
- **Suggested Action**: "Complete previous phases: `$kiro-spec-requirements`, `$kiro-spec-design`, `$kiro-spec-tasks`"

**Test Failures**:
- **Stop Implementation**: Fix failing tests before continuing
- **Action**: Debug and fix, then re-run

**All Tasks Blocked**:
- Stop and report all blocked tasks with reasons; human review needed

**Spec Conflicts with Reality**:
- Block the task with `_Blocked: <reason>_` -- do not silently work around it

**Upstream Ownership Detected**:
- If review, debug, or validation shows that the root cause belongs to an upstream, foundation, shared-platform, or dependency spec, do not patch around it inside the downstream feature
- Route the fix back to the owning upstream spec, keep the downstream task blocked until that contract is repaired, and re-run validation/smoke for dependent specs after the upstream fix lands

**Task Plan Invalidated During Implementation**:
- If debug returns `NEXT_ACTION: STOP_FOR_HUMAN` because of task ordering, boundary, or decomposition problems, stop and return for human review of `tasks.md` or the approved plan instead of forcing a code workaround

**Session Interrupted**:
- Safe to re-run `$kiro-impl $1` — completed tasks are already `[x]` in tasks.md and committed to git
- The controller re-reads tasks.md on each iteration, so it will pick up where it left off automatically
