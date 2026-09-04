// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const MAX_ARCHIVE_BYTES = 10_000_000;
const MAX_FILE_BYTES = 2_000_000;

async function listArtifacts(github, repo, runId) {
  return github.paginate(github.rest.actions.listWorkflowRunArtifacts, {
    ...repo,
    run_id: runId,
  });
}

function selectArtifact(artifacts, name) {
  const matches = artifacts.filter(item => item.name === name && !item.expired);
  if (matches.length !== 1 || matches[0].size_in_bytes > MAX_ARCHIVE_BYTES) {
    throw new Error(`Missing, ambiguous, expired, or oversized artifact: ${name}`);
  }
  return matches[0];
}

function readArchive(data, filenames) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'vihaco-artifact-'));
  try {
    const archive = path.join(directory, 'artifact.zip');
    fs.writeFileSync(archive, Buffer.from(data));
    // Callers supply fixed filenames. Read to stdout, never extract untrusted
    // paths into the checkout or execute anything from the archive.
    return Object.fromEntries(filenames.map(filename => [
      filename,
      execFileSync('unzip', ['-p', archive, filename], {
        encoding: 'utf8',
        maxBuffer: MAX_FILE_BYTES,
      }),
    ]));
  } finally {
    fs.rmSync(directory, { recursive: true });
  }
}

async function artifact(github, repo, runId, name, filenames) {
  const selected = selectArtifact(await listArtifacts(github, repo, runId), name);
  const response = await github.rest.actions.downloadArtifact({
    ...repo,
    artifact_id: selected.id,
    archive_format: 'zip',
  });
  return readArchive(response.data, filenames);
}

module.exports = { artifact, listArtifacts, selectArtifact };
