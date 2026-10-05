// State-machine checks; rendered native WebView QA is documented separately.
const fs = require('node:fs'), vm = require('node:vm'), assert = require('node:assert/strict');
const source = fs.readFileSync(require('node:path').join(__dirname,'../examples/tauri-host/ui/execution.js'),'utf8');
const tick = () => new Promise(resolve => setImmediate(resolve));
async function test(locale) {
  const elements = new Map(); const listeners = {}; let poll; let active = null; let error = null;
  const get = id => { if (!elements.has(id)) elements.set(id,{textContent:'',disabled:false,setAttribute(){}}); return elements.get(id); };
  const record = (sequence,state,extra={}) => ({id:'a',revision:0,sequence,state,total:3,finished:0,failed:0,blocked:0,cancel_requested:false,reason:null,...extra});
  let latest = record(0,'queued');
  const invoke = async (command) => {
    if (command === 'current_execution') return active;
    if (command === 'start_execution') {if (error) throw {code:error}; active=latest;return latest;}
    if (command === 'execution_status') return latest;
    if (command === 'cancel_execution') { latest=record(3,'running',{cancel_requested:true}); return latest; }
  };
  const context = {document:{getElementById:get},window:{addEventListener(){}},console:{error(){}},setInterval(fn){poll=fn;return 1;},clearInterval(){}};
  vm.createContext(context);vm.runInContext(source,context);
  context.createExecutionUI({invoke,listen:async (name,fn)=>{listeners[name]=fn;},locale:()=>locale,revision:()=>0});
  await tick();
  assert.equal(get('run').disabled,false);
  await get('run').onclick();assert.equal(get('run').disabled,true);
  listeners['unge://execution']({payload:record(2,'running',{finished:1})});
  listeners['unge://execution']({payload:record(1,'running')});assert.equal(get('runProgress').value,1);
  await get('cancelRun').onclick();assert.equal(get('cancelRun').disabled,true);
  // Recover terminal state without receiving a finished notification.
  active=null;latest=record(4,'finished',{finished:3,reason:'cancelled'});await poll();
  assert.equal(get('runProgress').value,3);assert.equal(get('run').disabled,false);
  assert.match(get('runStatus').textContent,/Cancelled|キャンセル済み|已取消/);
  error='invalid_graph';await get('run').onclick();
  assert.match(get('runStatus').textContent,/required|必須|必填/);
}
(async()=>{for(const locale of ['en','ja','zh-CN']) await test(locale);console.log('Execution UI: 3 language flows passed');})().catch(e=>{console.error(e);process.exitCode=1;});
