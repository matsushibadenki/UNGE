/* Only group summaries cross IPC. Rust resolves memberships and the invoking view's selection. */
function createGroupControls({ invoke, send, enqueue, locale }) {
  const $ = id => document.getElementById(id);
  const messages = {
    en: { groupsLabel:'Groups', groupsHelp:'Select nodes in the graph or node controls, then create a group or add/remove members. Drag the group header to move its members. Use the fold button to switch between a card and the frame.', groupChoiceLabel:'Group', groupNameLabel:'New name (1–256 characters)', createGroup:'Create from selection', renameGroup:'Rename group', collapse:'Collapse group', expand:'Expand group', selectGroup:'Select members', addGroupMembers:'Add selected nodes', removeGroupMembers:'Remove selected nodes', deleteGroup:'Delete group', refreshGroups:'Refresh groups', firstGroups:'First page', nextGroups:'Next page', empty:'No groups', members:'members', selected:'selected nodes', page:'groups on this page' },
    ja: { groupsLabel:'グループ', groupsHelp:'選択からグループを作成します。\nノードを追加・除外できます。\n見出しをドラッグして一括移動できます。\n折りたたみボタンで表示を切り替えます。', groupChoiceLabel:'グループ', groupNameLabel:'新しい名前（1〜256文字）', createGroup:'選択から作成', renameGroup:'名前を変更', collapse:'グループを折りたたむ', expand:'グループを展開', selectGroup:'所属を選択', addGroupMembers:'選択を追加', removeGroupMembers:'選択を除外', deleteGroup:'グループを削除', refreshGroups:'一覧を更新', firstGroups:'最初のページ', nextGroups:'次のページ', empty:'グループがありません', members:'所属ノード', selected:'選択', page:'表示グループ' },
    'zh-CN': { groupsLabel:'组', groupsHelp:'从所选节点创建组。\n添加或移除所选节点。\n拖动组标题可移动所有成员。\n使用折叠按钮切换显示。', groupChoiceLabel:'组', groupNameLabel:'新名称（1〜256个字符）', createGroup:'从选择创建', renameGroup:'重命名组', collapse:'折叠组', expand:'展开组', selectGroup:'选中成员', addGroupMembers:'添加所选节点', removeGroupMembers:'移除所选节点', deleteGroup:'删除组', refreshGroups:'刷新列表', firstGroups:'第一页', nextGroups:'下一页', empty:'没有组', members:'成员', selected:'所选节点', page:'本页组数' }
  };
  let page = null, cursor = null, observed = 0, queuedRefresh = false;
  function current() { return page?.groups.find(group => group.id === $('groupChoice').value); }
  function buttons() {
    const group = current(), selected = (page?.selected_nodes ?? 0) > 0;
    const stale = !page || page.revision < observed;
    for (const id of ['renameGroup','selectGroup','deleteGroup']) $(id).disabled = stale || !group;
    for (const id of ['addGroupMembers','removeGroupMembers']) $(id).disabled = stale || !group || !selected;
    $('createGroup').disabled = stale || !selected;
    $('toggleGroup').disabled = stale || !group?.members;
    $('toggleGroup').textContent = messages[locale()][group?.collapsed ? 'expand' : 'collapse'];
  }
  function render() {
    const previous = $('groupChoice').value, m = messages[locale()];
    $('groupChoice').replaceChildren();
    for (const group of page?.groups ?? []) {
      const option = document.createElement('option');
      option.value = group.id;
      option.textContent = `${group.label || group.id} · ${group.id} · ${m.members}: ${group.members}`;
      $('groupChoice').append(option);
    }
    if (page?.groups.some(group => group.id === previous)) $('groupChoice').value = previous;
    $('groupChoice').disabled = !page?.groups.length;
    $('nextGroups').disabled = !page?.next;
    $('groupStatus').textContent = `${page?.groups.length ? `${m.page}: ${page.groups.length} / ${page.total}` : m.empty} · ${m.selected}: ${page?.selected_nodes ?? 0}`;
    buttons();
  }
  async function refresh(after = cursor) {
    const summary = await send({ kind:'summary' });
    const result = await invoke('groups', { expectedRevision:summary.revision, after, limit:50 });
    cursor = after; page = result; render();
  }
  function translate() {
    for (const id of ['groupsLabel','groupsHelp','groupChoiceLabel','groupNameLabel','createGroup','renameGroup','selectGroup','addGroupMembers','removeGroupMembers','deleteGroup','refreshGroups','firstGroups','nextGroups']) $(id).textContent = messages[locale()][id];
    // Never replace a user-entered name with a truncated summary label.
    render();
  }
  function act(kind) {
    const group = current(), revision = page?.revision;
    if (!page || revision < observed || (kind !== 'create' && !group)) return;
    const action = { kind, id:kind === 'create' ? crypto.randomUUID() : group.id };
    if (kind === 'set_collapsed') action.collapsed = !group.collapsed;
    if (kind === 'create' || kind === 'rename') {
      if (!$('groupName').reportValidity()) return;
      action.label = $('groupName').value;
    }
    enqueue(async () => {
      await send({ kind:'group', expected_revision:revision, action });
      await refresh(kind === 'create' || kind === 'delete' ? null : cursor);
    });
  }
  for (const [id,kind] of [['createGroup','create'],['renameGroup','rename'],['toggleGroup','set_collapsed'],['selectGroup','select_members'],['addGroupMembers','add_selection'],['removeGroupMembers','remove_selection'],['deleteGroup','delete']]) $(id).onclick = () => act(kind);
  $('groupChoice').onchange = buttons;
  $('refreshGroups').onclick = () => enqueue(() => refresh());
  $('firstGroups').onclick = () => enqueue(() => refresh(null));
  $('nextGroups').onclick = () => { const after=page?.next; if (after) enqueue(() => refresh(after)); };
  translate();
  function observe(revision) {
    observed = Math.max(observed, revision);
    if (!page || page.revision >= observed || queuedRefresh) return;
    buttons();
    queuedRefresh = true;
    enqueue(async () => {
      queuedRefresh = false;
      if (page && page.revision < observed) await refresh();
    });
  }
  return { refresh, translate, observe };
}
