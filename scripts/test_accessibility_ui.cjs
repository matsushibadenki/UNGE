const fs = require('node:fs'), vm = require('node:vm'), assert = require('node:assert/strict');
const source = fs.readFileSync(require('node:path').join(__dirname,'../examples/tauri-host/ui/accessibility.js'),'utf8');
async function test(locale) {
  const elements = new Map(), calls = []; let revision = 7, pending = Promise.resolve();
  function get(id) {
    if (!elements.has(id)) elements.set(id, { value:'', disabled:false, textContent:'', children:[], replaceChildren(){this.children=[];this.value='';}, append(o){this.children.push(o);if(!this.value)this.value=o.value;}, reportValidity(){return true;} });
    return elements.get(id);
  }
  const node = { id:'node-a',title:'<script>組</script>',rect:{x:1,y:2,width:180,height:90},selected:false };
  const invoke = async (name,args) => { calls.push([name,args]); assert.equal(args.expectedRevision,revision);return {revision,total:51,nodes:args.after ? [{...node,id:'node-b'}] : [node],next:args.after ? null : 'node-a'}; };
  const send = async request => {calls.push(['dispatch',request]);if(request.kind==='apply'){if(request.expected_revision!==revision)throw {code:'revision_conflict'}; revision++;}return {revision};};
  let error;
  const enqueue = action => { pending=pending.then(action).catch(e=>{error=e;}); };
  const context={document:{getElementById:get,createElement:()=>({value:'',textContent:''})}};
  vm.createContext(context);vm.runInContext(source,context);
  const ui = context.createAccessibleGraph({invoke,send,enqueue,locale:()=>locale});
  await ui.refresh();
  assert.equal(get('nodeChoice').children[0].textContent.includes(node.title),true);
  assert.equal(get('nodeX').value,'1');
  assert.equal(get('nextNodes').disabled,false);
  get('selectNode').onclick();await pending;assert.deepEqual(calls.find(c=>c[1].kind==='select')[1].ids[0],'node-a');
  get('nodeX').value='300';get('nodeX').oninput();ui.translate();assert.equal(get('nodeX').value,'300');
  get('moveForm').onsubmit({preventDefault(){}});await pending;
  const move=calls.find(c=>c[1].command?.kind==='move_node')[1];
  assert.equal(move.expected_revision,7);assert.equal(move.command.rect.x,300);assert.equal(move.command.rect.width,180);
  // Another view edits after the page was displayed: reject the old intent, no retry.
  revision++;
  get('deleteNode').onclick();await pending;
  assert.equal(error.code,'revision_conflict');assert.equal(calls.filter(c=>c[1].command?.kind==='remove_node').length,1);
  await ui.refresh();get('nextNodes').onclick();await pending;
  assert.equal(get('nodeChoice').value,'node-b');assert.equal(get('nextNodes').disabled,true);
  get('previousNodes').onclick();await pending;assert.equal(get('nodeChoice').value,'node-a');
  assert.ok(get('graphLabel').textContent);assert.ok(get('xLabel').textContent);
}
(async()=>{for(const locale of ['en','ja','zh-CN'])await test(locale);console.log('Accessible graph UI: 3 language flows passed');})().catch(e=>{console.error(e);process.exitCode=1;});
