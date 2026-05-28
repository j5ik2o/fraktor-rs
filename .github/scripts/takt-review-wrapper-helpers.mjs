const minIssueLengthForDuplicateMatch = 40; // Avoid matching very short generic phrases across unrelated comments.
const minIssueLengthForPartialDuplicateMatch = 120; // Require enough text before using a prefix-only duplicate match.
const taktWrapperPartialDuplicatePrefixLength = 80; // Existing TAKT comments carry a wrapper marker, so a shorter prefix is acceptable.

export function stripMarkdown(value) {
  return String(value || "")
    .replace(/`/g, "")
    .replace(/\*\*/g, "")
    .trim();
}

export function firstLine(value) {
  return stripMarkdown(value || "").split(/\r?\n/)[0].slice(0, 180);
}

export function firstMeaningfulLine(value, commentHeader = "TAKT Review (Claude)") {
  let sawSourceLabel = false;
  for (const line of stripMarkdown(value || "").split(/\r?\n/)) {
    const trimmed = line.trim();
    const sourceLabelText = sourceLabelSummary(trimmed, commentHeader);
    if (sourceLabelText) {
      return sourceLabelText.slice(0, 180);
    }
    if (sourceLabelText === "") {
      sawSourceLabel = true;
      continue;
    }
    if (sawSourceLabel && isSeverityOnlyLine(trimmed)) {
      continue;
    }
    if (trimmed && !isReviewMetadataLine(trimmed, commentHeader)) {
      return trimmed.slice(0, 180);
    }
  }
  return firstLine(value);
}

export function isReviewMetadataLine(value, commentHeader = "TAKT Review (Claude)") {
  return sourceLabelSummary(value, commentHeader) !== undefined || isSeveritySourceLine(value);
}

export function sourceLabelSummary(value, commentHeader = "TAKT Review (Claude)") {
  const trimmed = String(value || "").trim();
  const labels = new Set([commentHeader, "Claude Code Review"].filter(Boolean));
  for (const label of labels) {
    const labelPrefix = `${label}:`;
    if (trimmed.toLowerCase() === label.toLowerCase() || trimmed.toLowerCase() === labelPrefix.toLowerCase()) {
      return "";
    }
    if (trimmed.toLowerCase().startsWith(labelPrefix.toLowerCase())) {
      return trimmed.slice(labelPrefix.length).trim();
    }
  }

  const match = /^(Claude Code Review|TAKT Review(?: \([^)]+\))?)\s*:\s*(.*)$/i.exec(trimmed);
  if (match) {
    return match[2].trim();
  }
  if (/^(Claude Code Review|TAKT Review(?: \([^)]+\))?)$/i.test(trimmed)) {
    return "";
  }
  return undefined;
}

export function isSeveritySourceLine(value) {
  return /^(p[0-3]|critical|blocker|high|medium|low|info|warning|error|major|minor|nit|suggestion)(\s*\/\s*[\p{L}\p{N}_ -]+){1,3}$/iu.test(
    String(value || "").trim(),
  );
}

export function isSeverityOnlyLine(value) {
  return /^(p[0-3]|critical|blocker|high|medium|low|info|warning|error|major|minor|nit|suggestion)$/iu.test(
    String(value || "").trim(),
  );
}

/**
 * Detect same-line duplicate findings conservatively:
 * exact full issue text first, long-prefix matches only for long issues,
 * and a shorter prefix only for comments known to come from this wrapper.
 */
export function isDuplicateComment(comment, path, line, normalizedIssue) {
  if (comment.path !== path || comment.line !== line || normalizedIssue.length === 0) {
    return false;
  }

  const normalizedBody = normalizeBody(comment.body);
  if (normalizedIssue.length >= minIssueLengthForDuplicateMatch && normalizedBody.includes(normalizedIssue)) {
    return true;
  }

  if (
    normalizedIssue.length >= minIssueLengthForPartialDuplicateMatch &&
    normalizedBody.includes(normalizedIssue.slice(0, minIssueLengthForPartialDuplicateMatch))
  ) {
    return true;
  }

  return (
    isTaktWrapperComment(comment.body) &&
    normalizedIssue.length >= minIssueLengthForDuplicateMatch &&
    normalizedBody.includes(normalizedIssue.slice(0, taktWrapperPartialDuplicatePrefixLength))
  );
}

export function isTaktWrapperComment(value) {
  return String(value || "").includes("<!-- takt-review-wrapper -->");
}

export function normalizeBody(value) {
  return stripMarkdown(value || "").replace(/\s+/g, " ").trim();
}
