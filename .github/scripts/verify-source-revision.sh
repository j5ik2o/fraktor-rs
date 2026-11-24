#!/usr/bin/env bash
set -euo pipefail

checkout_sha="$(git rev-parse HEAD)"
if [[ "${EVENT_NAME}" == "pull_request" ]]; then
  if [[ ! "${PR_NUMBER}" =~ ^[0-9]+$ || ! "${BASE_SHA}" =~ ^[0-9a-f]{40}$ || ! "${PR_HEAD_SHA}" =~ ^[0-9a-f]{40}$ ]]; then
    echo "::error::Pull request number, base SHA, or head SHA is invalid"
    exit 1
  fi

  matches_pr() {
    [[ "$(git show -s --format=%P "$1")" == "${BASE_SHA} ${PR_HEAD_SHA}" ]]
  }

  echo "PR #${PR_NUMBER}: expected parents ${BASE_SHA} ${PR_HEAD_SHA}"
  echo "Initial checkout ${checkout_sha}: parents $(git show -s --format=%P "${checkout_sha}")"
  if ! matches_pr "${checkout_sha}"; then
    # ベース変更直後は refs/pull/<n>/merge が旧ベースを指すことがある。
    for attempt in {1..6}; do
      if git fetch --no-tags origin "refs/pull/${PR_NUMBER}/merge"; then
        checkout_sha="$(git rev-parse FETCH_HEAD)"
        if matches_pr "${checkout_sha}"; then
          break
        fi
      fi
      echo "Waiting for matching PR merge ref (attempt ${attempt}/6)"
      sleep 5
    done
    if ! matches_pr "${checkout_sha}"; then
      echo "::error::No PR merge commit with the expected base and head parents"
      exit 1
    fi
    git checkout --detach "${checkout_sha}"
  fi
fi

echo "CI checkout ${checkout_sha}: parents $(git show -s --format=%P HEAD)"
echo "checkout_sha=${checkout_sha}" >> "${GITHUB_OUTPUT}"
