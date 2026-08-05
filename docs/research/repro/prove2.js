const fs=require('fs'),path=require('path');const {JSDOM}=require('jsdom');
const ROOT='/home/k1ase/App/soes_generator/.cache/EEPROM_generator';
const F=['src/constants.js','src/validation.js','src/od.js','src/file_io.js','src/backup.js','src/binaries.js','src/readers/xml_reader.js','src/generators/EEPROM.js','src/generators/esi_xml.js','src/generators/ecat_options.js','src/generators/objectlist.js','src/generators/utypes.js','spec/helpers/formMockHelper.js'];
let s='';for(const f of F)s+=`<script>\n${fs.readFileSync(path.join(ROOT,f),'utf8')}\n</script>\n`;
const p=`<script>
window.alert=function(m){(window.__a=window.__a||[]).push(''+m);};
window.__o={};
// Intel hex odd size
try{ var form=buildMockFormHelper(); form.EEPROMsize.value='2000';
 var rec=hex_generator(form); var hx=toIntelHex(rec); window.__o.hex2000='OK len='+hx.length;
}catch(e){window.__o.hex2000_err=''+e;}
// multiple of 32 sanity
try{ var f2=buildMockFormHelper(); f2.EEPROMsize.value='2048';
 var r2=hex_generator(f2); toIntelHex(r2); window.__o.hex2048='OK';
}catch(e){window.__o.hex2048_err=''+e;}
window.__done=true;
</script>`;
const dom=new JSDOM(`<!DOCTYPE html><body>${s}${p}</body>`,{runScripts:'dangerously',url:'http://localhost/'});
setTimeout(()=>{console.log(JSON.stringify(dom.window.__o,null,2));},400);
