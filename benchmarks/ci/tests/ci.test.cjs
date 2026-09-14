// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { parseCommand, workflowPath, validateSelection, request, commit } = require('../index.cjs');
const { selectArtifact } = require('../artifacts.cjs');
const { commentBody, commentTarget, postComment } = require('../comments.cjs');
const { COMMAND_WORKFLOW, validateRun } = require('../policy.cjs');
const { publicResults } = require('../results.cjs');

// Fresh fixtures prevent one rejection case from changing another test's inputs.
function selection() {
  const request = {
    action: 'run',
    pr: 5,
    head_sha: 'b'.repeat(40),
    base_sha: 'c'.repeat(40),
    run_id: '123',
    run_attempt: '1',
  };
  const estimate = {
    mean_ns: 100, lower_ns: 90, upper_ns: 110, samples: 10,
    uncertainty: '95% Criterion bootstrap mean interval',
  };
  const vm = { 'workload/short/sst/program': estimate, 'workload/short/sst/cpu-program': estimate };
  const result = {
    manifest: {
      schema_version: 3,
      run_id: request.run_id,
      run_attempt: request.run_attempt,
      head_sha: request.head_sha,
      base_sha: request.base_sha,
      reference_policy: 'once per comparison; native Rust uses the candidate suite build',
      created_at: '2026-09-04T00:00:00+00:00',
      workloads: { workload: 'd'.repeat(64) },
      environment: {
        platform: 'Linux', machine: 'x86_64', python: '3.12.0', rust: '1.90.0',
        cargo: '1.90.0', cpu_count: 4, build_flags_source: 'unset',
      },
      profile: 'full',
      dirty: false,
      suite_sha256: 'a'.repeat(64),
    },
    base_status: 'available',
    candidate: vm,
    base: vm,
    references: { 'workload/short/rust/program': estimate, 'workload/short/python/program': estimate },
  };
  const pr = {
    number: 5,
    state: 'open',
    user: { login: 'author' },
    head: { sha: request.head_sha, ref: 'feature', repo: { full_name: 'owner/repo' } },
    base: { repo: { full_name: 'owner/repo' } },
  };
  const run = {
    id: 123,
    run_attempt: 1,
    path: COMMAND_WORKFLOW + '@main',
    event: 'issue_comment',
    conclusion: 'success',
    html_url: 'https://github.com/owner/repo/actions/runs/123',
  };
  return { request, result, pr, run };
}

function context(body, actor = 'author') {
  return {
    repo: { owner: 'owner', repo: 'repo' },
    payload: {
      issue: { number: 5, pull_request: {} },
      comment: { body, user: { login: actor } },
    },
  };
}

function github(permission = 'write', fork = false) {
  const { pr } = selection();
  if (fork) pr.head.repo.full_name = 'fork/repo';
  return {
    rest: {
      pulls: { get: async () => ({ data: pr }) },
      repos: {
        getCollaboratorPermissionLevel: async () => ({ data: { permission } }),
      },
    },
  };
}

function archive(directory, name, files) {
  for (const [filename, content] of Object.entries(files)) {
    fs.writeFileSync(path.join(directory, filename), content);
  }
  execFileSync('zip', ['-q', name, ...Object.keys(files)], { cwd: directory });
  return fs.readFileSync(path.join(directory, name));
}

function attachArtifacts(api, directory, fixture) {
  const archives = new Map([
    [1, archive(directory, 'request.zip', {
      'benchmark-request.json': JSON.stringify(fixture.request),
    })],
    [2, archive(directory, 'results.zip', {
      'report.md': '# Results\n',
      'results.json': JSON.stringify(fixture.result),
    })],
  ]);
  api.rest.actions = {
    getWorkflowRun: async () => ({ data: fixture.run }),
    listWorkflowRunArtifacts() {},
    downloadArtifact: async ({ artifact_id }) => ({ data: archives.get(artifact_id) }),
  };
  api.paginate = async () => [
    { id: 1, name: 'benchmark-request-1', size_in_bytes: 100 },
    { id: 2, name: 'benchmark-results-1', size_in_bytes: 100 },
  ];
}

function publicationGit(updateRef) {
  return {
    getCommit: async () => ({ data: { tree: { sha: 'tree' } } }),
    createBlob: async () => ({ data: { sha: 'blob' } }),
    createTree: async ({ tree, base_tree }) => {
      assert.equal(base_tree, 'tree');
      assert.deepEqual(tree.map(entry => entry.path).sort(), [
        'benchmarks/results/pr-5/run-123/report.md',
        'benchmarks/results/pr-5/run-123/results.json',
      ]);
      return { data: { sha: 'new-tree' } };
    },
    createCommit: async ({ parents }) => {
      assert.deepEqual(parents, ['b'.repeat(40)]);
      return { data: { sha: 'commit' } };
    },
    updateRef,
  };
}

test('workflow identity accepts GitHub ref-qualified paths', () => {
  assert.equal(workflowPath(selection().run), COMMAND_WORKFLOW);
});

