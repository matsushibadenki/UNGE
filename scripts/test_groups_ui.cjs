const fs=require('node:fs'),vm=require('node:vm'),assert=require('node:assert/strict');
const source=fs.readFileSync(require('node:path').join(__dirname,'../examples/tauri-host/ui/groups.js'),'utf8');
async function test(locale){
 const elements=new Map(),calls=[];let pending=Promise.resolve(),revision=3,error;
 const get=id=>{if(!elements.has(id))elements.set(id,{value:'',disabled:false,textContent:'',children:[],replaceChildren(){this.children=[];this.value='';},append(o){this.children.push(o);if(!this.value)this.value=o.value;},reportValidity(){return true;}});return elements.get(id);};
 const send=async request=>{calls.push(request);if(request.kind==='group'&&request.expected_revision!==revision)throw{code:'revision_conflict'};if(request.kind==='group'&&request.action.kind!=='select_members')revision++;return{revision};};
 const invoke=async(_name,args)=>({revision,total:51,selected_nodes:2,groups:[{id:args.after?'last':'first',label:'<script>组</script>',members:2}],next:args.after?null:'first'});
 const enqueue=fn=>{pending=pending.then(fn).catch(e=>{error=e;});};
 const context={crypto:{randomUUID:()=> 'new'},document:{getElementById:get,createElement:()=>({value:'',textContent:''})}};
 vm.createContext(context);vm.runInContext(source,context);
 const ui=context.createGroupControls({invoke,send,enqueue,locale:()=>locale});await ui.refresh();
 assert.ok(get('groupChoice').children[0].textContent.includes('<script>组</script>'));
 get('groupName').value='入力 组';ui.translate();assert.equal(get('groupName').value,'入力 组');
 get('renameGroup').onclick();get('groupName').value='changed after click';await pending;
 const rename=calls.find(c=>c.action?.kind==='rename');assert.equal(rename.action.label,'入力 组');assert.equal(rename.expected_revision,3);
 get('selectGroup').onclick();await pending;assert.equal(calls.find(c=>c.action?.kind==='select_members').action.id,'first');
 for(const id of ['addGroupMembers','removeGroupMembers','createGroup']){get(id).onclick();await pending;}
 assert.equal(calls.find(c=>c.action?.kind==='create').action.id,'new');
 revision++;get('deleteGroup').onclick();await pending;assert.equal(error.code,'revision_conflict');assert.equal(calls.filter(c=>c.action?.kind==='delete').length,1);
 await ui.refresh();get('nextGroups').onclick();await pending;assert.equal(get('groupChoice').value,'last');assert.equal(get('nextGroups').disabled,true);
 get('firstGroups').onclick();await pending;assert.equal(get('groupChoice').value,'first');assert.ok(get('groupNameLabel').textContent);
}
(async()=>{for(const locale of ['en','ja','zh-CN'])await test(locale);console.log('Group UI: 3 language flows passed');})().catch(e=>{console.error(e);process.exitCode=1;});
