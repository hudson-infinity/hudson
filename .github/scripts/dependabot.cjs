const {execFile} = require('node:child_process');
const {promisify} = require('node:util');
const run = promisify(execFile);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

async function requestAutoMerge(repo, number, sha) {
  await run('gh', ['pr', 'merge', String(number), '--repo', `${repo.owner}/${repo.repo}`,
    '--auto', '--squash', '--match-head-commit', sha]);
}

function eligible(pr, repository) {
  return pr.state === 'open' && !pr.draft &&
    pr.user?.login === 'dependabot[bot]' && pr.user?.type === 'Bot' &&
    pr.base?.ref === 'main' && pr.base?.repo?.full_name === repository &&
    pr.head?.repo?.full_name === repository && pr.head.ref.startsWith('dependabot/');
}

async function enableDependabotAutoMerge(github, repo, number, core, merge = requestAutoMerge) {
  const repository = `${repo.owner}/${repo.repo}`;
  const {data: pr} = await github.rest.pulls.get({...repo, pull_number: number});
  if (!eligible(pr, repository)) {
    core.info(`Skipping #${number}: not an open, same-repository Dependabot PR to main.`);
    return;
  }
  const reviews = await github.paginate(github.rest.pulls.listReviews, {
    ...repo, pull_number: number, per_page: 100,
  });
  if (!reviews.some(review => review.user?.login === 'github-actions[bot]' &&
      review.state === 'APPROVED' && review.commit_id === pr.head.sha)) {
    await github.rest.pulls.createReview({
      ...repo, pull_number: number, event: 'APPROVE', commit_id: pr.head.sha,
    });
  }
  // Recheck after the review so a concurrent rebase is handled by its own event.
  const {data: current} = await github.rest.pulls.get({...repo, pull_number: number});
  if (!eligible(current, repository) || current.head.sha !== pr.head.sha) {
    core.info(`Skipping auto-merge for #${number}: the PR changed during approval.`);
    return;
  }
  if (!current.auto_merge) {
    // gh queues native auto-merge or merges an already eligible PR. Both paths
    // enforce branch protection; the expected SHA guards a concurrent rebase.
    await merge(repo, number, current.head.sha);
  }
  core.info(`Approved #${number}; requested protected squash auto-merge.`);
}

async function refreshDependabotBranch(github, repo, number, core, wait = sleep) {
  const repository = `${repo.owner}/${repo.repo}`;
  const {data: pr} = await github.rest.pulls.get({...repo, pull_number: number});
  if (!eligible(pr, repository)) return false;
  // A PR's base.sha is its original base snapshot, not the live branch tip.
  const {data: base} = await github.rest.repos.getBranch({...repo, branch: pr.base.ref});
  const {data: comparison} = await github.rest.repos.compareCommitsWithBasehead({
    ...repo, basehead: `${pr.head.sha}...${base.commit.sha}`,
  });
  let current = pr;
  let updated = false;
  if (comparison.ahead_by > 0) {
    try {
      await github.rest.pulls.updateBranch({
        ...repo, pull_number: number, expected_head_sha: pr.head.sha,
      });
    } catch (error) {
      if (error.status !== 422 && error.status !== 409) throw error;
      core.warning(`Cannot update #${number} yet; a conflict or concurrent update needs another attempt.`);
      return false;
    }
    for (let attempt = 0; attempt < 15; attempt++) {
      await wait(2000);
      current = (await github.rest.pulls.get({...repo, pull_number: number})).data;
      if (!eligible(current, repository)) return false;
      if (current.head.sha !== pr.head.sha) {
        updated = true;
        break;
      }
    }
    if (!updated) {
      core.warning(`Branch update for #${number} has not completed; the next sweep will retry.`);
      return false;
    }
  }
  // GITHUB_TOKEN branch updates do not trigger push/PR workflows. Dispatch them
  // explicitly, and recover a missed dispatch if a previous sweep was stopped.
  const checks = await github.paginate(github.rest.checks.listForRef, {
    ...repo, ref: current.head.sha, filter: 'latest', per_page: 100,
  });
  const names = new Set(checks.filter(check => check.app?.slug === 'github-actions').map(check => check.name));
  if (updated || !['CI', 'Repository policy and workflows', 'Rust, PostgreSQL, and Temporal'].some(name => names.has(name))) {
    await github.rest.actions.createWorkflowDispatch({
      ...repo, workflow_id: 'ci.yml', ref: current.head.ref,
    });
  }
  if (updated || !names.has('PR policy')) {
    await github.rest.actions.createWorkflowDispatch({
      ...repo, workflow_id: 'pr-policy.yml', ref: current.head.ref,
      inputs: {pull_request_number: String(number)},
    });
  }
  return true;
}

module.exports = {eligible, enableDependabotAutoMerge, refreshDependabotBranch};
