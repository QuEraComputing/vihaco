// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

const { artifact } = require('./artifacts.cjs');
const { validateRun, validateSelection } = require('./policy.cjs');
const { publicResults } = require('./results.cjs');

async function selectedResults(github, repo, pr, runId) {
  const { data: run } = await github.rest.actions.getWorkflowRun({
    ...repo,
    run_id: runId,
  });
  validateRun(run);
  const provenance = await artifact(
    github, repo, run.id, `benchmark-request-${run.run_attempt}`, ['benchmark-request.json'],
  );
  const downloaded = await artifact(
    github, repo, run.id, `benchmark-results-${run.run_attempt}`, ['results.json'],
  );
  const { result, files } = publicResults(downloaded['results.json']);
  validateSelection(
    JSON.parse(provenance['benchmark-request.json']),
    result,
    pr,
    run,
  );
  return { run, files };
}

async function reportTree(github, repo, pr, run, files) {
  const { data: head } = await github.rest.git.getCommit({
    ...repo,
    commit_sha: pr.head.sha,
  });
  const entries = [];
  for (const [name, content] of Object.entries(files)) {
    const { data: blob } = await github.rest.git.createBlob({
      ...repo,
      encoding: 'base64',
      content: Buffer.from(content).toString('base64'),
    });
    entries.push({
      path: `benchmarks/results/pr-${pr.number}/run-${run.id}/${name}`,
      mode: '100644',
      type: 'blob',
      sha: blob.sha,
    });
  }
  const { data: tree } = await github.rest.git.createTree({
    ...repo,
    base_tree: head.tree.sha,
    tree: entries,
  });
  return tree;
}

async function publishResults(github, repo, pr, run, files) {
  const tree = await reportTree(github, repo, pr, run, files);
  const { data: commit } = await github.rest.git.createCommit({
    ...repo,
    message: `chore: record benchmark results for PR #${pr.number}`,
    tree: tree.sha,
    parents: [pr.head.sha],
  });
  // The measured head remains the parent. A newer PR commit makes this
  // non-forced update fail, so publication cannot overwrite newer work.
  await github.rest.git.updateRef({
    ...repo,
    ref: `heads/${pr.head.ref}`,
    sha: commit.sha,
    force: false,
  });
  return commit;
}

module.exports = { selectedResults, publishResults };
