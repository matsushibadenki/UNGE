const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = id => document.getElementById(id);
const text = {
  en: { title:'A shared Rust document', description:'Controls live in this WebView. The second window renders the graph with wgpu.', add:'Add number', addSum:'Add sum', help:'Drag nodes to move. Shift adds to the selection. Drag empty space to select; drag ports to connect. Right-drag pans, the wheel zooms, and Esc cancels.', undo:'Undo', redo:'Redo', zoomLabel:'Zoom', nodes:'nodes', revision:'revision', failed:'Operation failed' },
  ja: { title:'Rustで共有するグラフ', description:'この画面から操作します。\nグラフは別画面に描画します。', add:'数値追加', addSum:'加算追加', help:'ドラッグで移動／Shiftで追加選択\n余白のドラッグで範囲選択\nポート間をドラッグして接続\n右ドラッグで画面移動\nホイールで拡大縮小／Escで中止', undo:'元に戻す', redo:'やり直す', zoomLabel:'ズーム', nodes:'ノード', revision:'リビジョン', failed:'操作に失敗しました' },
  'zh-CN': { title:'共享的 Rust 文档', description:'通过此 WebView 操作，在另一个窗口中使用 wgpu 绘制节点图。', add:'添加数值', addSum:'添加加法', help:'拖动节点移动，Shift追加选择。拖动空白处框选，拖动端口连接。右键拖动平移，滚轮缩放，Esc取消。', undo:'撤销', redo:'重做', zoomLabel:'缩放', nodes:'节点', revision:'版本', failed:'操作失败' }
};
let locale = navigator.language.startsWith('ja') ? 'ja' : navigator.language.startsWith('zh') ? 'zh-CN' : 'en';
let summary = { revision:0,nodes:0,edges:0 };
let pending = Promise.resolve();
function status() { $('status').textContent = `${summary.nodes} ${text[locale].nodes} · ${text[locale].revision} ${summary.revision}`; }
function translate() { document.documentElement.lang=locale; for (const key of ['title','description','add','addSum','undo','redo','zoomLabel','help']) $(key).textContent=text[locale][key]; status(); }
async function send(request) { summary=await invoke('dispatch',{request}); status(); return summary; }
function enqueue(action) { pending=pending.then(action).catch(async error => { await send({kind:'summary'}).catch(()=>{}); $('status').textContent=`${text[locale].failed}: ${error.code ?? error}`; }); }
$('locale').value=locale; $('locale').onchange=()=>{locale=$('locale').value;translate();enqueue(()=>send({kind:'set_locale',locale:locale.toLowerCase()}));};
function addNode(sum) {
  const port = name => ({name,data_type:{kind:'float'},cardinality:'single',required:true});
  return send({kind:'apply',expected_revision:summary.revision,command:{kind:'add_node',node:{id:crypto.randomUUID(),type_id:sum?'math.add':'math.number',inputs:sum?[port('a'),port('b')]:[],outputs:[port('value')],properties:sum?{}:{value:42}},rect:{x:40+(summary.nodes%3)*240,y:40+Math.floor(summary.nodes/3)*140,width:180,height:90}}});
}
$('add').onclick=()=>enqueue(()=>addNode(false));
$('addSum').onclick=()=>enqueue(()=>addNode(true));
for (const kind of ['undo','redo']) $(kind).onclick=()=>enqueue(()=>send({kind,expected_revision:summary.revision}));
$('zoom').oninput=()=>{ const zoom=Number($('zoom').value); enqueue(()=>send({kind:'set_viewport',viewport:{origin:[0,0],zoom,size:[960,640]}})); };
translate();
listen('unge://changed',({payload})=>{if(payload.revision>=summary.revision){summary=payload;status();}}).catch(error=>{$('status').textContent=String(error);});
enqueue(()=>send({kind:'set_locale',locale:locale.toLowerCase()}));

const inputErrors = {
  en: { revision_conflict:'The graph changed. Repeat the gesture.', invalid_connection:'These ports cannot be connected.', invalid_pointer:'The pointer position is invalid.', pointer_busy:'Another pointer is editing this view.' },
  ja: { revision_conflict:'グラフが更新されました。操作をやり直してください。', invalid_connection:'このポート同士は接続できません。', invalid_pointer:'ポインターの座標が無効です。', pointer_busy:'別のポインターで操作中です。' },
  'zh-CN': { revision_conflict:'节点图已更新，请重新操作。', invalid_connection:'这些端口无法连接。', invalid_pointer:'指针位置无效。', pointer_busy:'另一个指针正在操作。' }
};
listen('unge://interaction-error', ({payload}) => { console.error(payload); $('status').textContent=inputErrors[locale][payload.code] ?? text[locale].failed; });
