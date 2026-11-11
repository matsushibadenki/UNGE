/* Canvas actions carry the revision and target captured by the native menu. */
function createCanvasActions({ invoke, listen, send, enqueue, addNode, locale, errors, fallback }) {
  const $ = id => document.getElementById(id);
  const dialog = $('canvasDialog'), body = $('canvasDialogBody'), status = $('canvasDialogStatus');
  const tr = (en, ja, zh) => ({ en, ja, 'zh-CN':zh })[locale()];
  const apply = (revision, command) => send({ kind:'apply', expected_revision:revision, command });
  const group = (revision, action) => send({ kind:'group', expected_revision:revision, action });
  const panels = { nodeChoice:'graphLabel', groupChoice:'groupsLabel', run:'executionLabel', help:'title' };
  let restore, source, noticeTimer, moving = false;
  function errorMessage(error) { return errors()[error?.code] ?? fallback(); }
  function returnContent() {
    restore?.(); restore = null;
    body.replaceChildren(); status.textContent = '';
    window.propertyInspector.close();
  }
  dialog.addEventListener('close', () => {
    if (dialog.open) return;
    returnContent();
    delete dialog.dataset.canvas;
    if (moving) {
      moving = false;
      enqueue(() => invoke('canvas_editor', { position:null }));
    }
    window.propertyInspector.refreshSelection();
  });
  $('canvasDialogClose').onclick = () => dialog.close();
  async function show(title, position, height = 460) {
    $('canvasDialogTitle').textContent = title;
    $('canvasDialogClose').setAttribute('aria-label', tr('Close','閉じる','关闭'));
    if (!dialog.open) dialog.showModal();
    if (!moving && position) {
      moving = await invoke('canvas_editor', { position, height }).catch(() => false);
      if (moving && dialog.open) dialog.dataset.canvas = 'true';
      if (!dialog.open && moving) {
        moving = false;
        await invoke('canvas_editor', { position:null });
      }
    }
  }
  function borrow(element) {
    const marker = document.createComment('canvas editor');
    element.before(marker); body.replaceChildren(element);
    restore = () => marker.replaceWith(element);
  }
  async function form(title, action, fields, submit) {
    returnContent();
    const form = document.createElement('form');
    const inputs = fields.map(({ label, value, type = 'text', min, max, step, maxlength }) => {
      const row = document.createElement('label'), input = document.createElement('input');
      row.textContent = label; input.type = type; input.value = value ?? ''; input.required = true;
      if (min != null) input.min = min;
      if (max != null) input.max = max;
      if (step != null) input.step = step;
      if (maxlength != null) input.maxLength = maxlength;
      row.append(input); form.append(row); return input;
    });
    const button = document.createElement('button');
    button.type = 'submit'; button.textContent = tr('Apply','適用','应用');
    form.append(button);
    form.onsubmit = event => {
      event.preventDefault();
      button.disabled = true;
      const values = inputs.map(input => input.type === 'number' ? input.valueAsNumber : input.value.trim());
      enqueue(async () => {
        try { await submit(values); dialog.close(); }
        catch (error) { status.textContent = errorMessage(error); }
        finally { button.disabled = false; }
      });
    };
    body.append(form);
    await show(title, action.position, fields.length > 1 ? 260 : 200); inputs[0]?.focus();
  }
  function notify(message) {
    const notice = $('canvasNotice');
    notice.textContent = message;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => { notice.textContent = ''; }, 6500);

  }
  async function act(action) {
    const { revision, id, position } = action;
    if (action.kind !== 'port') { source = null; $('canvasNotice').textContent = ''; }
    switch (action.kind) {
      case 'request':
        await send(action.request);
        if (action.request.kind === 'apply' || ['undo','redo'].includes(action.request.kind)) await window.accessibleGraph.refresh();
        break;
      case 'add': await addNode(action.sum, action.point, revision); await window.accessibleGraph.refresh(); break;
      case 'control':
        if (id === 'run') await window.executionUI.run(revision);
        else $(id).click();
        break;
      case 'locale': $('locale').value = action.value; $('locale').onchange(); break;
      case 'browse':
        returnContent();
        if (action.target === 'nodeChoice') await window.accessibleGraph.refresh();
        if (action.target === 'groupChoice') await window.groupControls.refresh();
        if (action.target === 'help') {
          const help = document.createElement('p');
          help.textContent = $('help').textContent;
          body.append(help);
        } else borrow($(action.target).closest('section'));
        await show($(panels[action.target]).textContent, position);
        $(action.target === 'help' ? 'canvasDialogClose' : action.target).focus();
        break;
      case 'properties': {
        returnContent();
        borrow($('propertyForm').closest('section'));
        await show($('propertiesTitle').textContent, position);
        try {
          if (await window.propertyInspector.open(id, revision)) {
            $('propertyFields').querySelector('input:not([disabled]), textarea:not([disabled]), select:not([disabled])')?.focus();
          } else {
            status.textContent = tr('Apply or discard the existing draft first, then reopen this menu.', '既存の下書きを適用または破棄してから、このメニューを開き直してください。', '请先应用或放弃现有草稿，再重新打开此菜单。');
          }
        } catch (error) { status.textContent = errorMessage(error); }
        break;
      }
      case 'move':
        await form(tr('Set coordinates','座標を設定','设置坐标'), action,
          ['x','y'].map(axis => ({ label:axis.toUpperCase(), value:action.rect[axis], type:'number', step:'any', min:-1e9, max:1e9 })),
          ([x,y]) => apply(revision,{ kind:'move_node', id, rect:{ ...action.rect,x,y } }));
        break;
      case 'group_name':
        await form(tr('Group name','グループ名','组名称'), action,
          [{ label:tr('Name','名前','名称'), value:action.label, maxlength:256 }],
          async ([label]) => {
            if (!label) throw {code:'invalid_request'};
            if (action.select) {
              const now = await send({kind:'summary'});
              if (now.revision !== revision) throw {code:'revision_conflict'};
              await send({kind:'select',ids:[action.select]});
            }
            await group(revision, { kind:id ? 'rename' : 'create', id:id ?? crypto.randomUUID(), label });
          });
        break;
      case 'zoom':
        await form(tr('Zoom','ズーム','缩放'), action,
          [{ label:'%', value:Math.round(action.viewport.zoom*100), type:'number', step:'any', min:2, max:1600 }],
          ([value]) => send({kind:'set_viewport', viewport:{ ...action.viewport,zoom:value/100 }}));
        break;
      case 'port': {
        const previous = source;
        if (!previous || previous.output === action.output) {
          source = action;
          notify(tr('Right-click the opposite port and choose Start / finish connection.', '接続先のポートを右クリックし、「接続を開始／確定」を選んでください。', '右键单击另一端口，选择“开始或完成连接”。'));
          break;
        }
        source = null; $('canvasNotice').textContent = '';
        try {
          if (revision !== previous.revision) throw {code:'revision_conflict'};
          const [from,to] = action.output ? [action.endpoint,previous.endpoint] : [previous.endpoint,action.endpoint];
          await apply(revision,{kind:'connect',edge:{id:crypto.randomUUID(),from,to}});
        } catch (error) { notify(errorMessage(error.code === 'invalid_command' ? {code:'invalid_connection'} : error)); }
        break;
      }
    }
  }
  listen('unge://changed', ({payload}) => {
    if (source && source.revision !== payload.revision) { source = null; $('canvasNotice').textContent = ''; }
  }).catch(console.error);
  listen('unge://canvas-cancel', () => { source = null; $('canvasNotice').textContent = ''; }).catch(console.error);
  listen('unge://canvas-action', ({payload}) => enqueue(() => act(payload))).catch(console.error);
}
