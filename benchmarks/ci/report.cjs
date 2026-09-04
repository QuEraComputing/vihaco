// SPDX-FileCopyrightText: 2026 The vihaco Authors
// SPDX-License-Identifier: MIT

// Only validated results reach this renderer. Never forward artifact Markdown.
function interval(row) {
  return `${row.mean_ns.toFixed(2)} (${row.lower_ns.toFixed(2)}–${row.upper_ns.toFixed(2)})`;
}

function renderReport(result) {
  const { manifest, base, candidate, references } = result;
  const lines = [
    '# vihaco benchmark results', '',
    `Profile: **${manifest.profile}** · Run: \`${manifest.run_id}\``, '',
    `Base: \`${manifest.base_sha}\` · Candidate: \`${manifest.head_sha}\``,
    `Suite SHA-256: \`${manifest.suite_sha256}\``, '',
    'Times are in nanoseconds per invocation. A negative change means the candidate ran faster.',
    'The report marks overlapping intervals as inconclusive. Performance signals do not affect whether CI passes.',
    'The intervals do not account for hardware drift, and the comparison does not correct for multiple comparisons.', '',
  ];
  if (result.base_status !== 'available') lines.push('The base comparison is unavailable.', '');
  lines.push(
    '| Workload / case / implementation / level | Base ns | Candidate ns (95% interval) | Change | Signal |',
    '|---|---:|---:|---:|---|',
  );
  for (const name of Object.keys(candidate).sort()) {
    const row = candidate[name], previous = base[name];
    let change = '—', signal = 'unavailable';
    if (previous) {
      const delta = 100 * (row.mean_ns / previous.mean_ns - 1);
      change = `${delta >= 0 ? '+' : ''}${delta.toFixed(1)}%`;
      signal = row.lower_ns > previous.upper_ns ? 'possible slowdown'
        : row.upper_ns < previous.lower_ns ? 'possible improvement' : 'inconclusive';
    }
    lines.push(`| ${name} | ${previous ? previous.mean_ns.toFixed(2) : '—'} | ${interval(row)} | ${change} | ${signal} |`);
  }
  lines.push('', '## Shared references', '',
    'The runner measures native Rust and Python once per comparison, using the candidate build for native Rust.', '',
    '| Workload / case / implementation / level | Time ns (95% interval) |', '|---|---:|');
  for (const name of Object.keys(references).sort()) {
    lines.push(`| ${name} | ${interval(references[name])} |`);
  }
  lines.push('', '## Cross-language execution', '',
    'These ratios compare mean execution times against the same native Rust measurement.', '',
    '| Workload / case | Base SST / Rust | Candidate SST / Rust | Python / Rust |',
    '|---|---:|---:|---:|');
  for (const name of Object.keys(references).sort().filter(key => key.endsWith('/rust/program'))) {
    const prefix = name.slice(0, -'/rust/program'.length);
    const native = references[name].mean_ns;
    const ratio = row => row ? (row.mean_ns / native).toFixed(2) + '×' : '—';
    lines.push(`| ${prefix} | ${ratio(base[prefix + '/sst/program'])} | ${ratio(candidate[prefix + '/sst/program'])} | ${ratio(references[prefix + '/python/program'])} |`);
  }
  return lines.join('\n') + '\n';
}

module.exports = { renderReport };
