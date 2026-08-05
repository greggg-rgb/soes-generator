// Generates tests/fixtures/{default,foe,cia402}.json from the JS reference
// generator's own defaults/spec data, so the Rust model round-trips against
// exactly what the browser tool itself produces.
//
// Run from the repo root:
//   cd .cache/EEPROM_generator && NODE_PATH="$(pwd)/node_modules" node ../../scripts/gen_fixtures.js
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const JS_ROOT = path.resolve(__dirname, '..', '.cache', 'EEPROM_generator');
const OUT_DIR = path.resolve(__dirname, '..', 'tests', 'fixtures');

// constants.js is a plain browser script (no require/window/document), so it
// can be eval'd directly in a fresh sandbox to get at getFormDefaultValues().
// We do NOT eval cia402exampleProjectSpecs.js as a whole: its top-level
// describe(...) runs Jasmine setup on load and throws outside a Jasmine env.
const constantsSrc = fs.readFileSync(path.join(JS_ROOT, 'src', 'constants.js'), 'utf8');
const sandbox = {};
vm.createContext(sandbox);
vm.runInContext(constantsSrc + '\nthis.getFormDefaultValues = getFormDefaultValues;', sandbox);

const defaultForm = sandbox.getFormDefaultValues().form; // ESC: SupportedESC.ET1100 resolves to "ET1100" here

fs.mkdirSync(OUT_DIR, { recursive: true });

fs.writeFileSync(
  path.join(OUT_DIR, 'default.json'),
  JSON.stringify({ form: defaultForm, od: { sdo: {}, txpdo: {}, rxpdo: {} }, dc: [] }, null, 2) + '\n'
);

const foeForm = { ...defaultForm, DetailsEnableUseFoE: true };
fs.writeFileSync(
  path.join(OUT_DIR, 'foe.json'),
  JSON.stringify({ form: foeForm, od: { sdo: {}, txpdo: {}, rxpdo: {} }, dc: [] }, null, 2) + '\n'
);

// cia_esi_json is the spec file's first statement: a backtick template
// literal that is already valid JSON. Extract it verbatim without eval'ing
// the rest of the file (its describe(...) block throws outside Jasmine).
const specPath = path.join(JS_ROOT, 'spec', 'generators', 'cia402exampleProjectSpecs.js');
const specSrc = fs.readFileSync(specPath, 'utf8');
const m = specSrc.match(/^const cia_esi_json = `([\s\S]*?)`;/m);
if (!m) {
  throw new Error('could not find cia_esi_json backtick literal in ' + specPath);
}
const cia402Json = m[1];
JSON.parse(cia402Json); // sanity check: must already be valid JSON
fs.writeFileSync(path.join(OUT_DIR, 'cia402.json'), cia402Json.trimStart() + '\n');

console.log('wrote', path.join(OUT_DIR, 'default.json'));
console.log('wrote', path.join(OUT_DIR, 'foe.json'));
console.log('wrote', path.join(OUT_DIR, 'cia402.json'));
