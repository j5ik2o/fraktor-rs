import assert from "node:assert/strict";
import test from "node:test";

import { buildTaktArgs, collapseTaskPayload, resolveForwardedArgs } from "./kiro-staged.mjs";

const WF = "WF";
const DEFAULT_TASK = "既定タスク文";

// --- task 本文と takt オプションの分離（区切りあり / なし） ---

test("buildTaktArgs keeps trailing takt options out of the task payload", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "my-feature", "--provider", "mock"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "my-feature", "--provider", "mock"],
  );
});

test("buildTaktArgs moves options before `--` ahead of -t and joins the payload after", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "--provider", "mock", "--", "fix", "login"]),
    ["--pipeline", "--skip-git", "--provider", "mock", "-w", WF, "-t", "fix login"],
  );
});

test("buildTaktArgs joins a multi-word task without a separator", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "fix", "login", "bug"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "fix login bug"],
  );
});

test("buildTaktArgs leaves an option-like word inside a quoted task untouched", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "fix the --help flag"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "fix the --help flag"],
  );
});

test("collapseTaskPayload rejects a bare leading option without a `--` separator", () => {
  assert.throws(
    () => collapseTaskPayload(["--pipeline", "--skip-git", "-t", "--provider", "mock"]),
    /must be separated/,
  );
});

// --- help 判定は task 本文を除いた領域に限定する ---

test("buildTaktArgs strips -t for a pure help request", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "--help"]),
    ["--pipeline", "--skip-git", "--help", "-w", WF],
  );
});

test("buildTaktArgs does not treat --help inside the task payload as a help request", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "--provider", "mock", "--", "add", "--help", "screen"]),
    ["--pipeline", "--skip-git", "--provider", "mock", "-w", WF, "-t", "add --help screen"],
  );
});

// --- resolveForwardedArgs: no-arg コマンドへの既定 task 供給 ---

test("resolveForwardedArgs supplies the default task when -t ends with no value", () => {
  assert.deepEqual(
    resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t"]),
    ["--pipeline", "--skip-git", "-t", DEFAULT_TASK],
  );
});

test("resolveForwardedArgs leaves an explicit task untouched", () => {
  assert.deepEqual(
    resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t", "my-feature"]),
    ["--pipeline", "--skip-git", "-t", "my-feature"],
  );
});

test("resolveForwardedArgs is a passthrough without --default-task", () => {
  assert.deepEqual(
    resolveForwardedArgs(["--pipeline", "--skip-git", "-t", "x"]),
    ["--pipeline", "--skip-git", "-t", "x"],
  );
});

test("resolveForwardedArgs throws when --default-task has no value", () => {
  assert.throws(() => resolveForwardedArgs(["--default-task"]), /requires a value/);
});

test("resolveForwardedArgs default task flows through buildTaktArgs for a no-arg command", () => {
  assert.deepEqual(
    buildTaktArgs(WF, resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t"])),
    ["--pipeline", "--skip-git", "-w", WF, "-t", DEFAULT_TASK],
  );
});

test("resolveForwardedArgs keeps the help path for a no-arg command with --help", () => {
  assert.deepEqual(
    buildTaktArgs(WF, resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t", "--help"])),
    ["--pipeline", "--skip-git", "--help", "-w", WF],
  );
});

// --- 既定 task と takt オプションの併用 ---

test("resolveForwardedArgs applies the default task when only takt options follow -t", () => {
  assert.deepEqual(
    buildTaktArgs(WF, resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t", "--provider", "mock"])),
    ["--pipeline", "--skip-git", "-w", WF, "-t", DEFAULT_TASK, "--provider", "mock"],
  );
});

test("resolveForwardedArgs applies the default task at the payload position after a `--` separator", () => {
  assert.deepEqual(
    buildTaktArgs(WF, resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t", "--provider", "mock", "--"])),
    ["--pipeline", "--skip-git", "--provider", "mock", "-w", WF, "-t", DEFAULT_TASK],
  );
});

test("resolveForwardedArgs does not override an explicit task even with trailing options", () => {
  assert.deepEqual(
    buildTaktArgs(WF, resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t", "my-feature", "--provider", "mock"])),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "my-feature", "--provider", "mock"],
  );
});

test("resolveForwardedArgs keeps the help path when options precede --help", () => {
  assert.deepEqual(
    buildTaktArgs(WF, resolveForwardedArgs(["--default-task", DEFAULT_TASK, "--pipeline", "--skip-git", "-t", "--provider", "mock", "--help"])),
    ["--pipeline", "--skip-git", "--provider", "mock", "--help", "-w", WF],
  );
});

// --- Kiro 専用フラグ（-y / --auto）は takt に渡さず task 本文へ ---

test("buildTaktArgs folds -y into the task payload", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "my-feature", "-y"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "my-feature -y"],
  );
});

test("buildTaktArgs folds -y into the payload while preserving trailing takt options", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "my-feature", "-y", "--provider", "mock"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "my-feature -y", "--provider", "mock"],
  );
});

test("buildTaktArgs folds --auto into the task payload", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "user profile feature", "--auto"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "user profile feature --auto"],
  );
});

test("buildTaktArgs folds a leading kiro flag into the payload instead of erroring", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "-y", "my-feature"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "-y my-feature"],
  );
});

test("buildTaktArgs folds a kiro flag trailing takt options back into the payload", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "my-feature", "--provider", "mock", "-y"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "my-feature -y", "--provider", "mock"],
  );
});

test("buildTaktArgs folds --auto trailing takt options back into the payload", () => {
  assert.deepEqual(
    buildTaktArgs(WF, ["--pipeline", "--skip-git", "-t", "user profile", "--provider", "mock", "--auto"]),
    ["--pipeline", "--skip-git", "-w", WF, "-t", "user profile --auto", "--provider", "mock"],
  );
});
