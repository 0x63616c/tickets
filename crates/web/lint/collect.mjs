// Machine interface for xtask: both normal and staged contents arrive on stdin.
// Stylelint parses CSS; the Rust scanner supplies literal inline declarations.
import stylelint from 'stylelint';
import config from '../stylelint.config.mjs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { invalidValue } from './value-policy.mjs';

let input = '';
for await (const chunk of process.stdin) input += chunk;
const { files, inline } = JSON.parse(input);
const findings = [];
const configBasedir = fileURLToPath(new URL('..', import.meta.url));
for (const [path, code] of Object.entries(files)) {
  if (path === 'crates/web/style/tokens.css') continue;
  const { results } = await stylelint.lint({
    code, codeFilename: resolve(path), config, configBasedir, ignoreDisables: true,
  });
  for (const result of results) {
    for (const warning of result.warnings) {
      if (warning.rule !== 'tickets/design-tokens') throw new Error(`${path}:${warning.line}: ${warning.text}`);
      const declaration = warning.text.replace(/^Use a design token: /, '').replace(/ \(tickets\/design-tokens\)$/, '');
      findings.push({ key: `${path}:css:${declaration}`, line: warning.line });
    }
    if (result.parseErrors?.length) throw new Error(JSON.stringify(result.parseErrors));
  }
}
for (const { path, property, value, line } of inline) {
  if (invalidValue(property, value)) findings.push({ key: `${path}:rust-inline:${property}:${value}`, line });
}
process.stdout.write(JSON.stringify(findings));
