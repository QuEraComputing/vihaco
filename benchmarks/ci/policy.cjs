// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// Pure command/provenance checks and GitHub authorization.
const COMMAND_WORKFLOW = '.github/workflows/benchmark-command.yml';
const WRITE_PERMISSIONS = new Set(['write', 'maintain', 'admin']);

function parseCommand(body) {
  const text = body.trim();
  if (text === '@github-actions run benchmark') {
    return { action: 'run' };
  }
  const match = /^@github-actions commit benchmark ([1-9][0-9]*)$/.exec(text);
  return match ? { action: 'commit', runId: match[1] } : null;
}

function workflowPath(run) {
  return (run.path || '').split('@')[0];
}

async function authorizedPR(github, context, authorOnly = false) {
  if (!context.payload.issue?.pull_request) {
    throw new Error('Command requires a PR comment');
  }
  const { data: pr } = await github.rest.pulls.get({
    ...context.repo,
    pull_number: context.payload.issue.number,
  });
  const actor = context.payload.comment.user.login;
  const { data: permission } = await github.rest.repos.getCollaboratorPermissionLevel({
    ...context.repo,
    username: actor,
  });
  if (!WRITE_PERMISSIONS.has(permission.permission)) {
    throw new Error('Write access is required');
  }
  if (pr.state !== 'open' || pr.head.repo?.full_name !== pr.base.repo.full_name) {
    throw new Error('An open PR on a branch in this repository is required');
  }
  if (authorOnly && pr.user.login !== actor) {
    throw new Error('Only the PR author can request a results commit');
  }
  return pr;
}

function validateRun(run) {
  if (workflowPath(run) !== COMMAND_WORKFLOW ||
      run.event !== 'issue_comment' || run.conclusion !== 'success') {
    throw new Error('Select a successful benchmark command run');
  }
}

function matchesAttempt(record, run) {
  return record.run_id === String(run.id) &&
    String(record.run_attempt) === String(run.run_attempt);
}

function validateSelection(request, result, pr, run) {
  const manifest = result.manifest;
  // Bind results to an authorized request and exact run attempt.
  const matchingRequest = request.action === 'run' && request.pr === pr.number &&
    request.head_sha === pr.head.sha && matchesAttempt(request, run);
  const matchingMeasurement = manifest.head_sha === request.head_sha &&
    manifest.base_sha === request.base_sha && matchesAttempt(manifest, run);
  const completeComparison = manifest.profile === 'full' && !manifest.dirty &&
    result.base_status === 'available' && result.candidate &&
    Object.keys(result.candidate).length > 0 &&
    /^[a-f0-9]{64}$/.test(manifest.suite_sha256);

  if (!matchingRequest || !matchingMeasurement || !completeComparison) {
    throw new Error('Run must be a successful full comparison for this PR and its current head');
  }
}

module.exports = {
  COMMAND_WORKFLOW, parseCommand, workflowPath, authorizedPR, validateRun, validateSelection,
};
