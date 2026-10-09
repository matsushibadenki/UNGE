const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = id => document.getElementById(id);
const text = {
  en: { title:'A shared Rust document', description:'Select a node in the graph to edit its properties.', add:'Add number', addSum:'Add sum', help:'Drag nodes to move. Shift adds to the selection. Drag empty space to select; drag ports to connect. Right-drag pans, the wheel zooms, and Esc cancels.', undo:'Undo', redo:'Redo', zoomLabel:'Zoom', nodes:'nodes', revision:'revision', failed:'Operation failed', themeLabel:'Appearance', themeDark:'Dark', themeLight:'Light', language:'Language' },
  ja: { title:'Rustで共有するグラフ', description:'グラフのノードを選択して、プロパティを設定します。', add:'数値追加', addSum:'加算追加', help:'ドラッグで移動／Shiftで追加選択\n余白のドラッグで範囲選択\nポート間をドラッグして接続\n右ドラッグで画面移動\nホイールで拡大縮小／Escで中止', undo:'元に戻す', redo:'やり直す', zoomLabel:'ズーム', nodes:'ノード', revision:'リビジョン', failed:'操作に失敗しました', themeLabel:'表示モード', themeDark:'ダーク', themeLight:'ライト', language:'言語' },
  'zh-CN': { title:'共享的 Rust 文档', description:'在节点图中选中节点，设置其属性。', add:'添加数值', addSum:'添加加法', help:'拖动节点移动，Shift追加选择。拖动空白处框选，拖动端口连接。右键拖动平移，滚轮缩放，Esc取消。', undo:'撤销', redo:'重做', zoomLabel:'缩放', nodes:'节点', revision:'版本', failed:'操作失败', themeLabel:'外观', themeDark:'深色', themeLight:'浅色', language:'语言' }
};
let locale = navigator.language.startsWith('ja') ? 'ja' : navigator.language.startsWith('zh') ? 'zh-CN' : 'en';
let summary = { revision:0,nodes:0,edges:0 };
let pending = Promise.resolve();
function status() { $('status').textContent = `${summary.nodes} ${text[locale].nodes} · ${text[locale].revision} ${summary.revision}`; }
function translate() { document.documentElement.lang=locale; for (const key of ['title','description','add','addSum','undo','redo','zoomLabel','help','themeLabel','themeDark','themeLight']) $(key).textContent=text[locale][key]; $('locale').setAttribute('aria-label',text[locale].language); status(); if (window.executionUI) window.executionUI.translate(locale); if (window.accessibleGraph) window.accessibleGraph.translate(); if (window.groupControls) window.groupControls.translate(); if (window.propertyInspector) window.propertyInspector.translate(); if (window.panelControls) window.panelControls.translate(); }
async function send(request) { summary=await invoke('dispatch',{request}); status(); return summary; }
function applyAppearance(appearance) {
  const root = document.documentElement;
  root.dataset.theme = appearance.theme;
  for (const [key, value] of Object.entries(appearance.colors)) root.style.setProperty(`--${key}`, value);
  for (const [id, theme] of [['themeDark','dark'],['themeLight','light']]) $(id).setAttribute('aria-pressed', String(appearance.theme === theme));
}
async function refreshAppearance() { applyAppearance(await invoke('appearance')); }
async function chooseTheme(theme, remember = true) {
  await send({kind:'set_theme',theme});
  await refreshAppearance();
  if (remember) { try { localStorage.setItem('unge.theme',theme); } catch { /* Storage may be disabled by the host. */ } }
}
function enqueue(action) { pending=pending.then(action).catch(async error => { await send({kind:'summary'}).catch(()=>{}); console.error(error); $('status').textContent=inputErrors[locale][error.code] ?? text[locale].failed; }); }
for (const [id, theme] of [['themeDark','dark'],['themeLight','light']]) $(id).onclick=()=>enqueue(()=>chooseTheme(theme));
$('locale').value=locale; $('locale').onchange=()=>{locale=$('locale').value;translate();enqueue(async ()=>{ await send({kind:'set_locale',locale:locale.toLowerCase()}); await window.accessibleGraph.refresh(); });};
function addNode(sum) {
  const port = name => ({name,data_type:{kind:'float'},cardinality:'single',required:true});
  return send({kind:'apply',expected_revision:summary.revision,command:{kind:'add_node',node:{id:crypto.randomUUID(),type_id:sum?'math.add':'math.number',inputs:sum?[port('a'),port('b')]:[],outputs:[port('value')],properties:sum?{}:{value:42}},rect:{x:40+(summary.nodes%3)*240,y:40+Math.floor(summary.nodes/3)*140,width:180,height:90}}});
}
$('add').onclick=()=>enqueue(()=>addNode(false));
$('addSum').onclick=()=>enqueue(()=>addNode(true));
for (const kind of ['undo','redo']) $(kind).onclick=()=>enqueue(()=>send({kind,expected_revision:summary.revision}));
$('zoom').oninput=()=>{ const zoom=Number($('zoom').value); enqueue(()=>send({kind:'set_viewport',viewport:{origin:[0,0],zoom,size:[1280,640]}})); };
translate();
listen('unge://changed',({payload})=>{if(payload.revision>=summary.revision){summary=payload;status(); if (window.propertyInspector) window.propertyInspector.observe(payload.revision);}}).catch(error=>{console.error(error); $('status').textContent=text[locale].failed;});
enqueue(async ()=> {
  await send({kind:'set_locale',locale:locale.toLowerCase()});
  let saved;
  try { saved=localStorage.getItem('unge.theme'); } catch { /* Use the Rust view default. */ }
  if (saved === 'dark' || saved === 'light') await chooseTheme(saved, false);
  else await refreshAppearance();
});
listen('unge://appearance-changed',({payload})=>applyAppearance(payload)).catch(error=>console.error(error));

