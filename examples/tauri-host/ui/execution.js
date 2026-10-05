/* UI keeps only small run metadata. Rust owns the execution snapshot and results. */
function createExecutionUI({ invoke, listen, locale, revision }) {
  const el = id => document.getElementById(id);
  const words = {
    en: { revision:'revision', label:'Execution', run:'Run graph', cancel:'Cancel', idle:'Ready to run', queued:'Waiting to start', running:'Running', finished:'Finished', failed:'Execution failed', cancelled:'Cancelled', deadline:'Deadline exceeded', stale:'Graph edited since this run started', cancelling:'Cancellation requested', unavailable:'Unable to refresh execution status', invalid_graph:'Connect all required inputs before running.', execution_busy:'Another execution is active.', revision_conflict:'The graph changed. Refresh before running.', execution_unavailable:'Execution is not configured.', unknown_run:'This execution record is no longer retained.', unknown_view:'This window is not registered.' },
    ja: { revision:'リビジョン', label:'実行', run:'グラフを実行', cancel:'キャンセル', idle:'実行できます', queued:'開始待ち', running:'実行中', finished:'実行終了', failed:'実行に失敗しました', cancelled:'キャンセル済み', deadline:'実行期限を超過', stale:'実行開始後にグラフが更新されました', cancelling:'キャンセルを要求しました', unavailable:'実行状態を取得できません', invalid_graph:'必須入力を接続してから実行してください。', execution_busy:'別の実行が進行中です。', revision_conflict:'グラフが更新されました。再取得してから実行してください。', execution_unavailable:'実行サービスが未設定です。', unknown_run:'実行の記録は保持されていません。', unknown_view:'このウインドウは登録されていません。' },
    'zh-CN': { revision:'版本', label:'执行', run:'执行节点图', cancel:'取消', idle:'可以执行', queued:'等待开始', running:'执行中', finished:'执行结束', failed:'执行失败', cancelled:'已取消', deadline:'执行超时', stale:'执行开始后节点图已更新', cancelling:'已请求取消', unavailable:'无法获取执行状态', invalid_graph:'请先连接所有必填输入。', execution_busy:'另一个执行正在进行。', revision_conflict:'节点图已更新，请刷新后执行。', execution_unavailable:'尚未配置执行服务。', unknown_run:'执行记录已不再保留。', unknown_view:'此窗口尚未注册。' },
  };
  let generation = 0;
  let current = null, starting = false, cancelling = false, error = null, refreshing = false;
  const active = () => current && ['queued','running'].includes(current.state);
  function render() {
    const w = words[locale()];
    el('executionLabel').textContent = w.label;
    el('run').textContent = w.run;
    el('cancelRun').textContent = w.cancel;
    el('run').disabled = starting || !!active();
    el('cancelRun').disabled = !active() || cancelling || current.cancel_requested;
    el('runProgress').max = Math.max(1, current?.total ?? 0);
    el('runProgress').value = current?.finished ?? 0;
    el('runProgress').setAttribute('aria-label', w.label);
    let message = error ? (w[error] ?? w.failed) : w.idle;
    if (!error && current) {
      message = current.state === 'failed' || current.failed || current.blocked ? w.failed
        : current.reason === 'deadline_exceeded' ? w.deadline
        : current.reason === 'cancelled' ? w.cancelled
        : current.cancel_requested ? w.cancelling : w[current.state];
      message += ` · ${current.finished}/${current.total} · ${w.revision} ${current.revision}`;
      if (current.revision !== revision()) message += `\n${w.stale}`;
    }
    el('runStatus').textContent = message;
  }
  function accept(next) {
    if (!next) return;
    if (current?.id === next.id && next.sequence <= current.sequence) return;
    // A retained terminal record cannot replace a newer active execution.
    if (current?.id !== next.id && active() && next.state === 'finished') return;
    if (current?.id !== next.id) generation++;
    current = next; error = null; render();
  }
  async function refresh() {
    if (refreshing || starting) return;
    refreshing = true;
    const before = generation;
    try {
      const shared = await invoke('current_execution');
      if (generation !== before) return;
      if (shared) accept(shared);
      else if (active()) {
        const latest = await invoke('execution_status', { id: current.id });
        if (generation !== before) return;
        accept(latest);
      }
      if (error === 'unavailable') { error = null; render(); }
    } catch (e) { console.error(e); error = e.code === 'unknown_run' ? e.code : 'unavailable'; if (e.code === 'unknown_run') current = null; render(); }
    finally { refreshing = false; }
  }
  el('run').onclick = async () => {
    if (starting || active()) return;
    starting = true; error = null; render();
    try {
      const queued = await invoke('start_execution', { expectedRevision: revision() });
      accept(queued);
      accept(await invoke('execution_status', { id: queued.id }));
    } catch (e) { console.error(e); error = e.code ?? 'failed'; }
    finally { starting = false; render(); await refresh(); }
  };
  el('cancelRun').onclick = async () => {
    if (!active() || cancelling) return;
    cancelling = true; render();
    try { accept(await invoke('cancel_execution', { id: current.id })); }
    catch (e) { console.error(e); error = e.code ?? 'failed'; }
    finally { cancelling = false; render(); }
  };
  // Poll latest Rust state even when an advisory event was lost or subscription failed.
  listen('unge://execution', ({payload}) => accept(payload)).catch(e => console.error(e));
  listen('unge://changed', () => render()).catch(e => console.error(e));
  const timer = setInterval(refresh, 1000);
  window.addEventListener('focus', refresh);
  window.addEventListener('pagehide', () => clearInterval(timer), {once:true});
  refresh(); render();
  return { translate: render };
}
