// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// Publication accepts a closed data schema, never arbitrary metadata or Markdown.
// This limits accidental disclosure; it cannot detect secrets encoded as valid
// workload identifiers or numbers. Measurement jobs must remain secret-free.
const { renderReport } = require('./report.cjs');

function check(condition) {
  if (!condition) throw new Error('Result does not meet the public metadata policy');
}

function fields(value, names) {
  check(value !== null && typeof value === 'object' && !Array.isArray(value));
  check(Object.keys(value).sort().join(',') === [...names].sort().join(','));
}

function text(value, pattern) {
  check(typeof value === 'string' && pattern.test(value));
}

function environment(value) {
  fields(value, ['platform', 'machine', 'python', 'rust', 'cargo', 'cpu_count', 'build_flags_source']);
  check(['Linux', 'Darwin', 'Windows', 'other'].includes(value.platform));
  check(['arm64', 'aarch64', 'x86_64', 'AMD64', 'other'].includes(value.machine));
  for (const key of ['python', 'rust', 'cargo']) text(value[key], /^(unknown|[0-9]+\.[0-9]+\.[0-9]+)$/);
  check(value.cpu_count === null || (Number.isSafeInteger(value.cpu_count) && value.cpu_count > 0));
  check(['unset', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'].includes(value.build_flags_source));
}

function manifest(value) {
  fields(value, ['schema_version', 'run_id', 'reference_policy', 'run_attempt', 'profile',
    'head_sha', 'base_sha', 'suite_sha256', 'workloads', 'created_at', 'environment', 'dirty',
    ...(value.schema_version === 4 ? ['machine_sha256', 'base_machine_sha256',
      'base_machine_sha', 'lock_sha256', 'base_lock_sha256'] : [])]);
  check([3, 4].includes(value.schema_version) && typeof value.dirty === 'boolean');
  if (value.schema_version === 4) {
    for (const key of ['machine_sha256', 'base_machine_sha256', 'lock_sha256', 'base_lock_sha256']) {
      if (value[key] !== null) text(value[key], /^[a-f0-9]{64}$/);
    }
    if (value.base_machine_sha !== null) text(value.base_machine_sha, /^[a-f0-9]{40}$/);
  }
  check(['smoke', 'full'].includes(value.profile));
  text(value.run_id, /^[1-9][0-9]*$/);
  text(value.run_attempt, /^[1-9][0-9]*$/);
  for (const key of ['head_sha', 'base_sha']) text(value[key], /^[a-f0-9]{40}$/);
  text(value.suite_sha256, /^[a-f0-9]{64}$/);
  text(value.created_at, /^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:.]+(?:Z|\+00:00)$/);
  check(value.reference_policy === 'once per comparison; native Rust uses the candidate suite build');
  check(value.workloads && typeof value.workloads === 'object' && !Array.isArray(value.workloads));
  check(Object.keys(value.workloads).length > 0 && Object.keys(value.workloads).length <= 1000);
  for (const [name, digest] of Object.entries(value.workloads)) {
    text(name, /^[a-z0-9_-]{1,80}$/);
    text(digest, /^[a-f0-9]{64}$/);
  }
  environment(value.environment);
}

function measurements(rows, workloads, references) {
  check(rows && typeof rows === 'object' && !Array.isArray(rows));
  check(Object.keys(rows).length <= 10000);
  for (const [name, estimate] of Object.entries(rows)) {
    const parts = name.split('/');
    check(parts.length === 4);
    text(parts[0], /^[a-z0-9_-]{1,80}$/);
    text(parts[1], /^[a-z0-9_-]{1,80}$/);
    const instruction = parts[0] === 'instruction' && parts[1] === 'const';
    check(instruction || Object.hasOwn(workloads, parts[0]));
    if (references) {
      check(!instruction && ['rust', 'python'].includes(parts[2]) && parts[3] === 'program');
    } else {
      check(parts[2] === 'sst' && (instruction
        ? ['cpu-operation', 'cpu', 'composite'].includes(parts[3])
        : ['program', 'cpu-program'].includes(parts[3])));
    }
    fields(estimate, ['mean_ns', 'lower_ns', 'upper_ns', 'samples', 'uncertainty']);
    for (const key of ['mean_ns', 'lower_ns', 'upper_ns']) {
      check(typeof estimate[key] === 'number' && Number.isFinite(estimate[key]) && estimate[key] > 0);
    }
    check(estimate.lower_ns <= estimate.upper_ns);
    check(Number.isSafeInteger(estimate.samples) && estimate.samples > 0);
    check(['95% Criterion bootstrap mean interval', '95% bootstrap interval of process means']
      .includes(estimate.uncertainty));
  }
}

function publicResults(raw) {
  let result;
  try {
    result = JSON.parse(raw);
  } catch {
    throw new Error('Invalid public result JSON');
  }
  fields(result, ['manifest', 'base_status', 'base', 'candidate', 'references']);
  manifest(result.manifest);
  check(['available', 'unavailable'].includes(result.base_status));
  for (const group of ['base', 'candidate', 'references']) {
    measurements(result[group], result.manifest.workloads, group === 'references');
  }
  check(Object.keys(result.candidate).length > 0 && Object.keys(result.references).length > 0);
  check(result.base_status === 'available'
    ? Object.keys(result.base).sort().join() === Object.keys(result.candidate).sort().join()
    : Object.keys(result.base).length === 0);
  for (const name of Object.keys(result.references)) {
    const prefix = name.split('/').slice(0, 2).join('/');
    check(result.references[prefix + '/rust/program'] && result.references[prefix + '/python/program']);
    check(result.candidate[prefix + '/sst/program'] && result.candidate[prefix + '/sst/cpu-program']);
  }
  return {
    result,
    files: {
      'results.json': JSON.stringify(result, null, 2) + '\n',
      'report.md': renderReport(result),
    },
  };
}

module.exports = { publicResults };