test('commands must be explicit whole comments', () => {
  assert.deepEqual(parseCommand('@github-actions run benchmark'), { action: 'run' });
  assert.deepEqual(parseCommand('@github-actions commit benchmark 123'), {
    action: 'commit',
    runId: '123',
  });
  const invalid = [
    'please @github-actions run benchmark',
    '@github-actions commit benchmark 1; echo bad',
    '@github-actions commit benchmark',
    '@github-actions commit benchmark -1',
  ];
  for (const body of invalid) {
    assert.equal(parseCommand(body), null, body);
  }
});

test('publishing requires a successful command workflow', () => {
  const { run } = selection();
  assert.doesNotThrow(() => validateRun(run));
  for (const change of [
    { path: '.github/workflows/benchmark-smoke.yml' },
    { event: 'pull_request' },
    { conclusion: 'failure' },
  ]) {
    assert.throws(() => validateRun({ ...run, ...change }));
  }
});

test('publishing rejects stale, smoke, dirty, unrelated, rerun, or incomplete results', () => {
  const { request, result, pr, run } = selection();
  assert.doesNotThrow(() => validateSelection(request, result, pr, run));
  const invalidManifests = [
    { head_sha: 'old' },
    { base_sha: 'other' },
    { profile: 'smoke' },
    { dirty: true },
    { run_id: '456' },
    { run_attempt: '2' },
    { suite_sha256: 'invalid' },
  ];
  for (const change of invalidManifests) {
    const changed = { ...result, manifest: { ...result.manifest, ...change } };
    assert.throws(() => validateSelection(request, changed, pr, run));
  }
  for (const change of [{ base_status: 'unavailable' }, { candidate: {} }]) {
    assert.throws(() => validateSelection(request, { ...result, ...change }, pr, run));
  }
  for (const change of [{ pr: 6 }, { action: 'commit' }, { run_attempt: '2' }]) {
    assert.throws(() => validateSelection({ ...request, ...change }, result, pr, run));
  }
});

test('run requests require write access and same-repository PRs', async () => {
  const options = { context: context('@github-actions run benchmark') };
  await assert.rejects(request({ ...options, github: github('read') }), /Write access/);
  await assert.rejects(
    request({ ...options, github: github('write', true) }),
    /in this repository/,
  );
});

test('even maintainers cannot publish for another author', async () => {
  await assert.rejects(commit({
    github: github('admin'),
    context: context('@github-actions commit benchmark 123', 'maintainer'),
  }), /Only the PR author/);
});

test('artifact selection rejects missing, ambiguous, expired, and oversized data', () => {
  const valid = { name: 'results', size_in_bytes: 100 };
  assert.equal(selectArtifact([valid], 'results'), valid);
  for (const artifacts of [
    [],
    [valid, valid],
    [{ ...valid, expired: true }],
    [{ ...valid, size_in_bytes: 10_000_001 }],
  ]) {
    assert.throws(() => selectArtifact(artifacts, 'results'));
  }
});

test('comments suppress mentions, bound report size, and mark stale results', () => {
  const { run } = selection();
  const body = commentBody(run, true, '@someone ' + 'x'.repeat(60_000));
  assert.ok(body.includes('@\u200bsomeone'));
  assert.ok(!body.includes('@someone'));
  assert.ok(body.includes('older revision'));
  assert.ok(body.length < 51_000);
  assert.ok(body.includes('Raw logs are not uploaded'));
  const failure = commentBody({ ...run, conclusion: 'failure' }, false);
  assert.ok(failure.includes('infrastructure failures'));
  assert.ok(!failure.includes('older revision'));
});

test('comments skip runs without an authorized request or associated PR', async () => {
  const { run } = selection();
  const api = { paginate: async () => [], rest: { actions: {} } };
  const notices = [];
  const core = { notice: message => notices.push(message) };
  assert.equal(await commentTarget(api, {}, run, core), null);
  assert.equal(await commentTarget(api, {}, {
    path: '.github/workflows/benchmark-smoke.yml', event: 'pull_request',
  }, core), null);
  assert.equal(notices.length, 2);
});

test('publication writes only report files and refuses a branch that advances', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'vihaco-ci-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true }));
  const fixture = selection();
  const api = github();
  attachArtifacts(api, directory, fixture);

  let advance = false;
  api.rest.git = publicationGit(async ({ force, ref }) => {
    assert.equal(force, false);
    assert.equal(ref, 'heads/feature');
    if (advance) throw new Error('not a fast forward');
  });
  const options = {
    github: api,
    context: context('@github-actions commit benchmark 123'),
    core: { notice() {} },
  };
  await commit(options);
  advance = true;
  await assert.rejects(commit(options), /not a fast forward/);
});

