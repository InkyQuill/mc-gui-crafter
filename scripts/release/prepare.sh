#!/usr/bin/env bash
set -euo pipefail
# Only a merged release-please PR at this exact commit can authorize a release.
pr=$(gh api "repos/$GITHUB_REPOSITORY/commits/$GITHUB_SHA/pulls" --jq \
  '.[] | select(.merged_at != null and .base.ref == "main" and (.head.ref | startswith("release-please--branches--main"))) | .number')
[[ -n "$pr" ]] || exit 0
[[ "$pr" =~ ^[0-9]+$ ]]
version=$(python3 scripts/release/check-version.py)
tag="v$version"
git merge-base --is-ancestor "$GITHUB_SHA" origin/main
if git rev-parse "refs/tags/$tag^{commit}" >/dev/null 2>&1; then
  test "$(git rev-parse "refs/tags/$tag^{commit}")" = "$GITHUB_SHA"
else
  gh api --method POST "repos/$GITHUB_REPOSITORY/git/refs" -f "ref=refs/tags/$tag" -f "sha=$GITHUB_SHA"
fi
# workflow_dispatch is explicit because GITHUB_TOKEN tag pushes don't start workflows.
gh workflow run release.yml --ref "$tag"
gh label create 'autorelease: tagged' --color 0E8A16 --force
gh pr edit "$pr" --remove-label 'autorelease: pending' --add-label 'autorelease: tagged'
