import { setTimeout } from "node:timers/promises";

export async function waitForCodecov({ github, context, core, env, fetchUploads = fetch, sleep = setTimeout }) {
  const coverageResult = env.COVERAGE_RESULT;
  if (coverageResult !== "success") {
    throw new Error(`coverage failed or was skipped: ${coverageResult}`);
  }

  const owner = context.repo.owner;
  const repo = context.repo.repo;
  const ref = env.CODECOV_SHA;
  const isPullRequest = env.IS_PULL_REQUEST === "true";
  const uploadStartedAt = Date.parse(env.CODECOV_UPLOAD_STARTED_AT);
  const buildUrl = env.CODECOV_BUILD_URL;
  const uploadName = env.CODECOV_UPLOAD_NAME;
  const requiredChecks = ["codecov/project"];

  if (isPullRequest) {
    if (!Number.isFinite(uploadStartedAt) || !buildUrl || !uploadName) {
      throw new Error("Coverage job did not report valid Codecov upload metadata");
    }
    requiredChecks.push("codecov/patch");
  } else {
    core.info("Skipping codecov/patch outside pull_request events");
  }

  for (const checkName of requiredChecks) {
    await waitForCheck(checkName);
  }

  async function waitForCheck(checkName) {
    for (let attempt = 1; attempt <= 60; attempt += 1) {
      const run = await latestCheckRun(checkName);

      if (run) {
        core.info(`${checkName}: status=${run.status} conclusion=${run.conclusion || ""} url=${run.html_url}`);

        if (run.status === "completed" &&
            await matchesProcessedUpload(run.completed_at || run.started_at || run.created_at)) {
          if (run.conclusion === "success") {
            return;
          }

          throw new Error(`${checkName} failed with conclusion=${run.conclusion}`);
        }
      } else {
        const status = await latestCommitStatus(checkName);
        if (status) {
          core.info(`${checkName}: state=${status.state} url=${status.target_url || ""}`);

          if (["success", "failure", "error"].includes(status.state) &&
              await matchesProcessedUpload(status.created_at, status.state === "success")) {
            if (status.state === "success") {
              return;
            }
            throw new Error(`${checkName} failed with state=${status.state}`);
          }
        } else {
          core.info(`${checkName}: waiting for Codecov status (${attempt}/60)`);
        }
      }

      await sleep(10000);
    }

    if (isPullRequest) {
      throw new Error(`${checkName} did not report within timeout`);
    }
    core.warning(`${checkName} did not report within timeout; coverage upload already succeeded`);
  }

  async function latestCheckRun(checkName) {
    const response = await github.rest.checks.listForRef({
      owner,
      repo,
      ref,
      check_name: checkName,
      per_page: 100,
    });
    return response.data.check_runs
      .filter((checkRun) => checkRun.name === checkName)
      .sort((left, right) => {
        const rightTime = Date.parse(right.started_at || right.created_at);
        const leftTime = Date.parse(left.started_at || left.created_at);
        return rightTime - leftTime;
      })[0];
  }

  async function latestCommitStatus(checkName) {
    const response = await github.rest.repos.getCombinedStatusForRef({ owner, repo, ref });
    return response.data.statuses.find((status) => status.context === checkName);
  }

  async function matchesProcessedUpload(timestamp, allowUnchangedSuccess = false) {
    if (!isPullRequest) return true;

    // build_code は null の場合がある。実行試行の URL で今回の upload を
    // 特定し、その処理が終わった後の GitHub 通知を待つ。
    const statusAt = Date.parse(timestamp);
    if (!Number.isFinite(statusAt)) return false;
    const uploadsUrl = `https://api.codecov.io/api/v2/github/${owner}/repos/${repo}/commits/${ref}/uploads/?page_size=150`;
    try {
      const response = await fetchUploads(uploadsUrl, { signal: AbortSignal.timeout(10000) });
      if (!response.ok) return false;
      const { results, next } = await response.json();
      if (next || !Array.isArray(results)) return false;
      const merged = results
        .filter((upload) => upload.state === "merged")
        .map((upload) => ({ ...upload, processedAt: Date.parse(upload.updated_at) }))
        .sort((left, right) => right.processedAt - left.processedAt);
      const current = merged.find((upload) => upload.build_url === buildUrl &&
        upload.name === uploadName && Date.parse(upload.created_at) >= uploadStartedAt);
      if (!current || !Number.isFinite(current.processedAt)) return false;
      if (current.processedAt <= statusAt) return true;
      if (!allowUnchangedSuccess || merged.some((upload) => !Number.isFinite(upload.processedAt))) return false;

      // 同じ成功のコミットステータスは再投稿されないことがある。通知時点の
      // 最新レポートと、それ以降に処理されたレポートがすべて同じ入力であることを確認する。
      const lastPublishedUpload = merged.find((upload) => upload.processedAt <= statusAt);
      return lastPublishedUpload?.name === uploadName &&
        merged.every((upload) => upload.processedAt < lastPublishedUpload.processedAt || upload.name === uploadName);
    } catch (error) {
      core.info(`Could not verify Codecov upload: ${error.message}`);
      return false;
    }
  }
}
