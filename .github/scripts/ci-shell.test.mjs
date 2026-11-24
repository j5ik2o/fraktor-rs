import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

function workspace(t) {
  const dir = mkdtempSync(join(tmpdir(), "fraktor-ci-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

function runScript(name, cwd, env = {}) {
  const script = fileURLToPath(new URL(name, import.meta.url));
  const childEnv = { ...process.env, ...env };
  // The nested Node test runner must report to stdout as an independent process.
  delete childEnv.NODE_TEST_CONTEXT;
  const result = spawnSync("bash", [script], {
    cwd, env: childEnv, encoding: "utf8", timeout: 5000,
  });
  assert.ifError(result.error);
  return result;
}

function revisionFixture(t, { reverseParents = false } = {}) {
  const cwd = workspace(t);
  const git = (...args) => {
    const result = spawnSync("git", args, {
      cwd,
      env: {
        ...process.env,
        GIT_AUTHOR_NAME: "CI tests",
        GIT_AUTHOR_EMAIL: "ci@example.com",
        GIT_COMMITTER_NAME: "CI tests",
        GIT_COMMITTER_EMAIL: "ci@example.com",
      },
      encoding: "utf8",
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  git("init", "--quiet");
  git("-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m", "old base");
  const oldBase = git("rev-parse", "HEAD");
  const tree = git("rev-parse", "HEAD^{tree}");
  const base = git("commit-tree", tree, "-p", oldBase, "-m", "new base");
  const head = git("commit-tree", tree, "-p", oldBase, "-m", "PR head");
  const stale = git("commit-tree", tree, "-p", oldBase, "-p", head, "-m", "stale PR merge");
  const parents = reverseParents ? [head, base] : [base, head];
  const merge = git("commit-tree", tree, "-p", parents[0], "-p", parents[1], "-m", "PR merge");
  git("remote", "add", "origin", ".");
  const ref = "refs/pull/42/merge";
  const output = join(cwd, "output");
  const env = {
    EVENT_NAME: "pull_request", PR_NUMBER: "42", BASE_SHA: base, PR_HEAD_SHA: head,
    GITHUB_OUTPUT: output,
  };
  return { cwd, git, head, merge, stale, ref, output, env };
}

test("verifies an already matching merge checkout without fetching", (t) => {
  const { cwd, git, merge, output, env } = revisionFixture(t);
  git("checkout", "--quiet", "--detach", merge);
  const result = runScript("./verify-source-revision.sh", cwd, env);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, new RegExp(`CI checkout ${merge}: parents ${env.BASE_SHA} ${env.PR_HEAD_SHA}`));
  assert.equal(readFileSync(output, "utf8"), `checkout_sha=${merge}\n`);
});

test("retries a stale merge ref and checks out the validated commit", (t) => {
  const { cwd, git, merge, stale, ref, output, env } = revisionFixture(t);
  git("checkout", "--quiet", "--detach", stale);
  git("update-ref", ref, stale);
  writeFileSync(join(cwd, "sleep"), "#!/usr/bin/env bash\ngit update-ref \"$PR_REF\" \"$MATCHING_MERGE\"\n", {
    mode: 0o755,
  });
  const result = runScript("./verify-source-revision.sh", cwd, {
    ...env, PATH: `${cwd}:${process.env.PATH}`, PR_REF: ref, MATCHING_MERGE: merge,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.stdout.match(/Waiting for matching PR merge ref \(attempt .*\)/g), [
    "Waiting for matching PR merge ref (attempt 1/6)",
  ]);
  assert.equal(git("rev-parse", "HEAD"), merge);
  assert.equal(readFileSync(output, "utf8"), `checkout_sha=${merge}\n`);
});

test("rejects a PR head checkout and a merge ref with reversed parents", (t) => {
  const { cwd, git, head, merge, ref, env } = revisionFixture(t, { reverseParents: true });
  git("checkout", "--quiet", "--detach", head);
  git("update-ref", ref, merge);
  writeFileSync(join(cwd, "sleep"), "#!/usr/bin/env bash\nexit 0\n", { mode: 0o755 });
  const result = runScript("./verify-source-revision.sh", cwd, {
    ...env, PATH: `${cwd}:${process.env.PATH}`,
  });
  assert.equal(result.status, 1, result.stderr);
  assert.equal(result.stdout.match(/Waiting for matching PR merge ref/g).length, 6);
  assert.match(result.stdout, /No PR merge commit with the expected base and head parents/);
  assert.equal(git("rev-parse", "HEAD"), head);
});

test("rejects invalid PR metadata before fetching", (t) => {
  const { cwd, env } = revisionFixture(t);
  const result = runScript("./verify-source-revision.sh", cwd, { ...env, PR_NUMBER: "invalid" });
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stdout, /Pull request number, base SHA, or head SHA is invalid/);
  assert.doesNotMatch(result.stdout, /Waiting for matching PR merge ref/);
});

test("runs Node tests when only one candidate glob matches", (t) => {
  const cwd = workspace(t);
  mkdirSync(join(cwd, ".github", "scripts"), { recursive: true });
  writeFileSync(join(cwd, ".github", "scripts", "one.test.mjs"), [
    'import test from "node:test";',
    'test("glob fixture", () => {});',
  ].join("\n"));
  const result = runScript("./run-node-tests.sh", cwd);
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /glob fixture/);
});

test("fails with the intended diagnostic when neither Node test glob matches", (t) => {
  const result = runScript("./run-node-tests.sh", workspace(t));
  assert.equal(result.status, 1, result.stderr);
  assert.equal(result.stdout.trim(), "No Node test files found");
});
