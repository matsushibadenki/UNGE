export type Locale = 'en' | 'ja' | 'zh-CN';
export const messages = {
  en: { revision_conflict: 'The document changed. Refresh before editing again.', invalid_command: 'This edit is invalid.', unknown_view: 'This window is not registered.', limit_exceeded: 'This request exceeds the supported limit.', state_unavailable: 'The document is unavailable.', missing_node: 'This node no longer exists.', invalid_selection: 'The selection is invalid.', renderer_unavailable: 'The renderer is unavailable.', gpu_error: 'Rendering failed.', worker_error: 'The operation failed.', invalid_request: 'The request is invalid.', invalid_node: 'Node metadata is invalid.' },
  ja: { revision_conflict: 'ドキュメントが更新されました。再読み込みしてから編集してください。', invalid_command: 'この編集は無効です。', unknown_view: 'このウインドウは登録されていません。', limit_exceeded: 'リクエストが上限を超えています。', state_unavailable: 'ドキュメントを利用できません。', missing_node: 'このノードは存在しません。', invalid_selection: '選択が無効です。', renderer_unavailable: 'レンダラーを利用できません。', gpu_error: '描画に失敗しました。', worker_error: '処理に失敗しました。', invalid_request: 'リクエストが無効です。', invalid_node: 'ノード情報が無効です。' },
  'zh-CN': { revision_conflict: '文档已更新，请刷新后再编辑。', invalid_command: '此编辑无效。', unknown_view: '此窗口尚未注册。', limit_exceeded: '请求超出限制。', state_unavailable: '文档不可用。', missing_node: '此节点已不存在。', invalid_selection: '选择无效。', renderer_unavailable: '渲染器不可用。', gpu_error: '渲染失败。', worker_error: '操作失败。', invalid_request: '请求无效。', invalid_node: '节点信息无效。' },
} as const;
export function errorMessage(locale: Locale, code: string): string {
  const table: Record<string, string> = messages[locale];
  return table[code] ?? code;
}
