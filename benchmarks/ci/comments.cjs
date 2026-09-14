// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

const { artifact, listArtifacts } = require('./artifacts.cjs');
const { COMMAND_WORKFLOW, workflowPath } = require('./policy.cjs');
const { publicResults } = require('./results.cjs');

const MAX_REPORT_CHARACTERS = 50_000;
const SMOKE_WORKFLOW = '.github/workflows/benchmark-smoke.yml';

function provenance(condition) {
  if (!condition) throw new Error('Benchmark comment provenance mismatch');
}

async function commentTarget(github, repo, run, core) {
  if (workflowPath(run) === COMMAND_WORKFLOW) {
    provenance(run.event === 'issue_comment');
    const name = `benchmark-request-${run.run_attempt}`;
    const artifacts = await listArtifacts(github, repo, run.id);
    if (!artifacts.some(item => item.name === name && !item.expired)) {
      core.notice('No authorized benchmark request was recorded');
      return null;
    }
    const files = await artifact(github, repo, run.id, name, ['benchmark-request.json']);
    const request = JSON.parse(files['benchmark-request.json']);
    // A publication command does not represent a new measurement.
    if (request.action !== 'run') return null;
    provenance(request.run_id === String(run.id)
      && request.run_attempt === String(run.run_attempt)
      && Number.isSafeInteger(request.pr) && request.pr > 0
      && typeof request.head_sha === 'string' && /^[a-f0-9]{40}$/.test(request.head_sha)
      && typeof request.base_sha === 'string' && /^[a-f0-9]{40}$/.test(request.base_sha));
    return {
      number: request.pr, measuredHead: request.head_sha,
      measuredBase: request.base_sha, profile: 'full',
    };
  }

  if (workflowPath(run) !== SMOKE_WORKFLOW || run.event !== 'pull_request') return null;
  const pr = run.pull_requests?.[0];
  if (!pr) {
    core.notice('No associated PR; results remain in the workflow summary');
    return null;
  }
  provenance(run.pull_requests.length === 1
    && Number.isSafeInteger(pr.number) && pr.number > 0
    && typeof pr.head?.sha === 'string' && /^[a-f0-9]{40}$/.test(pr.head.sha));
  return { number: pr.number, measuredHead: pr.head.sha, profile: 'smoke' };
}

function validateCommentResult(manifest, target, run) {
  provenance(manifest.run_id === String(run.id)
    && manifest.run_attempt === String(run.run_attempt)
    && manifest.head_sha === target.measuredHead
    && manifest.profile === target.profile
    && (target.measuredBase === undefined || manifest.base_sha === target.measuredBase));
}

function commentBody(run, stale, report) {
  let body = `Benchmark run [${run.id}](${run.html_url}) finished: **${run.conclusion}**.`;
  if (stale) {
    body += '\n\nThe PR has advanced; these results describe an older revision.';
  }
  if (run.conclusion === 'success') {
    // Suppress mentions in untrusted workload text and bound comment length.
    body += '\n\n' + report.replaceAll('@', '@\u200b').slice(0, MAX_REPORT_CHARACTERS);
    body += '\n\nThe run artifacts contain the exported results and numeric samples. Raw logs are not uploaded.';
  } else {
    body += '\n\nCheck the run status for validation or infrastructure failures. To see detailed diagnostics, reproduce the failure locally and check the logs for private information before sharing them.';
  }
  return body;
}

async function postComment(github, repo, run, core) {
  const target = await commentTarget(github, repo, run, core);
  if (!target) return;
  const { data: pr } = await github.rest.pulls.get({
    ...repo,
    pull_number: target.number,
  });
  let report;
  if (run.conclusion === 'success') {
    const files = await artifact(
      github, repo, run.id, `benchmark-results-${run.run_attempt}`, ['results.json'],
    );
    const validated = publicResults(files['results.json']);
    validateCommentResult(validated.result.manifest, target, run);
    report = validated.files['report.md'];
  }
  await github.rest.issues.createComment({
    ...repo,
    issue_number: target.number,
    body: commentBody(run, pr.head.sha !== target.measuredHead, report),
  });
}

module.exports = { postComment, commentBody, commentTarget };
