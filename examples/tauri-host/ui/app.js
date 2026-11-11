const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = id => document.getElementById(id);
const text = {
  en: { title:'Graph', description:'Select a node in the graph to edit its properties.', add:'Add number', addSum:'Add sum', help:'Drag nodes to move. Shift adds to the selection. Drag empty space to select; drag ports to connect. Two-finger scroll or right-drag to pan. Pinch on macOS or use the mouse wheel to zoom. Esc cancels a drag.', undo:'Undo', redo:'Redo', zoomLabel:'Zoom', nodes:'nodes', revision:'revision', failed:'Operation failed', themeLabel:'Appearance', themeDark:'Dark', themeLight:'Light', language:'Language' },
  ja: { title:'グラフ', description:'ノードを選択すると設定できます。', add:'数値追加', addSum:'加算追加', help:'ドラッグで移動／Shiftで追加選択\n余白のドラッグで範囲選択\nポート間をドラッグして接続\n2本指スクロール／右ドラッグで画面移動\nピンチ（macOS）／ホイールで拡大・縮小\nEscでドラッグを中止', undo:'元に戻す', redo:'やり直す', zoomLabel:'ズーム', nodes:'ノード', revision:'リビジョン', failed:'操作に失敗しました', themeLabel:'表示モード', themeDark:'ダーク', themeLight:'ライト', language:'言語' },
  'zh-CN': { title:'节点图', description:'选择节点以编辑属性。', add:'添加数值', addSum:'添加加法', help:'拖动节点移动，Shift追加选择。拖动空白处框选，拖动端口连接。双指滚动或右键拖动可平移。macOS捏合或鼠标滚轮可缩放。Esc取消拖动。', undo:'撤销', redo:'重做', zoomLabel:'缩放', nodes:'节点', revision:'版本', failed:'操作失败', themeLabel:'外观', themeDark:'深色', themeLight:'浅色', language:'语言' }
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
function addNode(sum, point = [40+(summary.nodes%3)*240,40+Math.floor(summary.nodes/3)*140], revision = summary.revision) {
  const port = name => ({name,data_type:{kind:'float'},cardinality:'single',required:true});
  return send({kind:'apply',expected_revision:revision,command:{kind:'add_node',node:{id:crypto.randomUUID(),type_id:sum?'math.add':'math.number',inputs:sum?[port('a'),port('b')]:[],outputs:[port('value')],properties:sum?{}:{value:42}},rect:{x:point[0],y:point[1],width:180,height:90}}});
}
$('add').onclick=()=>enqueue(()=>addNode(false));
$('addSum').onclick=()=>enqueue(()=>addNode(true));
for (const kind of ['undo','redo']) $(kind).onclick=()=>enqueue(()=>send({kind,expected_revision:summary.revision}));
$('zoom').oninput=()=>{ const zoom=Number($('zoom').value); enqueue(()=>send({kind:'set_viewport',viewport:{origin:[0,0],zoom,size:[1280,640]}})); };
translate();
listen('unge://changed',({payload})=>{if(payload.revision>=summary.revision){summary=payload;status(); window.groupControls?.observe(payload.revision); if (window.propertyInspector) window.propertyInspector.observe(payload.revision);}}).catch(error=>{console.error(error); $('status').textContent=text[locale].failed;});
enqueue(async ()=> {
  await send({kind:'set_locale',locale:locale.toLowerCase()});
  let saved;
  try { saved=localStorage.getItem('unge.theme'); } catch { /* Use the Rust view default. */ }
  if (saved === 'dark' || saved === 'light') await chooseTheme(saved, false);
  else await refreshAppearance();
});
listen('unge://appearance-changed',({payload})=>applyAppearance(payload)).catch(error=>console.error(error));

const inputErrors = {
  en: { storage_unavailable: "Saving failed. Stop editing and inspect storage before retrying.", storage_full: "Storage is full. Preserve history and increase the host capacity.", storage_locked: "Another process owns this document storage.", storage_corrupt: "Stored history is invalid. Preserve the files and inspect the backup pair.", invalid_policy: "The storage configuration does not match this document.", missing_group:'This group no longer exists. Refresh the list.', invalid_request:'The request is invalid.', state_unavailable:'The view is unavailable.', unknown_view:'This window is not registered.', revision_conflict:'The graph changed. Repeat the gesture.', invalid_connection:'These ports cannot be connected.', invalid_pointer:'The pointer position is invalid.', pointer_busy:'Another pointer is editing this view.' },
  ja: { storage_unavailable: "保存に失敗しました。編集を停止し、再試行前に保存先を確認してください。", storage_full: "保存先の容量が不足しています。履歴を保全し、ホストで上限を変更してください。", storage_locked: "別のプロセスがこの保存先を使用しています。", storage_corrupt: "保存履歴を検証できません。ファイルを保全し、バックアップの組を確認してください。", invalid_policy: "保存設定がこのドキュメントと一致しません。", missing_group:'このグループは削除されています。一覧を更新してください。', invalid_request:'リクエストが無効です。', state_unavailable:'画面を利用できません。', unknown_view:'このウインドウは登録されていません。', revision_conflict:'グラフが更新されました。操作をやり直してください。', invalid_connection:'このポート同士は接続できません。', invalid_pointer:'ポインターの座標が無効です。', pointer_busy:'別のポインターで操作中です。' },
  'zh-CN': { storage_unavailable: "保存失败。请停止编辑并在重试前检查存储。", storage_full: "存储容量不足。请保留历史并由宿主增加容量。", storage_locked: "另一个进程正在使用此存储。", storage_corrupt: "无法验证保存的历史。请保留文件并检查成对备份。", invalid_policy: "存储配置与此文档不一致。", missing_group:'此组已删除，请刷新列表。', invalid_request:'请求无效。', state_unavailable:'视图不可用。', unknown_view:'此窗口尚未注册。', revision_conflict:'节点图已更新，请重新操作。', invalid_connection:'这些端口无法连接。', invalid_pointer:'指针位置无效。', pointer_busy:'另一个指针正在操作。' }
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

createCanvasActions({
  invoke, listen, send, enqueue, addNode, locale: () => locale,
  errors: () => inputErrors[locale], fallback: () => text[locale].failed,
});
