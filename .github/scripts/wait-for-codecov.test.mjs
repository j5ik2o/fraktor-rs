import assert from "node:assert/strict";
import test from "node:test";
import { waitForCodecov } from "./wait-for-codecov.mjs";

const env = {
  COVERAGE_RESULT: "success",
  CODECOV_SHA: "a".repeat(40),
  CODECOV_UPLOAD_STARTED_AT: "2026-11-21T15:10:23Z",
  CODECOV_BUILD_URL: "https://github.com/example/project/actions/runs/42/attempts/2",
  CODECOV_UPLOAD_NAME: `fraktor-coverage-${"b".repeat(64)}`,
  IS_PULL_REQUEST: "true",
};
const upload = {
  build_code: null,
  build_url: env.CODECOV_BUILD_URL,
  name: env.CODECOV_UPLOAD_NAME,
  state: "merged",
  created_at: "2026-11-21 15:10:26.589488+00:00",
  updated_at: "2026-11-21 15:10:34.871876+00:00",
};
const publishedAt = "2026-11-21T15:10:51Z";

function checkRun(name, conclusion = "success", timestamp = publishedAt) {
  return { name, status: "completed", conclusion, started_at: timestamp, completed_at: timestamp };
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

test("rejects a check started before the upload was processed even if it finished later", async () => {
  let attempts = 0;
  const { run, checks } = fixture({ check: (name) => {
    const result = checkRun(name);
    return attempts++ === 0 ? { ...result, started_at: "2026-11-21T15:10:30Z" } : result;
  } });
  await run();
  assert.deepEqual(checks, ["codecov/project", "codecov/project", "codecov/patch"]);
});

test("accepts publication in the same second as upload processing", async () => {
  await fixture({ check: (name) => checkRun(name, "success", "2026-11-21T15:10:34Z") }).run();
});

test("accepts a matching upload on the first page even when there is a next page", async () => {
  await fixture({ fetchUploads: async (url) => {
    assert.equal(new URL(url).searchParams.get("page"), null);
    return { ok: true, json: async () => ({ next: `${url}&page=2`, results: [upload] }) };
  } }).run();
});

test("follows the next page when the first page has no matching upload", async () => {
  const pages = [];
  await fixture({ fetchUploads: async (url) => {
    const page = new URL(url).searchParams.get("page");
    pages.push(page);
    return { ok: true, json: async () => page === null
      ? { next: `${url}&page=2`, results: [] }
      : { next: null, results: [upload] } };
  } }).run();
  assert.deepEqual(pages, [null, "2", null, "2"]);
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

const identicalPreviousUpload = {
  ...upload,
  build_url: env.CODECOV_BUILD_URL.replace(/2$/, "1"),
  created_at: "2026-11-21T14:00:00Z",
  updated_at: "2026-11-21T14:00:30Z",
};
const olderSuccesses = ["codecov/project", "codecov/patch"].map((context) => ({
  context, state: "success", created_at: "2026-11-21T14:00:45Z",
}));

test("accepts a fresh project check and an unchanged patch check on an identical rerun", async () => {
  const { run, checks } = fixture({
    check: (name) => checkRun(name, "success", name === "codecov/project"
      ? publishedAt : olderSuccesses[1].created_at),
    uploads: [identicalPreviousUpload, upload],
  });
  await run();
  assert.deepEqual(checks, ["codecov/project", "codecov/patch"]);
});

test("waits past ten minutes for a fresh patch check when the rerun report differs", async () => {
  let patchPolls = 0;
  const { run } = fixture({
    check: (name) => checkRun(name, "success", name === "codecov/patch" && patchPolls++ < 68
      ? olderSuccesses[1].created_at : publishedAt),
    uploads: [{ ...identicalPreviousUpload, name: "different-report" }, upload],
  });
  await run();
  assert.equal(patchPolls, 69);
});

test("does not reuse a previous patch check that finished after the rerun started", async () => {
  let patchPolls = 0;
  const { run } = fixture({
    check: (name) => {
      if (name === "codecov/patch" && patchPolls++ === 0) {
        return { ...checkRun(name, "success", olderSuccesses[1].created_at), completed_at: publishedAt };
      }
      return checkRun(name);
    },
    uploads: [identicalPreviousUpload, upload],
  });
  await run();
  assert.equal(patchPolls, 2);
});

test("reuses an unchanged patch commit status after an identical rerun is processed", async () => {
  await fixture({
    check: () => undefined,
    statuses: [{ ...olderSuccesses[0], created_at: publishedAt }, olderSuccesses[1]],
    uploads: [identicalPreviousUpload, upload],
  }).run();
});

test("waits for the current project verdict even when an identical report previously passed", async () => {
  let projectPolls = 0;
  const { run, checks } = fixture({
    check: (name) => name === "codecov/project" && projectPolls++ === 0
      ? checkRun(name, "success", olderSuccesses[0].created_at)
      : checkRun(name, "failure"),
    uploads: [identicalPreviousUpload, upload],
  });
  await assert.rejects(run(), /codecov\/project failed with conclusion=failure/);
  assert.deepEqual(checks, ["codecov/project", "codecov/project"]);
});

test("fails closed when only an unchanged project commit status is available", async () => {
  const { run } = fixture({
    check: () => undefined,
    statuses: olderSuccesses,
    uploads: [identicalPreviousUpload, upload],
  });
  await assert.rejects(run(), /codecov\/project did not report within timeout/);
});

test("refuses to reuse commit statuses if a different report was merged after publication", async () => {
  const { run } = fixture({
    check: () => undefined,
    statuses: [{ ...olderSuccesses[0], created_at: publishedAt }, olderSuccesses[1]],
    uploads: [identicalPreviousUpload, upload, {
      ...identicalPreviousUpload,
      name: "different-merge-or-coverage",
      updated_at: "2026-11-21T15:00:30Z",
    }],
  });
  await assert.rejects(run(), /codecov\/patch did not report within timeout/);
});

test("refuses to reuse a status following a different most recent report", async () => {
  const { run } = fixture({
    check: () => undefined,
    statuses: [{ ...olderSuccesses[0], created_at: publishedAt }, olderSuccesses[1]],
    uploads: [identicalPreviousUpload, upload, {
      ...identicalPreviousUpload,
      name: "different-merge-or-coverage",
      updated_at: "2026-11-21T14:00:40Z",
    }],
  });
  await assert.rejects(run(), /codecov\/patch did not report within timeout/);
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
