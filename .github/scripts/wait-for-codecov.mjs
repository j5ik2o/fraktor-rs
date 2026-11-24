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
    const isPatchCheck = checkName === "codecov/patch";
    // 再実行時、project の結果が届いてから patch の再通知まで10分を超えることがある。
    const maxAttempts = isPatchCheck ? 90 : 60;
    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      const run = await latestCheckRun(checkName);

      if (run) {
        core.info(`${checkName}: status=${run.status} conclusion=${run.conclusion || ""} url=${run.html_url}`);

        // project の比較基準は同じベース SHA でも更新され得る。
        const allowUnchangedSuccess = isPatchCheck && run.conclusion === "success" &&
          Date.parse(run.completed_at) < uploadStartedAt;
        if (run.status === "completed" &&
            await matchesProcessedUpload(run.started_at || run.created_at, allowUnchangedSuccess)) {
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
              await matchesProcessedUpload(status.created_at, isPatchCheck && status.state === "success")) {
            if (status.state === "success") {
              return;
            }
            throw new Error(`${checkName} failed with state=${status.state}`);
          }
        } else {
          core.info(`${checkName}: waiting for Codecov status (${attempt}/${maxAttempts})`);
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

    // GitHub の通知に upload ID はない。実行試行の URL とレポート名で今回の
    // upload を特定し、処理が終わった秒以降に作成された通知を待つ。
    const statusAt = Date.parse(timestamp);
    if (!Number.isFinite(statusAt)) return false;
    const uploadsUrl = new URL(
      `https://api.codecov.io/api/v2/github/${owner}/repos/${repo}/commits/${ref}/uploads/?page_size=150`,
    );
    try {
      const signal = AbortSignal.timeout(10000);
      const visited = new Set();
      const merged = [];
      let currentProcessed = false;
      let page = uploadsUrl;
      while (page) {
        if (page.origin !== uploadsUrl.origin || page.pathname !== uploadsUrl.pathname || visited.has(page.href)) {
          return false;
        }
        visited.add(page.href);
        const response = await fetchUploads(page.href, { signal });
        if (!response.ok) return false;
        const { results, next } = await response.json();
        if (!Array.isArray(results)) return false;

        for (const upload of results) {
          if (upload.state !== "merged") continue;
          const processedAt = Math.floor(Date.parse(upload.updated_at) / 1000) * 1000;
          if (upload.build_url === buildUrl && upload.name === uploadName &&
              Date.parse(upload.created_at) >= uploadStartedAt && Number.isFinite(processedAt)) {
            if (processedAt <= statusAt) return true;
            currentProcessed = true;
          }
          merged.push({ name: upload.name, processedAt });
        }
        page = next ? new URL(next, page) : null;
      }
      if (!allowUnchangedSuccess || !currentProcessed ||
          merged.some((upload) => !Number.isFinite(upload.processedAt))) return false;

      // 同じ patch の成功結果は再通知されないことがある。通知時点の最新レポートと、
      // それ以降に処理されたレポートがすべて同じ入力であることを確認する。
      const lastPublishedUpload = merged
        .sort((left, right) => right.processedAt - left.processedAt)
        .find((upload) => upload.processedAt <= statusAt);
      return lastPublishedUpload?.name === uploadName &&
        merged.every((upload) => upload.processedAt < lastPublishedUpload.processedAt || upload.name === uploadName);
    } catch (error) {
      core.info(`Could not verify Codecov upload: ${error.message}`);
      return false;
    }
  }
}
