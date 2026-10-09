/** Sample host commands; not part of the portable Engine IPC contract. */
export interface PanelLayout { supported: boolean; floating: boolean }
export interface PanelCommandMap {
  panel_layout: { args: Record<string, never>; result: PanelLayout };
  set_panel_floating: { args: { floating: boolean }; result: PanelLayout };
}
export interface PanelError { code: 'unknown_view' | 'panel_busy' | 'panel_unavailable' | 'panel_error'; message: string }
export interface PanelControlsOptions {
  invoke(command: 'panel_layout'): Promise<PanelLayout>;
  invoke(command: 'set_panel_floating', args: { floating: boolean }): Promise<PanelLayout>;
  listen(event: 'unge://panel-layout', callback: (event: { payload: PanelLayout }) => void): Promise<() => void>;
  locale(): 'en' | 'ja' | 'zh-CN';
}
export function createPanelControls(options: PanelControlsOptions): { translate(): void };
