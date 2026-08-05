const fs = require('fs'), path = require('path');
const { JSDOM } = require('jsdom');
const ROOT = '/home/k1ase/App/soes_generator/.cache/EEPROM_generator';
const srcFiles = ['src/constants.js','src/validation.js','src/od.js','src/file_io.js','src/backup.js','src/binaries.js','src/readers/xml_reader.js','src/generators/EEPROM.js','src/generators/esi_xml.js','src/generators/ecat_options.js','src/generators/objectlist.js','src/generators/utypes.js','spec/helpers/formMockHelper.js'];
let scripts='';
for(const f of srcFiles){ scripts+=`<script>\n${fs.readFileSync(path.join(ROOT,f),'utf8')}\n</script>\n`; }
// probe script
const probe = `<script>
window.__out = {};
window.alert = function(m){ (window.__alerts=window.__alerts||[]).push(String(m)); };

//=== BUG #3: REAL32 default value in objectlist ===
try {
  var form = buildMockFormHelper();
  var od = { '2000': { otype:'VAR', dtype:'REAL32', name:'MyFloat', value:1.5 } };
  var res = objectlist_generator(form, od, ['2000']);
  var line = res.split('\\n').find(function(l){return l.indexOf('DTYPE_REAL32')>=0;});
  window.__out.real32_line = line;
} catch(e){ window.__out.real32_err = ''+e; }

//=== BUG #4: Port3 dropped from ESI Physics ===
try {
  var f2 = buildMockFormHelper();
  f2.Port0Physical.value='Y'; f2.Port1Physical.value='Y'; f2.Port2Physical.value='Y'; f2.Port3Physical.value='H';
  var odSections = getEmptyObjDict();
  var od2 = buildObjectDictionary(f2, odSections);
  var idx = getUsedIndexes(od2);
  var esi = esi_generator(f2, od2, idx, []);
  var m = esi.match(/<Device Physics="([^"]*)"/);
  window.__out.physics = m ? m[1] : null;
  // EEPROM getPhysicalPort for same ports (via hex generator config? just note):
} catch(e){ window.__out.physics_err = ''+e; }

//=== BUG #2: reader field-name mismatch for UploadAtStartup ===
//=== BUG #1: reader getElementsByTagGroupType typo when Type empty ===
var xml = '<?xml version="1.0"?><EtherCATInfo><Vendor><Name>V</Name><Id>#x1</Id></Vendor>'
 +'<Groups><Group><Type>G</Type><Name>GN</Name></Group></Groups>'
 +'<Devices><Device><Type ProductCode="#xab" RevisionNo="#x2">DEVTYPE</Type><Name>DN</Name>'
 +'<Profile><ProfileNo>5001</ProfileNo><Dictionary><DataTypes></DataTypes><Objects></Objects></Dictionary></Profile>'
 +'<Mailbox><CoE SdoInfo="true" PdoAssign="true" PdoConfig="false" PdoUpload="true" CompleteAccess="false"/></Mailbox>'
 +'<Eeprom><ByteSize>2048</ByteSize><ConfigData>ABCD</ConfigData></Eeprom></Device></Devices></EtherCATInfo>';
try {
  var r = xml_reader(xml, 'ET1100');
  window.__out.reader_UploadAtStartup_correctKey = r.form.CoeDetailsEnableUploadAtStartup;  // undefined if bug
  window.__out.reader_UploadAtStartup_wrongKey = r.form.CoeDetailsEnablePdoUploadAtStartup;  // true -> proof
} catch(e){ window.__out.reader_err = ''+e; }

// BUG #1 proof: empty <Type> triggers getElementsByTagGroupType
var xml2 = xml.replace('<Type ProductCode="#xab" RevisionNo="#x2">DEVTYPE</Type>','<Type ProductCode="#xab" RevisionNo="#x2"></Type>');
try {
  var r2 = xml_reader(xml2, 'ET1100');
  window.__out.reader_emptyType = 'NO THROW: '+r2.form.TextDeviceType;
} catch(e){ window.__out.reader_emptyType_err = ''+e; }

window.__done=true;
</script>`;
const html = `<!DOCTYPE html><html><body>${scripts}${probe}</body></html>`;
const dom = new JSDOM(html,{runScripts:'dangerously',url:'http://localhost/'});
setTimeout(()=>{
  const o = dom.window.__out||{};
  console.log(JSON.stringify(o,null,2));
  if(dom.window.__alerts) console.log('ALERTS:',JSON.stringify(dom.window.__alerts));
  if(dom.window.__loadErrors) console.log('LOADERR:',dom.window.__loadErrors);
},400);
