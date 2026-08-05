/**
 * Golden-vector dumper: drives the JS reference generator (headless, via jsdom)
 * with the committed fixtures and writes its raw output into tests/golden/.
 *
 * These files are the oracle every later Rust generator task diffs against
 * byte-for-byte, so this script must call the reference exactly the way its
 * own spec suite does. Loading approach (inline src/*.js + formMockHelper.js
 * into a jsdom window) is lifted from docs/research/repro/harness.js.
 *
 * Run from .cache/EEPROM_generator with jsdom on NODE_PATH:
 *   cd .cache/EEPROM_generator && NODE_PATH="$(pwd)/node_modules" node ../../scripts/dump_golden.js
 */
const fs = require('fs');
const path = require('path');
const { JSDOM } = require('jsdom');

const REF_ROOT = path.resolve(__dirname, '../.cache/EEPROM_generator');
const REPO_ROOT = path.resolve(__dirname, '..');
const GOLDEN_ROOT = path.join(REPO_ROOT, 'tests/golden');
const FIXTURES_ROOT = path.join(REPO_ROOT, 'tests/fixtures');

// Only what's needed to define the generator + helper functions (no jasmine/specs).
const files = [
  'src/constants.js',
  'src/validation.js',
  'src/od.js',
  'src/file_io.js',
  'src/backup.js',
  'src/binaries.js',
  'src/readers/xml_reader.js',
  'src/generators/EEPROM.js',
  'src/generators/esi_xml.js',
  'src/generators/ecat_options.js',
  'src/generators/objectlist.js',
  'src/generators/utypes.js',
  'spec/helpers/formMockHelper.js',
];

let scripts = '';
for (const f of files) {
  const code = fs.readFileSync(path.join(REF_ROOT, f), 'utf8');
  scripts += `<script>\n//# FILE ${f}\n${code}\n</script>\n`;
}

const doneScript = `<script>window.__ready = true;</script>`;
const html = `<!DOCTYPE html><html><head></head><body>${scripts}${doneScript}</body></html>`;
const dom = new JSDOM(html, { runScripts: 'dangerously', pretendToBeVisual: true, url: 'http://localhost/' });
const w = dom.window;

if (!w.__ready) {
  throw new Error('jsdom scripts failed to load');
}

function writeFile(dir, name, data) {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, name), data);
}

function dumpFixture(name) {
  const fixture = JSON.parse(fs.readFileSync(path.join(FIXTURES_ROOT, `${name}.json`), 'utf8'));
  const form = w.buildMockFormHelper(fixture.form);
  const odSections = fixture.od;
  const dc = fixture.dc;
  const od = w.buildObjectDictionary(form, odSections);
  const indexes = w.getUsedIndexes(od);

  const outDir = path.join(GOLDEN_ROOT, name);
  writeFile(outDir, 'objectlist.c', w.objectlist_generator(form, od, indexes));
  writeFile(outDir, 'utypes.h', w.utypes_generator(form, od, indexes));
  writeFile(outDir, 'ecat_options.h', w.ecat_options_generator(form, od, indexes));
  writeFile(outDir, 'device.xml', w.esi_generator(form, od, indexes, dc));

  const bytes = w.hex_generator(form); // Uint8Array, stringOnly=false
  writeFile(outDir, 'eeprom.bin', Buffer.from(bytes));
  writeFile(outDir, 'eeprom.hex', w.toIntelHex(bytes));
  writeFile(outDir, 'eeprom.h', w.toEsiEepromH(bytes));
  writeFile(outDir, 'configdata.txt', w.hex_generator(form, true));

  return form;
}

for (const name of ['default', 'foe', 'cia402']) {
  dumpFixture(name);
}

// ESC-specific configdata dumps: default form, ESC overridden per SupportedESC value.
// Filenames map SupportedESC keys -> lowercase tokens (spaces -> underscore).
const escFilenames = {
  ET1100: 'et1100.txt',
  AX58100: 'ax58100.txt',
  LAN9252: 'lan9252.txt',
  'LAN9253 Beckhoff': 'lan9253_beckhoff.txt',
  'LAN9253 Direct': 'lan9253_direct.txt',
  'LAN9253 Indirect': 'lan9253_indirect.txt',
};

const escDefault = JSON.parse(fs.readFileSync(path.join(FIXTURES_ROOT, 'default.json'), 'utf8'));
for (const [escValue, filename] of Object.entries(escFilenames)) {
  const form = w.buildMockFormHelper(escDefault.form);
  form.ESC.value = escValue;
  const configdata = w.hex_generator(form, true);
  writeFile(path.join(GOLDEN_ROOT, 'esc'), filename, configdata);
}

console.log('Golden vectors written to', GOLDEN_ROOT);
