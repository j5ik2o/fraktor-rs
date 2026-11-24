import assert from "node:assert/strict";
import test from "node:test";
import { waitForCodecov } from "./wait-for-codecov.mjs";

const env = {
  COVERAGE_RESULT: "success",
  CODECOV_SHA: "a".repeat(40),
  CODECOV_UPLOAD_STARTED_AT: "2026-11-21T15:10:23Z",
  CODECOV_BUILD_URL: "https://github.com/example/project/actions/runs/42/attempts/2",
  IS_PULL_REQUEST: "true",
};
const upload = {
  build_code: null,
  build_url: env.CODECOV_BUILD_URL,
  state: "merged",
  created_at: "2026-11-21 15:10:26.589488+00:00",
  updated_at: "2026-11-21 15:10:34.871876+00:00",
};
const publishedAt = "2026-11-21T15:10:51Z";

function checkRun(name, conclusion = "success", timestamp = publishedAt) {
  return { name, status: "completed", conclusion, completed_at: timestamp };
}

function fixture({
  uploads = [upload],
  check = checkRun,
  statuses = [],
  fetchUploads,
  overrides,
} = {}) {
  const checks = [];
  const warnings = [];
  const messages = [];
  return {
    checks,
    warnings,
    messages,
    run: () => waitForCodecov({
      env: { ...env, ...overrides },
      context: { repo: { owner: "example", repo: "project" } },
      core: {
        info: (message) => messages.push(message),
        warning: (message) => warnings.push(message),
      },
      github: { rest: {
        checks: { listForRef: async ({ ref, check_name }) => {
          assert.equal(ref, env.CODECOV_SHA);
          checks.push(check_name);
          const run = check(check_name);
          return { data: { check_runs: run ? [run] : [] } };
        } },
        repos: { getCombinedStatusForRef: async () => ({ data: { statuses } }) },
      } },
      fetchUploads: fetchUploads ?? (async (url) => {
        assert.match(url, new RegExp(`/commits/${env.CODECOV_SHA}/uploads/`));
        return { ok: true, json: async () => ({ next: null, results: uploads }) };
      }),
      sleep: async () => {},
    }),
  };
}

test("accepts the current processed upload with null build_code and an older different build", async () => {
  const { run, checks } = fixture({ uploads: [
    {
      ...upload,
      build_code: "older-build",
      build_url: env.CODECOV_BUILD_URL.replace(/2$/, "1"),
      created_at: "2026-11-21T14:00:00Z",
      updated_at: "2026-11-21T14:00:30Z",
    },
    upload,
  ] });
  await run();
  assert.deepEqual(checks, ["codecov/project", "codecov/patch"]);
});

test("reports Codecov's current failure instead of timing out", async () => {
  const { run, checks } = fixture({ check: (name) => checkRun(name, "failure") });
  await assert.rejects(run(), /codecov\/project failed with conclusion=failure/);
  assert.deepEqual(checks, ["codecov/project"]);
});

test("rejects stale success and waits for a fresh check", async () => {
  let attempts = 0;
  const { run, checks } = fixture({ check: (name) => {
    const timestamp = attempts++ === 0 ? "2026-11-21T15:10:30Z" : publishedAt;
    return checkRun(name, "success", timestamp);
  } });
  await run();
  assert.deepEqual(checks, ["codecov/project", "codecov/project", "codecov/patch"]);
});

test("times out when only another attempt's upload was processed", async () => {
  const { run, checks } = fixture({ uploads: [
    { ...upload, build_url: env.CODECOV_BUILD_URL.replace(/2$/, "1") },
    { ...upload, state: "pending" },
  ] });
  await assert.rejects(run(), /codecov\/project did not report within timeout/);
  assert.equal(checks.length, 60);
});

test("times out when the matching attempt predates this coverage upload", async () => {
  const { run } = fixture({ uploads: [{ ...upload, created_at: "2026-11-21T15:10:00Z" }] });
  await assert.rejects(run(), /codecov\/project did not report within timeout/);
});

test("retries after a Codecov uploads API error", async () => {
  let attempts = 0;
  const { run, messages } = fixture({ fetchUploads: async () => {
    if (attempts++ === 0) throw new Error("network error");
    return { ok: true, json: async () => ({ next: null, results: [upload] }) };
  } });
  await run();
  assert.ok(messages.includes("Could not verify Codecov upload: network error"));
});

test("supports commit statuses when Codecov has no check runs", async () => {
  const statuses = ["codecov/project", "codecov/patch"].map((context) => ({
    context, state: "success", created_at: publishedAt,
  }));
  await fixture({ check: () => undefined, statuses }).run();
});

test("fails before polling if coverage or PR upload metadata is missing", async () => {
  const failed = fixture({ overrides: { COVERAGE_RESULT: "failure" } });
  await assert.rejects(failed.run(), /coverage failed or was skipped: failure/);
  assert.deepEqual(failed.checks, []);
  const missing = fixture({ overrides: { CODECOV_BUILD_URL: "" } });
  await assert.rejects(missing.run(), /did not report valid Codecov upload metadata/);
  assert.deepEqual(missing.checks, []);
});

test("warns on timeout and skips the patch check outside pull requests", async () => {
  const { run, checks, warnings } = fixture({
    overrides: { IS_PULL_REQUEST: "false" },
    check: () => undefined,
  });
  await run();
  assert.equal(checks.length, 60);
  assert.ok(checks.every((check) => check === "codecov/project"));
  assert.match(warnings[0], /codecov\/project did not report within timeout/);
});
