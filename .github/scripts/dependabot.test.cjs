const test = require('node:test');
const assert = require('node:assert/strict');
const {eligible, enableDependabotAutoMerge} = require('./dependabot.cjs');

const repo = {owner: 'hudson-infinity', repo: 'hudson'};
const repository = 'hudson-infinity/hudson';
const pr = () => ({
  number: 1, node_id: 'PR_node', state: 'open', draft: false,
  user: {login: 'dependabot[bot]', type: 'Bot'},
  head: {sha: 'abc', ref: 'dependabot/cargo/update', repo: {full_name: repository}},
  base: {ref: 'main', repo: {full_name: repository}}, auto_merge: null,
});

test('only accepts genuine same-repository Dependabot updates to main', () => {
  assert.equal(eligible(pr(), repository), true);
  const variants = [
    {user: {login: 'member', type: 'User'}},
    {user: {login: 'dependabot[bot]', type: 'User'}},
    {state: 'closed'}, {draft: true},
    {head: {...pr().head, repo: {full_name: 'fork/hudson'}}},
    {head: {...pr().head, ref: 'feature'}},
    {base: {...pr().base, ref: 'release'}},
    {base: {...pr().base, repo: {full_name: 'another/repo'}}},
  ];
  for (const change of variants) assert.equal(eligible({...pr(), ...change}, repository), false);
});

function fixture(first = pr(), second = first, reviews = []) {
  const calls = {reviews: [], mutations: []};
  let reads = 0;
  return {calls, merge: async (...args) => calls.mutations.push(args), github: {
    rest: {pulls: {
      get: async () => ({data: reads++ === 0 ? first : second}),
      listReviews: 'reviews',
      createReview: async value => calls.reviews.push(value),
    }},
    paginate: async () => reviews,
  }};
}
const core = {info() {}};

test('pins the approval to the current commit and requests native squash auto-merge', async () => {
  const {github, calls, merge} = fixture();
  await enableDependabotAutoMerge(github, repo, 1, core, merge);
  assert.equal(calls.reviews[0].commit_id, 'abc');
  assert.equal(calls.reviews[0].event, 'APPROVE');
  assert.deepEqual(calls.mutations[0], [repo, 1, 'abc']);
});

test('does not enable auto-merge on a concurrently replaced or closed PR', async () => {
  for (const current of [{...pr(), head: {...pr().head, sha: 'new'}}, {...pr(), state: 'closed'}]) {
    const {github, calls, merge} = fixture(pr(), current);
    await enableDependabotAutoMerge(github, repo, 1, core, merge);
    assert.equal(calls.mutations.length, 0);
  }
});

test('repeated events do not duplicate bot approval or re-enable auto-merge', async () => {
  const current = {...pr(), auto_merge: {merge_method: 'squash'}};
  const {github, calls, merge} = fixture(current, current, [
    {user: {login: 'github-actions[bot]'}, state: 'APPROVED', commit_id: 'abc'},
  ]);
  await enableDependabotAutoMerge(github, repo, 1, core, merge);
  assert.equal(calls.reviews.length, 0);
  assert.equal(calls.mutations.length, 0);
});

test('a human approval or an old bot approval does not count for the new commit', async () => {
  const {github, calls, merge} = fixture(pr(), pr(), [
    {user: {login: 'member'}, state: 'APPROVED', commit_id: 'abc'},
    {user: {login: 'github-actions[bot]'}, state: 'APPROVED', commit_id: 'old'},
  ]);
  await enableDependabotAutoMerge(github, repo, 1, core, merge);
  assert.equal(calls.reviews.length, 1);
});

test('untrusted PRs perform no review or mutation', async () => {
  const {github, calls, merge} = fixture({...pr(), user: {login: 'contributor', type: 'User'}});
  await enableDependabotAutoMerge(github, repo, 1, core, merge);
  assert.equal(calls.reviews.length, 0);
  assert.equal(calls.mutations.length, 0);
});