const inputErrors = {
  en: { missing_group:'This group no longer exists. Refresh the list.', invalid_request:'The request is invalid.', state_unavailable:'The view is unavailable.', unknown_view:'This window is not registered.', revision_conflict:'The graph changed. Repeat the gesture.', invalid_connection:'These ports cannot be connected.', invalid_pointer:'The pointer position is invalid.', pointer_busy:'Another pointer is editing this view.' },
  ja: { missing_group:'このグループは削除されています。一覧を更新してください。', invalid_request:'リクエストが無効です。', state_unavailable:'画面を利用できません。', unknown_view:'このウインドウは登録されていません。', revision_conflict:'グラフが更新されました。操作をやり直してください。', invalid_connection:'このポート同士は接続できません。', invalid_pointer:'ポインターの座標が無効です。', pointer_busy:'別のポインターで操作中です。' },
  'zh-CN': { missing_group:'此组已删除，请刷新列表。', invalid_request:'请求无效。', state_unavailable:'视图不可用。', unknown_view:'此窗口尚未注册。', revision_conflict:'节点图已更新，请重新操作。', invalid_connection:'这些端口无法连接。', invalid_pointer:'指针位置无效。', pointer_busy:'另一个指针正在操作。' }
};
listen('unge://interaction-error', ({payload}) => { console.error(payload); $('status').textContent=inputErrors[locale][payload.code] ?? text[locale].failed; });

window.executionUI = createExecutionUI({ invoke, listen, locale: () => locale, revision: () => summary.revision });
window.executionUI.translate(locale);

window.accessibleGraph = createAccessibleGraph({ invoke, send, enqueue, locale: () => locale });
enqueue(() => window.accessibleGraph.refresh());

window.groupControls = createGroupControls({ invoke, send, enqueue, locale: () => locale });
enqueue(() => window.groupControls.refresh());

window.propertyInspector = createPropertyInspector({ invoke, send, locale: () => locale, selectedId: () => $('nodeChoice').value });

window.panelControls = createPanelControls({ invoke, listen, locale: () => locale });

// Native pointer selection can change without a document revision. Events invalidate
// pending reads; focus and a low-frequency visible-page query recover missed events.
listen('unge://selection-changed', () => window.propertyInspector.refreshSelection()).catch(console.error);
window.addEventListener('focus', () => window.propertyInspector.refreshSelection());
document.addEventListener('visibilitychange', () => { if (!document.hidden) window.propertyInspector.refreshSelection(); });
setInterval(() => { if (!document.hidden) window.propertyInspector.refreshSelection({invalidate:false}); }, 2000);
window.propertyInspector.refreshSelection();