test('successful command comments use request provenance and regenerated reports', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'vihaco-ci-test-'));
  t.after(() => fs.rmSync(directory, { recursive: true }));
  const fixture = selection();
  const api = github();
  attachArtifacts(api, directory, fixture);
  const comments = [];
  api.rest.issues = { createComment: async value => comments.push(value) };
  await postComment(api, context('').repo, fixture.run, { notice() {} });
  assert.equal(comments.length, 1);
  assert.equal(comments[0].issue_number, 5);
  assert.ok(comments[0].body.includes('# vihaco benchmark results'));
  assert.ok(!comments[0].body.includes('# Results'));
  assert.ok(!comments[0].body.includes('older revision'));
});

test('public schema rejects private metadata, old schemas, and unbounded text', () => {
  const original = selection().result;
  assert.doesNotThrow(() => publicResults(JSON.stringify(original)));
  const mutations = [
    result => { result.manifest.schema_version = 2; },
    result => { result.manifest.environment.rustflags = '/private/SECRET_SENTINEL'; },
    result => { result.manifest.environment.rust = '1.90.0 /private/SECRET_SENTINEL'; },
    result => { result.manifest.environment.cpuinfo = 'SECRET_SENTINEL'; },
    result => { result.manifest.reference_policy = 'SECRET_SENTINEL'; },
    result => { result.extra = 'SECRET_SENTINEL'; },
    result => { result.candidate['workload/short/sst/program'].uncertainty = 'SECRET_SENTINEL'; },
  ];
  for (const mutate of mutations) {
    const changed = structuredClone(original);
    mutate(changed);
    assert.throws(() => publicResults(JSON.stringify(changed)), error => {
      assert.ok(!error.message.includes('SECRET_SENTINEL'));
      return true;
    });
  }
  assert.throws(() => publicResults('{"SECRET_SENTINEL'), error => {
    assert.ok(!error.message.includes('SECRET_SENTINEL'));
    return true;
  });
});

test('comments reject mismatched result provenance for command and smoke runs', async t => {
  for (const smoke of [false, true]) {
    for (const field of ['run_id', 'run_attempt', 'head_sha', 'profile', ...(smoke ? [] : ['base_sha'])]) {
      const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'vihaco-ci-test-'));
      t.after(() => fs.rmSync(directory, { recursive: true }));
      const fixture = selection();
      if (smoke) {
        fixture.run.path = '.github/workflows/benchmark-smoke.yml';
        fixture.run.event = 'pull_request';
        fixture.run.pull_requests = [fixture.pr];
        fixture.result.manifest.profile = 'smoke';
      }
      fixture.result.manifest[field] = {
        run_id: '999', run_attempt: '2', head_sha: 'f'.repeat(40),
        base_sha: 'e'.repeat(40), profile: smoke ? 'full' : 'smoke',
      }[field];
      const api = github();
      attachArtifacts(api, directory, fixture);
      let comments = 0;
      api.rest.issues = { createComment: async () => { comments++; } };
      await assert.rejects(postComment(api, context('').repo, fixture.run, { notice() {} }), /provenance/);
      assert.equal(comments, 0);
    }
  }
});

test('comments reject request artifacts from another run or attempt', async t => {
  for (const field of ['run_id', 'run_attempt']) {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'vihaco-ci-test-'));
    t.after(() => fs.rmSync(directory, { recursive: true }));
    const fixture = selection();
    fixture.request[field] = '999';
    const api = github();
    attachArtifacts(api, directory, fixture);
    await assert.rejects(commentTarget(api, context('').repo, fixture.run, { notice() {} }), /provenance/);
  }
});

test('genuine older results remain publishable with a stale warning', async t => {
  for (const smoke of [false, true]) {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'vihaco-ci-test-'));
    t.after(() => fs.rmSync(directory, { recursive: true }));
    const fixture = selection();
    if (smoke) {
      fixture.run.path = '.github/workflows/benchmark-smoke.yml';
      fixture.run.event = 'pull_request';
      fixture.run.pull_requests = [fixture.pr];
      fixture.result.manifest.profile = 'smoke';
    }
    const api = github();
    api.rest.pulls.get = async () => ({ data: { ...fixture.pr, head: { sha: 'e'.repeat(40) } } });
    attachArtifacts(api, directory, fixture);
    const comments = [];
    api.rest.issues = { createComment: async value => comments.push(value) };
    await postComment(api, context('').repo, fixture.run, { notice() {} });
    assert.equal(comments.length, 1);
    assert.ok(comments[0].body.includes('older revision'));
  }
});


test('machine provenance accepts only bounded hashes and revisions', () => {
  const { result } = selection();
  Object.assign(result.manifest, {
    schema_version: 4,
    machine_sha256: 'b'.repeat(64),
    base_machine_sha256: 'c'.repeat(64),
    base_machine_sha: 'd'.repeat(40),
    lock_sha256: 'e'.repeat(64),
    base_lock_sha256: 'f'.repeat(64),
  });
  assert.doesNotThrow(() => publicResults(JSON.stringify(result)));
  for (const field of ['machine_sha256', 'base_machine_sha256', 'base_machine_sha',
    'lock_sha256', 'base_lock_sha256']) {
    const changed = structuredClone(result);
    changed.manifest[field] = '/private/machine/path';
    assert.throws(() => publicResults(JSON.stringify(changed)));
  }
});
