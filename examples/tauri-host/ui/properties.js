/* Portable, schema-driven inspector. Holds a one-node draft; Rust owns the document. */
function createPropertyInspector({ invoke, send, locale, selectedId }) {
  const $ = id => document.getElementById(id);
  const words = {
    en: { title:'Properties', load:'Edit chosen node', discard:'Discard draft & reload', empty:'Choose a node above to edit its properties.', save:'Apply changes', reset:'Use default', enabled:'Set value', none:'This node has no editable properties.', saved:'Changes saved. Undo restores the previous values.', dirty:'Unsaved changes', ready:'Values from the shared document', stale:'The graph changed. Your draft is preserved. Reload before applying.', invalid:'Check the highlighted values.', failed:'Unable to load or save properties.', missing_node:'This node was deleted. Choose another node.', properties_unavailable:'This host has no property definitions configured.', limit_exceeded:'These properties exceed the inspector limit.', revision_conflict:'The graph changed. Your draft is preserved. Reload before applying.', invalid_properties:'These values were rejected by the node definition.', required:'Required', optional:'Optional', loading:'Working…', range:'Allowed range', number:'Enter a finite number within the allowed range.', integer:'Enter a safe integer within the allowed range.', string:'Check the text length and allowed choices.', json:'Enter valid JSON. Null removes a property and cannot be saved as a value.', unsafe:'Integers outside the JavaScript safe range cannot be edited here.', revision:'Revision' },
    ja: { title:'プロパティ', load:'選択候補のノードを編集', discard:'変更を破棄して再読込', empty:'上の一覧でノードを選び、設定を読み込んでください。', save:'変更を適用', reset:'初期値に戻す', enabled:'値を設定', none:'このノードに編集可能な設定はありません。', saved:'保存しました。「元に戻す」で復元できます。', dirty:'未保存の変更があります', ready:'共有ドキュメントの設定値', stale:'グラフが更新されました。入力内容は保持しています。再読込してから適用してください。', invalid:'強調された値を確認してください。', failed:'設定の読込・保存に失敗しました。', missing_node:'ノードは削除されています。別のノードを選んでください。', properties_unavailable:'ホストにプロパティ定義が設定されていません。', limit_exceeded:'設定がインスペクターの上限を超えています。', revision_conflict:'グラフが更新されました。入力内容は保持しています。再読込してから適用してください。', invalid_properties:'ノードの定義に適合しない値です。', required:'必須', optional:'任意', loading:'処理中…', range:'設定範囲', number:'設定範囲内の有限の数値を入力してください。', integer:'設定範囲内の安全な整数を入力してください。', string:'文字数と選択肢を確認してください。', json:'有効なJSONを入力してください。nullは削除を意味するため値として保存できません。', unsafe:'JavaScriptの安全な整数範囲を超える値は、この画面では編集できません。', revision:'リビジョン' },
    'zh-CN': { title:'属性', load:'编辑列表中的节点', discard:'放弃更改并重新加载', empty:'在上方列表中选择节点，然后加载属性。', save:'应用更改', reset:'恢复默认值', enabled:'设置值', none:'此节点没有可编辑的属性。', saved:'已保存。可以撤销以恢复原值。', dirty:'有未保存的更改', ready:'共享文档中的属性值', stale:'节点图已更新，输入已保留。请重新加载后再应用。', invalid:'请检查标记的值。', failed:'无法加载或保存属性。', missing_node:'此节点已删除，请选择其他节点。', properties_unavailable:'此应用未配置属性定义。', limit_exceeded:'属性超出检查器限制。', revision_conflict:'节点图已更新，输入已保留。请重新加载后再应用。', invalid_properties:'这些值不符合节点定义。', required:'必填', optional:'可选', loading:'处理中…', range:'允许范围', number:'请输入允许范围内的有限数值。', integer:'请输入允许范围内的安全整数。', string:'请检查文字长度和允许的选项。', json:'请输入有效的JSON。null表示删除，不能作为值保存。', unsafe:'无法编辑超出JavaScript安全整数范围的值。', revision:'版本' }
  };
  let latestRevision = 0;
  let snapshot = null, fields = [], captions = [], busy = false, dirty = false, stale = false, message = 'empty';
  const t = key => words[locale()][key] || words[locale()].failed;
  const local = value => value?.[locale() === 'zh-CN' ? 'zh_cn' : locale()] || value?.en || '';
  const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
  function element(tag, className, text) {
    const el = document.createElement(tag); if (className) el.className = className;
    if (text !== undefined) el.textContent = text; return el;
  }
  function translate() {
    $('propertiesTitle').textContent = t('title');
    $('loadProperties').textContent = t(dirty || stale ? 'discard' : 'load');
    $('saveProperties').textContent = t('save');
    $('propertiesStatus').textContent = t(busy ? 'loading' : message);
    $('propertiesStatus').dataset.warning = String(stale || ['invalid','invalid_properties','failed','revision_conflict'].includes(message));
    $('loadProperties').disabled = busy;
    $('saveProperties').disabled = busy || stale || !dirty;
    $('propertyFields').disabled = busy;
    if (snapshot) {
      $('propertyNodeName').textContent = local(snapshot.name) || snapshot.type_id;
      $('propertyNodeDescription').textContent = local(snapshot.description);
      $('propertyNodeType').textContent = `${snapshot.type_id} · v${snapshot.version} · ${t('revision')} ${snapshot.revision} · #${snapshot.id.slice(0,8)}`;
    }
    captions.forEach(update => update());
  }
  function markDirty() { dirty = true; message = stale ? 'stale' : 'dirty'; translate(); }
  function build(data) {
    snapshot = data; fields = []; captions = []; dirty = false; stale = latestRevision > data.revision;
    $('propertyCard').hidden = false;
    $('propertyFields').replaceChildren();
    Object.entries(data.schema.fields).forEach(([key, definition], index) => {
      const type = definition.value_type;
      const row = element('div','property-field'), heading = element('div','property-heading');
      const label = element('label'), badge = element('span','property-optional');
      const id = `property-${index}`, error = element('span','property-error'), hint = element('small','property-hint');
      const input = element(type.kind === 'string' && type.choices ? 'select' : ['json','string'].includes(type.kind) ? 'textarea' : 'input');
      input.id = id; label.htmlFor = id; error.id = `${id}-error`; hint.id = `${id}-hint`;
      input.setAttribute('aria-describedby', `${hint.id} ${error.id}`);
      heading.append(label,badge); row.append(heading); captions.push(() => { label.textContent = local(definition.name) || key; badge.textContent = t(definition.required ? 'required' : 'optional'); hint.textContent = local(definition.description); });
      if (type.kind === 'bool') { input.type = 'checkbox'; input.className = 'property-toggle'; input.setAttribute('role','switch'); }
      if (['int','float'].includes(type.kind)) { input.type = 'number'; input.step = type.kind === 'int' ? '1' : 'any'; if (type.minimum != null) input.min = type.minimum; if (type.maximum != null) input.max = type.maximum; }
      if (type.kind === 'string' && type.choices) type.choices.forEach(value => { const option = element('option',null,value); option.value = value; input.append(option); });
      if (input.tagName === 'TEXTAREA') { input.rows = type.kind === 'json' ? 4 : 3; input.spellcheck = type.kind !== 'json'; }
      const enabled = element('input'); enabled.type = 'checkbox'; enabled.checked = own(data.properties,key);
      const setLabel = element('label','property-enable'), setText = element('span'); setLabel.append(enabled,setText);
      captions.push(() => setText.textContent = t('enabled'));
      if (!definition.required) row.append(setLabel);
      const controls = element('div','property-value'); controls.append(input); row.append(controls);
      let slider;
      if (['int','float'].includes(type.kind) && Number.isFinite(type.minimum) && Number.isFinite(type.maximum) && type.maximum > type.minimum) {
        slider = element('input'); slider.type = 'range'; slider.min = type.minimum; slider.max = type.maximum;
        slider.step = type.kind === 'int' ? '1' : 'any';
        captions.push(() => slider.setAttribute('aria-label',local(definition.name) || key));
        controls.prepend(slider);
        const range = element('small','property-hint'); captions.push(() => range.textContent = `${t('range')}: ${type.minimum} – ${type.maximum}`); row.append(range);
        slider.oninput = () => { input.value = slider.value; input.dispatchEvent(new Event('input')); };
      }
      function setValue(value) {
        if (type.kind === 'bool') input.checked = value === true;
        else input.value = type.kind === 'json' ? JSON.stringify(value ?? null,null,2) : value ?? '';
        if (slider) slider.value = input.value;
      }
      function availability() { input.disabled = !enabled.checked; if (slider) slider.disabled = !enabled.checked; }
      const initial = own(data.properties,key) ? data.properties[key] : definition.default;
      setValue(initial); availability();
      enabled.onchange = () => { availability(); markDirty(); };
      input.oninput = () => { if (slider) slider.value = input.value; error.textContent = ''; input.removeAttribute('aria-invalid'); markDirty(); };
      if (definition.default != null) {
        const reset = element('button','property-reset'); reset.type = 'button';
        reset.textContent = '↺'; captions.push(() => { reset.title = t('reset'); reset.setAttribute('aria-label', `${local(definition.name) || key}: ${t('reset')}`); });
        reset.onclick = () => { enabled.checked = true; setValue(definition.default); availability(); markDirty(); }; heading.append(reset);
      }
      row.append(hint,error); $('propertyFields').append(row);
      fields.push({key,definition,input,enabled,error});
    });
    message = stale ? 'stale' : fields.length ? 'ready' : 'none'; translate();
  }
  function read(field) {
    const { definition, input, enabled } = field, type = definition.value_type;
    if (!enabled.checked && !definition.required) return undefined;
    let value;
    if (type.kind === 'bool') value = input.checked;
    else if (['int','float'].includes(type.kind)) {
      value = input.value.trim() === '' ? NaN : Number(input.value);
      if (!Number.isFinite(value) || (type.kind === 'int' && !Number.isSafeInteger(value)) || (type.minimum != null && value < type.minimum) || (type.maximum != null && value > type.maximum)) throw type.kind === 'int' ? 'integer' : 'number';
    } else if (type.kind === 'string') {
      value = input.value; const length = [...value].length;
      if (length < type.min_length || (type.max_length != null && length > type.max_length) || (type.choices && !type.choices.includes(value))) throw 'string';
    } else { try { value = JSON.parse(input.value); } catch { throw 'json'; } }
    return value;
  }
  function hasUnsafeInteger(value) {
    if (typeof value === 'number') return Number.isInteger(value) && !Number.isSafeInteger(value);
    if (value && typeof value === 'object') return Object.values(value).some(hasUnsafeInteger);
    return false;
  }
  async function load() {
    const id = selectedId(); if (!id || busy) return;
    busy = true; translate();
    try { const current = await send({kind:'summary'}); build(await invoke('node_properties',{id,expectedRevision:current.revision})); }
    catch (error) { message = error.code || 'failed'; }
    finally { busy = false; translate(); }
  }
  async function save(event) {
    event.preventDefault(); if (!snapshot || busy || stale || !dirty) return;
    const commands = []; let invalid = false;
    for (const field of fields) {
      try {
        const value = read(field), original = snapshot.properties[field.key];
        field.error.textContent = ''; field.input.removeAttribute('aria-invalid');
        if (JSON.stringify(value) === JSON.stringify(original) && own(snapshot.properties,field.key) === (value !== undefined)) continue;
        if (value === null) throw 'json';
        if (field.definition.value_type.kind === 'json' && hasUnsafeInteger(value)) throw 'unsafe';
        commands.push({kind:'set_property',id:snapshot.id,key:field.key,value:value === undefined ? null : value});
      } catch (code) { field.error.textContent = t(code); field.input.setAttribute('aria-invalid','true'); invalid = true; }
    }
    if (invalid) { message = 'invalid'; translate(); fields.find(f => f.input.getAttribute('aria-invalid') === 'true').input.focus(); return; }
    if (!commands.length) { dirty = false; message = 'ready'; translate(); return; }
    busy = true; translate();
    try {
      const result = await send({kind:'apply',expected_revision:snapshot.revision,command:{kind:'batch',commands}});
      // The accepted values are the new baseline. A later concurrent edit remains a conflict.
      for (const command of commands) { if (command.value === null) delete snapshot.properties[command.key]; else Object.defineProperty(snapshot.properties,command.key,{value:command.value,enumerable:true,configurable:true,writable:true}); }
      snapshot.revision = result.revision; dirty = false; stale = latestRevision > result.revision; message = stale ? 'stale' : 'saved';
    } catch (error) { message = error.code || 'failed'; if (error.code === 'revision_conflict') stale = true; }
    finally { busy = false; translate(); }
  }
  $('loadProperties').onclick = load;
  $('propertyForm').onsubmit = save;
  translate();
  return { translate, observe(revision) { latestRevision = Math.max(latestRevision, revision); if (snapshot && revision > snapshot.revision) { stale = true; message = 'stale'; translate(); } } };
}
