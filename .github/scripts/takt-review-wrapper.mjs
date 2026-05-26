#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const env = process.env;
const repo = requiredEnv("GITHUB_REPOSITORY");
const token = requiredEnv("GITHUB_TOKEN");
const prNumber = requiredEnv("PR_NUMBER");
const workflow = env.TAKT_WORKFLOW || "review-default";
const provider = env.TAKT_PROVIDER || "claude-sdk";
const model = env.TAKT_MODEL || "";
const maxComments = Number.parseInt(env.TAKT_MAX_COMMENTS || "5", 10);
const [owner, repoName] = repo.split("/");

if (!owner || !repoName) {
  throw new Error(`Invalid GITHUB_REPOSITORY: ${repo}`);
}

if (env.GITHUB_EVENT_NAME === "issue_comment" && env.COMMENT_BODY && !/^@takt(?:\s|$)/.test(env.COMMENT_BODY)) {
  console.log("Comment does not contain an @takt command. Skipping.");
  process.exit(0);
}

const pr = ghJson(["pr", "view", prNumber, "--json", "title,body,headRefOid,baseRefName,headRefName,url"]);
const diff = ghText(["pr", "diff", prNumber]);
const changedFiles = ghPaginatedJson(`repos/${repo}/pulls/${prNumber}/files`);
const existingComments = ghPaginatedJson(`repos/${repo}/pulls/${prNumber}/comments`);
const allowedLines = collectReviewableLines(changedFiles);

const task = buildTask({ repo, prNumber, pr, diff, changedFiles, existingComments, maxComments });
const taskFile = join(mkdtempSync(join(tmpdir(), "takt-review-")), "task.md");
writeFileSync(taskFile, task, "utf8");

const runEnv = {
  ...env,
  ANTHROPIC_API_KEY: env.ANTHROPIC_API_KEY || env.TAKT_ANTHROPIC_API_KEY || "",
  TAKT_ANTHROPIC_API_KEY: env.TAKT_ANTHROPIC_API_KEY || env.ANTHROPIC_API_KEY || "",
  GITHUB_TOKEN: token,
  GH_TOKEN: token,
};

const args = [
  "--yes",
  "takt@0.42.0",
  "--pipeline",
  "--skip-git",
  "--workflow",
  workflow,
  "--provider",
  provider,
  "--task",
  readFileSync(taskFile, "utf8"),
];

if (model) {
  args.push("--model", model);
}

console.log(`Running TAKT workflow "${workflow}" with provider "${provider}" for PR #${prNumber}`);
const result = spawnSync("npx", args, {
  env: runEnv,
  encoding: "utf8",
  stdio: ["ignore", "pipe", "pipe"],
  maxBuffer: 1024 * 1024 * 50,
});

if (result.stdout) {
  console.log(result.stdout);
}
if (result.stderr) {
  console.error(result.stderr);
}
if (result.status !== 0) {
  throw new Error(`takt exited with code ${result.status}`);
}

const report = readLatestReport();
if (!report) {
  console.log("No TAKT report found; no review comments will be posted.");
  process.exit(0);
}

const parsedFindings = parseFindings(report.content);
const reviewComments = parsedFindings
  .map((finding) => toReviewComment(finding, allowedLines, existingComments))
  .filter(Boolean)
  .slice(0, maxComments);

if (reviewComments.length === 0) {
  console.log("TAKT produced no actionable inline findings on changed lines.");
  process.exit(0);
}

await postReview({
  commit_id: pr.headRefOid,
  event: "COMMENT",
  body: `TAKT review posted ${reviewComments.length} inline finding(s).\n\nSource report: ${report.relativePath}`,
  comments: reviewComments,
});

console.log(`Posted ${reviewComments.length} TAKT inline review comment(s).`);

