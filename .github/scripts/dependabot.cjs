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

module.exports = {eligible, enableDependabotAutoMerge};
const {execFile} = require('node:child_process');
const {promisify} = require('node:util');
const run = promisify(execFile);

async function requestAutoMerge(repo, number, sha) {
  await run('gh', ['pr', 'merge', String(number), '--repo', `${repo.owner}/${repo.repo}`,
    '--auto', '--squash', '--match-head-commit', sha]);
}
