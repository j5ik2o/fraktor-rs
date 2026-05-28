import assert from "node:assert/strict";
import test from "node:test";

import {
  firstMeaningfulLine,
  isDuplicateComment,
  isReviewMetadataLine,
  isSeverityOnlyLine,
  isSeveritySourceLine,
  isTaktWrapperComment,
  normalizeBody,
  sourceLabelSummary,
} from "./takt-review-wrapper-helpers.mjs";

test("firstMeaningfulLine skips TAKT metadata and returns the finding text", () => {
  const body = ["**TAKT Review (Claude)**", "", "**medium / security**", "", "Actual finding text"].join("\n");

  assert.equal(firstMeaningfulLine(body), "Actual finding text");
});

test("firstMeaningfulLine skips severity-only TAKT metadata after the source label", () => {
  const body = ["**TAKT Review (Claude)**", "", "**High**", "", "Actual finding text"].join("\n");

  assert.equal(firstMeaningfulLine(body), "Actual finding text");
});

test("firstMeaningfulLine does not skip severity-like words outside TAKT metadata context", () => {
  assert.equal(firstMeaningfulLine("medium\n\nActual finding text"), "medium");
});

test("firstMeaningfulLine keeps the summary from a Claude label line", () => {
  assert.equal(firstMeaningfulLine("**Claude Code Review**: Keep this summary"), "Keep this summary");
});

test("firstMeaningfulLine honors a custom TAKT comment header", () => {
  assert.equal(firstMeaningfulLine("**Codex Review**: Custom summary", "Codex Review"), "Custom summary");
});

test("sourceLabelSummary handles label-only and label-with-summary lines", () => {
  assert.equal(sourceLabelSummary("TAKT Review (Claude)"), "");
  assert.equal(sourceLabelSummary("TAKT Review (Sonnet): Finding summary"), "Finding summary");
  assert.equal(sourceLabelSummary("Codex Review:", "Codex Review"), "");
  assert.equal(sourceLabelSummary("Not a label"), undefined);
});

test("isReviewMetadataLine detects source labels and severity/source metadata", () => {
  assert.equal(isReviewMetadataLine("Claude Code Review"), true);
  assert.equal(isReviewMetadataLine("medium / security"), true);
  assert.equal(isReviewMetadataLine("Actual finding text"), false);
});

test("isSeveritySourceLine requires a slash-separated source token", () => {
  assert.equal(isSeveritySourceLine("medium / security"), true);
  assert.equal(isSeveritySourceLine("P2 / maintainability"), true);
  assert.equal(isSeveritySourceLine("High / セキュリティ"), true);
  assert.equal(isSeveritySourceLine("medium"), false);
  assert.equal(isSeveritySourceLine("suggestion"), false);
});

test("isSeverityOnlyLine detects standalone severity metadata", () => {
  assert.equal(isSeverityOnlyLine("High"), true);
  assert.equal(isSeverityOnlyLine("medium"), true);
  assert.equal(isSeverityOnlyLine("Actual finding text"), false);
});

test("isDuplicateComment ignores short generic issue text", () => {
  const issue = "x".repeat(39);
  const comment = { path: "file.js", line: 10, body: issue };

  assert.equal(isDuplicateComment(comment, "file.js", 10, issue), false);
});

test("isDuplicateComment matches full issue text at the same location", () => {
  const issue = "x".repeat(40);
  const comment = { path: "file.js", line: 10, body: `prefix ${issue} suffix` };

  assert.equal(isDuplicateComment(comment, "file.js", 10, issue), true);
  assert.equal(isDuplicateComment(comment, "other.js", 10, issue), false);
  assert.equal(isDuplicateComment(comment, "file.js", 11, issue), false);
});

test("isDuplicateComment uses long prefixes only for long issue text", () => {
  const issue119 = "a".repeat(119);
  const issue120 = "b".repeat(120);
  const comment119 = { path: "file.js", line: 10, body: issue119.slice(0, 118) };
  const comment120 = { path: "file.js", line: 10, body: issue120.slice(0, 120) };

  assert.equal(isDuplicateComment(comment119, "file.js", 10, issue119), false);
  assert.equal(isDuplicateComment(comment120, "file.js", 10, issue120), true);
});

test("isDuplicateComment allows shorter prefixes only for wrapper comments", () => {
  const issue = "c".repeat(90);
  const wrapperComment = {
    path: "file.js",
    line: 10,
    body: `${issue.slice(0, 80)}\n<!-- takt-review-wrapper -->`,
  };
  const plainComment = { path: "file.js", line: 10, body: issue.slice(0, 80) };

  assert.equal(isDuplicateComment(wrapperComment, "file.js", 10, issue), true);
  assert.equal(isDuplicateComment(plainComment, "file.js", 10, issue), false);
});

test("normalizeBody and isTaktWrapperComment handle wrapper body text", () => {
  assert.equal(normalizeBody("**Finding**\n\n`code`"), "Finding code");
  assert.equal(isTaktWrapperComment("body\n<!-- takt-review-wrapper -->"), true);
  assert.equal(isTaktWrapperComment("body"), false);
});