function requiredEnv(name) {
  const value = env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function ghJson(args) {
  return JSON.parse(ghText(args));
}

function ghText(args) {
  return execFileSync("gh", args, {
    env: { ...env, GH_TOKEN: token, GITHUB_TOKEN: token },
    encoding: "utf8",
    maxBuffer: 1024 * 1024 * 50,
  });
}

function ghPaginatedJson(endpoint) {
  const pages = ghJson(["api", "--paginate", "--slurp", endpoint]);
  return pages.flatMap((page) => (Array.isArray(page) ? page : [page]));
}

function buildTask({ repo, prNumber, pr, diff, changedFiles, existingComments, maxComments }) {
  const existing = existingComments
    .slice(-80)
    .map((comment) => `- ${comment.path}:${comment.line || comment.original_line || "?"}: ${firstLine(comment.body)}`)
    .join("\n");
  const fileList = changedFiles.map((file) => `- ${file.filename}`).join("\n");

  return `Review PR #${prNumber}: ${pr.title}

Repository: ${repo}
PR URL: ${pr.url}
Base branch: ${pr.baseRefName}
Head branch: ${pr.headRefName}
Head SHA: ${pr.headRefOid}

Use the PR diff below as the authoritative review target. Review only changed behavior.
Do not run builds or tests. Do not modify files. Do not create commits.
Postable findings must be concrete bugs, security issues, behavioral regressions, or maintainability problems that justify an inline PR comment.
Do not report style-only nits or duplicate the existing comments listed below.
If there are no actionable findings, return APPROVE with no findings.

For every actionable finding, include an exact changed-line location in the final Review Summary table as \`path:line\`.
The line must be a RIGHT-side line present in the diff. Limit findings to at most ${maxComments}.

Existing inline comments:
${existing || "- none"}

Changed files:
${fileList || "- none"}

PR body:
${pr.body || "(empty)"}

PR diff:
\`\`\`diff
${diff}
\`\`\`
`;
}

function collectReviewableLines(files) {
  const byPath = new Map();
  for (const file of files) {
    const lines = new Set();
    const patch = file.patch || "";
    let newLine = 0;
    for (const rawLine of patch.split("\n")) {
      const hunk = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(rawLine);
      if (hunk) {
        newLine = Number.parseInt(hunk[1], 10);
        continue;
      }
      if (!rawLine) {
        continue;
      }
      if (rawLine.startsWith("+") && !rawLine.startsWith("+++")) {
        lines.add(newLine);
        newLine += 1;
      } else if (!rawLine.startsWith("-")) {
        lines.add(newLine);
        newLine += 1;
      }
    }
    byPath.set(file.filename, lines);
  }
  return byPath;
}

function readLatestReport() {
  const runsDir = ".takt/runs";
  if (!existsSync(runsDir)) {
    return undefined;
  }

  const runDirs = readdirSync(runsDir)
    .map((name) => join(runsDir, name))
    .filter((path) => statSync(path).isDirectory())
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);

  for (const runDir of runDirs) {
    const summary = join(runDir, "reports", "review-summary.md");
    if (existsSync(summary)) {
      return {
        content: readFileSync(summary, "utf8"),
        relativePath: summary,
      };
    }

    const reportsDir = join(runDir, "reports");
    if (!existsSync(reportsDir)) {
      continue;
    }
    const reports = readdirSync(reportsDir)
      .filter((name) => name.endsWith(".md"))
      .map((name) => join(reportsDir, name));
    if (reports.length > 0) {
      const content = reports.map((path) => readFileSync(path, "utf8")).join("\n\n");
      return { content, relativePath: reportsDir };
    }
  }

  return undefined;
}

