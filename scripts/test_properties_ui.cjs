/* Browser/IPC-mock regression. Supply externally installed Playwright via NODE_PATH;
   optional PLAYWRIGHT_EXECUTABLE_PATH and UNGE_SCREENSHOTS. No dependencies are installed. */
const fs = require('node:fs'), path = require('node:path'), assert = require('node:assert/strict');
const { chromium } = require('playwright');
const root = path.resolve(__dirname,'../examples/tauri-host/ui');
(async () => {
 const browser = await chromium.launch({headless:true, executablePath:process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined});
 try {
  for (const locale of ['en','ja','zh-CN']) {
   const page = await browser.newPage({viewport:{width:380,height:900},locale});
   const errors = []; page.on('pageerror', e=>errors.push(e.message)); page.on('console', m=>{if(m.type()==='error') errors.push(m.text());});
   await page.route('http://unge.test/**', route => {
    const file = path.join(root,new URL(route.request().url()).pathname.replace(/^\//,'') || 'index.html');
    route.fulfill({body:fs.readFileSync(file),contentType:file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html'});
   });
   await page.addInitScript(() => {
    const text = (en,ja,zh_cn)=>({en,ja,zh_cn});
    const field = (name,value_type,defaultValue,required=true)=>({name,description:text('Stored in the shared document','共有ドキュメントに保存します','保存在共享文档中'),value_type,default:defaultValue,required});
    const data = {id:'node-1',revision:0,type_id:'example.adjust',version:'1',name:text('Adjust value','数値を調整','调整数值'),description:text('Scale, offset and round','倍率・加算値・丸めを設定','设置倍率、偏移和取整'),schema:{additional_properties:false,fields:{
     enabled:field(text('Enabled','処理を有効にする','启用处理'),{kind:'bool'},true),
     gain:field(text('Gain','倍率','倍率'),{kind:'float',minimum:0,maximum:4},1),
     note:field(text('Note','メモ','备注'),{kind:'string',min_length:0,max_length:240,choices:null},'',false),
     offset:field(text('Offset','加算値','偏移'),{kind:'int',minimum:-100,maximum:100},0),
     rounding:field(text('Rounding','丸め処理','取整'),{kind:'string',min_length:1,max_length:5,choices:['none','round','floor']},'none'),
    }},properties:{enabled:true,gain:1,note:'',offset:0,rounding:'none'}};
    const second = structuredClone(data); second.id='node-2'; second.properties.gain=0.5;
    const callbacks = {}; let revision = 0, theme='dark', selection=['node-1']; window.applied=[]; window.propertyRequests=[];
    window.selectNodes=(ids,notify=true)=>{selection=ids; if(notify) callbacks['unge://selection-changed']?.forEach(callback=>callback({payload:{revision,count:ids.length,single:ids.length===1?ids[0]:null}}));};
    window.releaseProperties=()=>{};
    const summary=()=>({revision,nodes:1,edges:0});
    window.externalEdit=()=>{revision++;callbacks['unge://changed']?.forEach(callback=>callback({payload:summary()}));};
    window.addJson=()=>{data.schema.fields.json=field(text('Metadata','メタデータ','元数据'),{kind:'json'},{safe:true},false);data.properties.json={safe:true};};
    window.__TAURI__={event:{listen:async(name,callback)=>{(callbacks[name] ||= []).push(callback);return()=>{};}},core:{invoke:async(name,args)=>{
     if(name==='current_execution')return null;
     if(name==='panel_layout')return{supported:false,floating:false};
     if(name==='selection_summary'){if(window.holdSelection){window.holdSelection=false;window.selectionHeld=true;await new Promise(resolve=>window.releaseSelection=resolve);window.selectionHeld=false;}if(window.rejectSelection)throw{code:'state_unavailable'};return{revision,count:selection.length,single:selection.length===1?selection[0]:null};}
     if(name==='appearance')return{theme,colors:theme==='dark'?{}:{background:'#f5f7fb',surface:'#ffffff',border:'#68768b',accent:'#285dd5',text:'#172235',muted:'#4e617d',hover:'#e9eef8'}};
     if(name==='accessible_nodes')return{revision,total:1,nodes:[{id:data.id,title:'Adjust / 調整 / 调整',rect:{x:0,y:0,width:224,height:128},selected:true}],next:null};
     if(name==='groups')return{revision,total:0,selected_nodes:1,groups:[],next:null};
     if(name==='node_properties'){
      window.propertyRequests.push(args.id);
      if(window.rejectLoad)throw{code:'missing_node'};
      const result=structuredClone({...args.id==='node-2'?second:data,revision});
      if(window.holdProperty===args.id){window.holdProperty=null;return new Promise(resolve=>{window.releaseProperties=()=>resolve(result);});}
      return result;
     }
     if(name==='dispatch'){
      const request=args.request;
      if(request.kind==='select') window.selectNodes(request.ids);
      if(request.kind==='set_theme')theme=request.theme;
      if(request.kind==='apply'){
       if(window.holdSave){window.holdSave=false;window.saveHeld=true;await new Promise(resolve=>window.releaseSave=resolve);window.saveHeld=false;}
       if(window.rejectSave)throw{code:window.rejectSave};
       if(request.expected_revision!==revision)throw{code:'revision_conflict'};
       window.applied.push(structuredClone(request));
       request.command.commands.forEach(c=>{const target=c.id==='node-2'?second:data;if(c.value===null)delete target.properties[c.key];else target.properties[c.key]=c.value;});
       revision++; callbacks['unge://changed']?.forEach(callback=>callback({payload:summary()}));
      }
      return summary();
     }
     throw Error(`Unexpected command ${name}`);
    }}};
   });
   await page.goto('http://unge.test/');
   assert.equal(page.url(),'http://unge.test/'); assert.equal(await page.title(),'UNGE');
   await page.waitForFunction(()=>document.querySelector('#nodeChoice').options.length===1);
   await page.locator('#loadProperties').click(); await page.locator('#propertyCard').waitFor({state:'visible'});
   const gain=page.locator('#property-1');
   await gain.fill('2.5'); await page.locator('#saveProperties').click();
   await page.waitForFunction(()=>window.applied.length===1 && document.querySelector('#saveProperties').disabled);
   assert.equal(await page.evaluate(()=>window.applied[0].command.commands[0].value),2.5);
   await gain.fill('9'); await page.locator('#saveProperties').click(); assert.equal(await gain.getAttribute('aria-invalid'),'true');
   assert.equal(await page.evaluate(()=>window.applied.length),1);
   await gain.fill('1.75'); await page.evaluate(()=>window.externalEdit()); assert.ok(await page.locator('#saveProperties').isDisabled()); assert.equal(await gain.inputValue(),'1.75');
   await page.locator('#locale').selectOption(locale==='ja'?'en':'ja'); assert.equal(await gain.inputValue(),'1.75');
   await page.locator('#locale').selectOption(locale);
   await page.locator('#loadProperties').click(); assert.equal(await gain.inputValue(),'2.5');
   await gain.fill('3'); await page.evaluate(()=>window.rejectSave='invalid_properties'); await page.locator('#saveProperties').click();
   await page.waitForFunction(()=>!document.querySelector('#propertyFields').disabled);
   assert.equal(await gain.inputValue(),'3'); assert.equal(await page.evaluate(()=>window.applied.length),1);
   await page.evaluate(()=>window.rejectSave=null); await page.locator('#saveProperties').click(); await page.waitForFunction(()=>window.applied.length===2);
   // Optional field removal is explicit; an untouched default never becomes a write.
   await page.locator('.property-field').nth(2).locator('.property-enable input').uncheck(); await page.locator('#saveProperties').click();
   await page.waitForFunction(()=>window.applied.length===3); assert.equal(await page.evaluate(()=>window.applied[2].command.commands[0].value),null);
   // Native form controls retain keyboard focus and both themes fit narrow windows.
   for (const theme of ['dark','light']) {
    await page.locator(theme==='dark'?'#themeDark':'#themeLight').click(); await page.waitForFunction(theme=>document.documentElement.dataset.theme===theme,theme);
    for(const width of [320,380,768]) {await page.setViewportSize({width,height:900}); assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));}
    await page.setViewportSize({width:380,height:900});
    if(process.env.UNGE_SCREENSHOTS){fs.mkdirSync(process.env.UNGE_SCREENSHOTS,{recursive:true});await page.locator('.property-inspector').screenshot({path:path.join(process.env.UNGE_SCREENSHOTS,`properties-${theme}-${locale}.png`)});}
   }
   await page.evaluate(()=>window.addJson()); await page.locator('#loadProperties').click();
   const json=page.locator('#property-5'); await json.fill('null'); await page.locator('#saveProperties').click();assert.equal(await json.getAttribute('aria-invalid'),'true');
   await json.fill('{"n":9007199254740993}');await page.locator('#saveProperties').click();assert.equal(await json.getAttribute('aria-invalid'),'true');
   await json.fill('{"n":2}'); await page.locator('#saveProperties').click();await page.waitForFunction(()=>window.applied.length===4);
   await page.locator('#property-0').uncheck(); await page.locator('#property-3').fill('17'); await page.locator('#property-4').selectOption('floor');
   await page.locator('#saveProperties').click(); await page.waitForFunction(()=>window.applied.length===5);
   assert.deepEqual(await page.evaluate(()=>window.applied[4].command.commands.map(c=>[c.key,c.value])),[['enabled',false],['offset',17],['rounding','floor']]);
   await page.locator('.property-field').nth(1).locator('.property-reset').click(); assert.equal(await gain.inputValue(),'1');
   assert.equal(await page.evaluate(()=>window.applied.length),5);
   // Failed reload retains the existing draft and values.
   await gain.fill('2');await page.evaluate(()=>window.rejectLoad=true);await page.locator('#loadProperties').click();await page.waitForFunction(()=>!document.querySelector('#propertyFields').disabled);assert.equal(await gain.inputValue(),'2');
   // Selection follows authoritative Rust metadata, even outside the current list page.
   await page.evaluate(()=>{window.rejectLoad=false;}); await page.locator('#loadProperties').click();
   await page.waitForFunction(()=>!document.querySelector('#propertyFields').disabled);
   await page.evaluate(()=>window.selectNodes(['node-2']));
   await page.waitForFunction(()=>document.querySelector('#propertyNodeType').textContent.includes('node-2'));
   assert.equal(await gain.inputValue(),'0.5');
   await gain.fill('2'); await page.evaluate(()=>window.selectNodes(['node-1']));
   await page.waitForFunction(()=>document.querySelector('#propertySelectionStatus').textContent.length>0);
   assert.equal(await gain.inputValue(),'2'); assert.ok((await page.locator('#propertyNodeType').textContent()).includes('node-2'));
   for (const width of [320,380,768]) {await page.setViewportSize({width,height:900}); assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));}
   await page.setViewportSize({width:380,height:900});
   if(process.env.UNGE_SCREENSHOTS){await page.locator('.property-inspector').screenshot({path:path.join(process.env.UNGE_SCREENSHOTS,`selection-held-${locale}.png`)});}
   await page.evaluate(()=>window.rejectSave='invalid_properties');
   await page.locator('#saveProperties').click(); await page.waitForFunction(()=>!document.querySelector('#propertyFields').disabled);
   assert.equal(await gain.inputValue(),'2'); assert.ok((await page.locator('#propertyNodeType').textContent()).includes('node-2'));
   await page.evaluate(()=>{window.rejectSave=null;window.holdSave=true;});
   await page.locator('#saveProperties').click(); await page.waitForFunction(()=>window.saveHeld);
   await page.evaluate(()=>{window.selectNodes([]);window.selectNodes(['node-1']);});
   assert.equal(await page.locator('#propertyFields').evaluate(el=>el.disabled),true);
   assert.ok(await gain.isDisabled());
   await page.evaluate(()=>window.releaseSave());
   await page.waitForFunction(()=>document.querySelector('#propertyNodeType').textContent.includes('node-1'));
   assert.equal(await page.evaluate(()=>window.applied.at(-1).command.commands[0].id),'node-2');
   // Empty/multiple selections retain dirty forms until explicit discard.
   await gain.fill('9'); await page.evaluate(()=>window.selectNodes([]));
   await page.waitForFunction(()=>document.querySelector('#propertySelectionStatus').textContent.length>0);
   assert.equal(await gain.inputValue(),'9'); await page.locator('#loadProperties').click();
   await page.locator('#propertyCard').waitFor({state:'hidden'});
   await page.evaluate(()=>window.selectNodes(['node-1','node-2']));
   await page.waitForFunction(()=>window.propertyInspector && document.querySelector('#propertiesStatus').textContent.length>0);
   assert.ok(await page.locator('#propertyCard').isHidden());
   await page.evaluate(()=>window.selectNodes(['node-1'])); await page.locator('#propertyCard').waitFor({state:'visible'});
   await gain.fill('2'); await page.evaluate(()=>{window.selectNodes(['node-2']);window.rejectLoad=true;});
   await page.locator('#loadProperties').click(); await page.waitForFunction(()=>!document.querySelector('#propertyFields').disabled);
   assert.equal(await gain.inputValue(),'2'); assert.ok((await page.locator('#propertyNodeType').textContent()).includes('node-1'));
   await page.evaluate(()=>{window.rejectLoad=false;window.holdSelection=true;}); await page.locator('#loadProperties').click();
   await page.waitForFunction(()=>window.selectionHeld);
   assert.ok(await page.locator('#saveProperties').isDisabled()); assert.ok(await gain.isDisabled());
   await page.evaluate(()=>window.releaseSelection());
   await page.waitForFunction(()=>document.querySelector('#propertyNodeType').textContent.includes('node-2'));
   // Out-of-order reads must never replace the newest selection.
   await page.evaluate(()=>window.selectNodes(['node-1']));
   await page.waitForFunction(()=>document.querySelector('#propertyNodeType').textContent.includes('node-1'));
   await page.evaluate(()=>{window.holdProperty='node-2';window.selectNodes(['node-2']);});
   await page.waitForFunction(()=>window.holdProperty===null);
   await page.evaluate(()=>{window.selectNodes(['node-1']);window.releaseProperties();});
   await page.waitForFunction(()=>!document.querySelector('#propertyFields').disabled);
   assert.ok((await page.locator('#propertyNodeType').textContent()).includes('node-1'));
   // Missing events recover through periodic reads; unchanged selection avoids property reload.
   await page.evaluate(()=>window.selectNodes(['node-2'],false));
   await page.waitForFunction(()=>document.querySelector('#propertyNodeType').textContent.includes('node-2'));
   const reads=await page.evaluate(()=>window.propertyRequests.length);
   await page.evaluate(()=>window.propertyInspector.refreshSelection());
   assert.equal(await page.evaluate(()=>window.propertyRequests.length),reads);
   await gain.fill('1.25'); await page.evaluate(()=>{window.rejectSelection=true;return window.propertyInspector.refreshSelection();});
   assert.equal(await gain.inputValue(),'1.25');
   await page.evaluate(()=>{window.rejectSelection=false;window.externalEdit();});
   assert.ok(await page.locator('#saveProperties').isDisabled()); assert.equal(await gain.inputValue(),'1.25');
   assert.deepEqual(errors,[]); await page.close();
  }
  console.log('Properties UI: three languages, both themes, narrow layout, save/validation/conflict/draft/JSON and selection/race/recovery flows passed');
 } finally { await browser.close(); }
})().catch(error=>{console.error(error);process.exitCode=1;});
