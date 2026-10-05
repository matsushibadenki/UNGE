/* Bounded presentation metadata only. Rust owns selection, placement and revision. */
function createAccessibleGraph({ invoke, send, enqueue, locale }) {
  const $ = id => document.getElementById(id);
  const messages = {
    en: { graphLabel:'Keyboard graph controls', graphHelp:'Choose a node, then select, move or delete it. Coordinates are in graph units. Each page contains up to 50 nodes.', nodeLabel:'Node', selectNode:'Select node', moveNode:'Move node', deleteNode:'Delete node', previousNodes:'First page', nextNodes:'Next page', refreshNodes:'Refresh nodes', empty:'No nodes', page:'nodes on this page', selected:'selected', xLabel:'X position', yLabel:'Y position' },
    ja: { graphLabel:'キーボードでグラフを操作', graphHelp:'ノードを選び、選択・移動・削除できます。座標はグラフ内の単位です。1ページに最大50ノードを表示します。', nodeLabel:'ノード', selectNode:'ノードを選択', moveNode:'ノードを移動', deleteNode:'ノードを削除', previousNodes:'最初のページ', nextNodes:'次のページ', refreshNodes:'一覧を更新', empty:'ノードがありません', page:'このページのノード数', selected:'選択中', xLabel:'X座標', yLabel:'Y座標' },
    'zh-CN': { graphLabel:'键盘节点图操作', graphHelp:'选择节点后，可以选中、移动或删除。坐标以节点图单位表示。每页最多显示50个节点。', nodeLabel:'节点', selectNode:'选中节点', moveNode:'移动节点', deleteNode:'删除节点', previousNodes:'第一页', nextNodes:'下一页', refreshNodes:'刷新列表', empty:'没有节点', page:'本页节点数', selected:'已选中', xLabel:'X坐标', yLabel:'Y坐标' }
  };
  let page = null;
  let cursor = null;
  let dirty = false;
  function current() { return page?.nodes.find(node => node.id === $('nodeChoice').value); }
  function coordinates() {
    const node = current();
    for (const id of ['selectNode','moveNode','deleteNode','nodeX','nodeY']) $(id).disabled = !node;
    $('nodeX').value = node ? String(node.rect.x) : '';
    $('nodeY').value = node ? String(node.rect.y) : '';
    dirty = false;
  }
  function render() {
    const previous = $('nodeChoice').value;
    $('nodeChoice').replaceChildren();
    const m = messages[locale()];
    for (const node of page?.nodes ?? []) {
      const option = document.createElement('option');
      option.value = node.id;
      option.textContent = `${node.title} · ${node.id}${node.selected ? ` · ${m.selected}` : ''}`;
      $('nodeChoice').append(option);
    }
    if (page?.nodes.some(node => node.id === previous)) $('nodeChoice').value = previous;
    $('nodeChoice').disabled = !page?.nodes.length;
    $('nextNodes').disabled = !page?.next;
    $('graphStatus').textContent = page?.nodes.length ? `${m.page}: ${page.nodes.length} / ${page.total}` : m.empty;
    coordinates();
  }
  async function refresh(after = cursor) {
    const summary = await send({ kind:'summary' });
    // Do not retry edits on a new revision. Only refresh this read-only page.
    const result = await invoke('accessible_nodes', { expectedRevision:summary.revision, after, limit:50 });
    cursor = after;
    page = result;
    render();
  }
  function translate() {
    for (const id of ['graphLabel','graphHelp','nodeLabel','selectNode','moveNode','deleteNode','previousNodes','nextNodes','refreshNodes','xLabel','yLabel']) $(id).textContent = messages[locale()][id];
    if (!dirty) render();
  }
  $('nodeChoice').onchange = coordinates;
  for (const id of ['nodeX','nodeY']) $(id).oninput = () => { dirty = true; };
  $('refreshNodes').onclick = () => enqueue(() => refresh());
  $('previousNodes').onclick = () => enqueue(() => refresh(null));
  $('nextNodes').onclick = () => { const after = page?.next; if (after) enqueue(() => refresh(after)); };
  $('selectNode').onclick = () => {
    const node = current();
    if (node) enqueue(async () => { await send({ kind:'select', ids:[node.id] }); await refresh(); });
  };
  $('deleteNode').onclick = () => {
    const node = current(), revision = page?.revision;
    if (node) enqueue(async () => { await send({ kind:'apply', expected_revision:revision, command:{ kind:'remove_node', id:node.id } }); await refresh(); });
  };
  $('moveForm').onsubmit = event => {
    event.preventDefault();
    if (!$('moveForm').reportValidity()) return;
    const node = current(), revision = page?.revision;
    const x = Number($('nodeX').value), y = Number($('nodeY').value);
    if (!node || !Number.isFinite(x) || !Number.isFinite(y)) return;
    const rect = { ...node.rect, x, y };
    enqueue(async () => { await send({ kind:'apply', expected_revision:revision, command:{ kind:'move_node', id:node.id, rect } }); await refresh(); });
  };
  translate();
  return { refresh, translate };
}