function parseFindings(markdown) {
  const findings = [];
  const lines = markdown.split(/\r?\n/);
  for (let i = 0; i < lines.length - 1; i += 1) {
    if (!isTableLine(lines[i]) || !isSeparatorLine(lines[i + 1])) {
      continue;
    }

    const headers = splitTableLine(lines[i]).map(normalizeHeader);
    const locationIndex = headers.indexOf("location");
    const issueIndex = headers.indexOf("issue");
    if (locationIndex === -1 || issueIndex === -1) {
      continue;
    }

    const severityIndex = headers.indexOf("severity");
    const sourceIndex = headers.indexOf("source");
    const suggestionIndex = headers.findIndex((header) => header.includes("suggestion") || header.includes("fix"));

    for (let rowIndex = i + 2; rowIndex < lines.length && isTableLine(lines[rowIndex]); rowIndex += 1) {
      const cells = splitTableLine(lines[rowIndex]);
      const finding = {
        location: cells[locationIndex] || "",
        issue: cells[issueIndex] || "",
        severity: severityIndex >= 0 ? cells[severityIndex] || "" : "",
        source: sourceIndex >= 0 ? cells[sourceIndex] || "" : "",
        suggestion: suggestionIndex >= 0 ? cells[suggestionIndex] || "" : "",
      };
      if (finding.location && finding.issue && !isPlaceholder(finding.location) && !isPlaceholder(finding.issue)) {
        findings.push(finding);
      }
    }
  }
  return findings;
}

function toReviewComment(finding, allowedLines, existingComments) {
  const location = stripMarkdown(finding.location);
  const match = /([^:\s`]+(?:\/[^:\s`]+)*):(\d+)/.exec(location);
  if (!match) {
    console.log(`Skipping finding without file:line location: ${finding.location}`);
    return undefined;
  }

  const path = match[1];
  const line = Number.parseInt(match[2], 10);
  const allowed = allowedLines.get(path);
  if (!allowed?.has(line)) {
    console.log(`Skipping finding outside reviewable diff lines: ${path}:${line}`);
    return undefined;
  }

  const body = formatCommentBody(finding);
  const duplicate = existingComments.some((comment) => {
    const commentLine = comment.line || comment.original_line;
    return comment.path === path && commentLine === line && normalizeBody(comment.body).includes(normalizeBody(finding.issue).slice(0, 80));
  });
  if (duplicate) {
    console.log(`Skipping duplicate finding: ${path}:${line}`);
    return undefined;
  }

  return { path, line, side: "RIGHT", body };
}

async function postReview(payload) {
  const response = await fetch(`https://api.github.com/repos/${owner}/${repoName}/pulls/${prNumber}/reviews`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      accept: "application/vnd.github+json",
      "x-github-api-version": "2022-11-28",
      "content-type": "application/json",
      "user-agent": "takt-review-wrapper",
    },
    body: JSON.stringify(payload),
  });

  const text = await response.text();
  if (!response.ok) {
    console.error(text);
    throw new Error(`failed to post GitHub review: ${response.status}`);
  }
  console.log(text);
}

function isTableLine(line) {
  return /^\s*\|.*\|\s*$/.test(line);
}

function isSeparatorLine(line) {
  return /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(line);
}

function splitTableLine(line) {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => stripMarkdown(cell.trim()));
}

function normalizeHeader(header) {
  return header.toLowerCase().replace(/[^a-z]/g, "");
}

function stripMarkdown(value) {
  return value.replace(/`/g, "").replace(/\*\*/g, "").trim();
}

function firstLine(value) {
  return stripMarkdown(value || "").split(/\r?\n/)[0].slice(0, 180);
}

function formatCommentBody(finding) {
  const parts = [];
  const prefix = [finding.severity, finding.source].filter(Boolean).join(" / ");
  if (prefix) {
    parts.push(`**${prefix}**`);
  }
  parts.push(stripMarkdown(finding.issue));
  if (finding.suggestion && !isPlaceholder(finding.suggestion)) {
    parts.push(`\n提案: ${stripMarkdown(finding.suggestion)}`);
  }
  parts.push("\n<!-- takt-review-wrapper -->");
  return parts.join("\n\n");
}

function normalizeBody(value) {
  return stripMarkdown(value || "").replace(/\s+/g, " ").trim();
}

function isPlaceholder(value) {
  return /^{.*}$/.test(value.trim()) || value.includes("Description") || value.includes("file:line");
}
