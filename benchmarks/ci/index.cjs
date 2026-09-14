// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// Workflow entry points. Privileged jobs load these modules from the default
// branch; downloaded benchmark artifacts are data, never executable code.
const fs = require('node:fs');
const {
  parseCommand, workflowPath, authorizedPR, validateSelection,
} = require('./policy.cjs');
const { selectedResults, publishResults } = require('./publication.cjs');
const { postComment } = require('./comments.cjs');

async function request({ github, context, core }) {
  const command = parseCommand(context.payload.comment?.body || '');
  if (!command || !context.payload.issue?.pull_request) return;

  const pr = await authorizedPR(github, context, command.action === 'commit');
  const { data: comparison } = await github.rest.repos.compareCommitsWithBasehead({
    ...context.repo,
    basehead: `${pr.base.sha}...${pr.head.sha}`,
  });
  const provenance = {
    pr: pr.number,
    head_sha: pr.head.sha,
    base_sha: comparison.merge_base_commit.sha,
    target_sha: pr.base.sha,
    run_id: String(context.runId),
    run_attempt: process.env.GITHUB_RUN_ATTEMPT,
    actor: context.payload.comment.user.login,
    action: command.action,
  };
  fs.writeFileSync('benchmark-request.json', JSON.stringify(provenance));
  const outputs = { ...provenance, selected_run: command.runId || '' };
  for (const [key, value] of Object.entries(outputs)) {
    core.setOutput(key, value);
  }
}

async function commit({ github, context, core }) {
  const command = parseCommand(context.payload.comment?.body || '');
  if (command?.action !== 'commit') {
    throw new Error('Explicit commit command required');
  }
  const pr = await authorizedPR(github, context, true);
  const { run, files } = await selectedResults(github, context.repo, pr, command.runId);
  const newCommit = await publishResults(github, context.repo, pr, run, files);
  core.notice(`Recorded benchmark ${run.id} in ${newCommit.sha}`);
}

async function comment({ github, context, core }) {
  const { data: run } = await github.rest.actions.getWorkflowRun({
    ...context.repo,
    run_id: context.payload.workflow_run.id,
  });
  await postComment(github, context.repo, run, core);
}

module.exports = { parseCommand, workflowPath, validateSelection, request, commit, comment };
