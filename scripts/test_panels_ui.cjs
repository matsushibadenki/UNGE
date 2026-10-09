const fs = require('node:fs'), vm = require('node:vm'), assert = require('node:assert/strict');
const source = fs.readFileSync(require('node:path').join(__dirname, '../examples/tauri-host/ui/panels.js'), 'utf8');
const tick = () => new Promise(resolve => setImmediate(resolve));
async function test(locale) {
  const elements = new Map(), listeners = {};
  const get = id => { if (!elements.has(id)) elements.set(id, {}); return elements.get(id); };
  let layout = { supported:true, floating:false }, complete, calls = 0, failure = null;
  const context = { document:{ getElementById:get } }; vm.createContext(context); vm.runInContext(source,context);
  const controls = context.createPanelControls({ locale:()=>locale, listen:async (key, fn)=>{listeners[key]=fn;}, invoke:async (key,args)=>{
    if(key==='panel_layout') return layout;
    calls++; await new Promise(resolve=>{complete=resolve;});
    layout={supported:true,floating:args.floating};
    if(failure) throw { code:failure };
    return layout;
  }});
  await tick(); const toggle=get('panelToggle');
  assert.equal(get('panelToolbar').hidden,false); assert.equal(toggle.disabled,false);
  const pending=toggle.onclick(); await toggle.onclick(); assert.equal(calls,1); assert.equal(toggle.disabled,true);
  complete(); await pending; assert.match(toggle.textContent,/Dock|ドッキング|停靠/);
  // Native close returns the same WebView to its dock without another IPC edit.
  layout={supported:true,floating:false};listeners['unge://panel-layout']({payload:layout});
  assert.match(toggle.textContent,/Float|切り離す|浮动/);
  // A focus/resize error after a successful move must refresh actual ownership.
  failure='panel_error';const failed=toggle.onclick();complete();await failed;
  assert.match(toggle.textContent,/Dock|ドッキング|停靠/);assert.ok(get('panelStatus').textContent);
  assert.equal(toggle.disabled,false); controls.translate();
  listeners['unge://panel-layout']({payload:{supported:false,floating:false}});
  assert.equal(get('panelToolbar').hidden,true);assert.equal(toggle.disabled,true);
}
(async()=>{for(const locale of ['en','ja','zh-CN']) await test(locale);console.log('Panel UI: 3 language flows passed');})().catch(e=>{console.error(e);process.exitCode=1;});
