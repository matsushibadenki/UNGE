/* Host-only panel operations. The same WebView moves, preserving form drafts. */
function createPanelControls({ invoke, listen, locale }) {
  const toolbar = document.getElementById('panelToolbar');
  const button = document.getElementById('panelToggle');
  const status = document.getElementById('panelStatus');
  const messages = {
    en: { detach:'Float settings panel', dock:'Dock settings panel', working:'Moving panel…', failed:'Unable to move the panel. Try again.', busy:'The panel is already moving.', unavailable:'Panel docking is unavailable in this mode.' },
    ja: { detach:'設定パネルを切り離す', dock:'設定パネルをドッキング', working:'パネルを移動中…', failed:'パネルを移動できませんでした。再度お試しください。', busy:'パネルを移動中です。', unavailable:'この表示モードではドッキングを利用できません。' },
    'zh-CN': { detach:'浮动设置面板', dock:'停靠设置面板', working:'正在移动面板…', failed:'无法移动面板，请重试。', busy:'面板正在移动。', unavailable:'此显示模式不支持停靠。' }
  };
  let layout = { supported:false, floating:false }, busy = false, error = null;
  function translate() {
    const text = messages[locale()];
    toolbar.hidden = !layout.supported;
    button.textContent = text[layout.floating ? 'dock' : 'detach'];
    button.disabled = busy || !layout.supported;
    status.textContent = busy ? text.working : error ? text[error] : '';
  }
  async function refresh() { layout = await invoke('panel_layout'); translate(); }
  button.onclick = async () => {
    if (busy) return;
    busy = true; error = null; translate();
    try { layout = await invoke('set_panel_floating', { floating: !layout.floating }); }
    catch (e) {
      error = e.code === 'panel_busy' ? 'busy' : e.code === 'panel_unavailable' ? 'unavailable' : 'failed';
      // A native move may succeed before a later focus/resize call fails.
      await refresh().catch(() => {});
    } finally { busy = false; translate(); }
  };
  // Register first so native close-to-dock cannot be lost during initial loading.
  listen('unge://panel-layout', ({ payload }) => { layout = payload; translate(); })
    .then(refresh).catch(() => { error = 'failed'; translate(); });
  return { translate };
}
if (typeof module !== 'undefined') module.exports = { createPanelControls };
