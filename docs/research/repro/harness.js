const fs = require('fs');
const path = require('path');
const { JSDOM } = require('jsdom');

const ROOT = '/home/k1ase/App/soes_generator/.cache/EEPROM_generator';
const files = [
  'lib/jasmine-3.8.0/jasmine.js',
  'lib/jasmine-3.8.0/jasmine-html.js',
  'lib/jasmine-3.8.0/boot.js',
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
  'spec/helpers/customMatchers.js',
  'spec/helpers/formMockHelper.js',
  'spec/backupSpecs.js',
  'spec/binariesSpecs.js',
  'spec/odSpecs.js',
  'spec/readers/xml_reader_cia402exampleSpecs.js',
  'spec/generators/emptyProjectSpecs.js',
  'spec/generators/cia402exampleProjectSpecs.js',
  'spec/generators/enabledFoEProjectSpecs.js',
  'spec/generators/escSpecificSpecs.js',
  'spec/generators/spiModeHexSpecs.js',
  'spec/generators/VAR/ARRAY_Specs.js',
  'spec/generators/VAR/INTEGER8_Specs.js',
  'spec/generators/VAR/INTEGER64_Specs.js',
  'spec/generators/VAR/VISIBLE_STRING_Specs.js',
];

let scripts = '';
for (const f of files) {
  const p = path.join(ROOT, f);
  const code = fs.readFileSync(p, 'utf8');
  scripts += `<script>\n//# FILE ${f}\n${code}\n</script>\n`;
}

// reporter injected after boot but before onload; boot calls env.execute on onload
const reporterScript = `<script>
(function(){
  var env = jasmine.getEnv();
  window.__results = { specs: [], failures: [] };
  env.addReporter({
    specDone: function(r){
      window.__results.specs.push({name:r.fullName, status:r.status});
      if(r.status==='failed'){
        window.__results.failures.push({name:r.fullName, messages:r.failedExpectations.map(function(f){return f.message;})});
      }
    },
    jasmineDone: function(r){ window.__done = true; window.__overall = r.overallStatus; }
  });
})();
</script>`;

const html = `<!DOCTYPE html><html><head></head><body>${scripts}${reporterScript}</body></html>`;

const dom = new JSDOM(html, { runScripts: 'dangerously', pretendToBeVisual: true, url: 'http://localhost/' });
const w = dom.window;
w.alert = function(m){ (w.__alerts=w.__alerts||[]).push(String(m)); };

function poll(tries){
  if (w.__done) return report();
  if (tries<=0) { console.log('TIMEOUT waiting for jasmineDone'); return report(); }
  setTimeout(()=>poll(tries-1), 100);
}
function report(){
  if (w.__loadErrors) { console.log('LOAD ERRORS:'); w.__loadErrors.forEach(e=>console.log('  '+e)); }
  const res = w.__results || {specs:[],failures:[]};
  console.log('TOTAL SPECS:', res.specs.length, 'OVERALL:', w.__overall);
  const failed = res.failures;
  console.log('FAILURES:', failed.length);
  failed.forEach(f=>{ console.log('--- FAIL:', f.name); f.messages.forEach(m=>console.log('    '+m.split('\n').slice(0,6).join('\n    '))); });
  if (w.__alerts) console.log('ALERTS:', w.__alerts.length, JSON.stringify(w.__alerts.slice(0,10)));
  process.exit(0);
}
// give scripts time to parse then rely on onload; jsdom fires load automatically
setTimeout(()=>poll(100), 200);
