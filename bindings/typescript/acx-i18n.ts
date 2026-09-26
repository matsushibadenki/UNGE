import type { Locale } from './i18n';
const en = {
  invalid_request: 'The ACX request is invalid.', invalid_graph: 'The graph is invalid.',
  limit_exceeded: 'The request exceeds a host limit.', capacity_exceeded: 'The provider session is full.',
  policy_denied: 'The host policy does not permit this operation.', unknown_node_type: 'The node type is not registered.',
  document_mismatch: 'The document does not match.', revision_conflict: 'The document changed. Create a new preflight.',
  digest_mismatch: 'The approved request or preflight changed.', unauthorized: 'The authorization does not match this operation.',
  already_committed: 'This preflight was already committed.', expired: 'The preflight expired.',
  not_found: 'The requested item was not found.', unknown_method: 'The ACX method is not supported.',
  host_unavailable: 'The Rust document service is unavailable.', recovery_unavailable: 'This edit cannot be recovered.',
  execution_uncertain: 'Execution already started. Check its outcome before retrying.',
  execution_failed: 'Node execution failed.', invalid_policy: 'The host policy is invalid.',
} as const;
type Translations = { [K in keyof typeof en]: string };
const ja: Translations = {
  invalid_request: 'ACXリクエストが無効です。', invalid_graph: 'グラフが無効です。',
  limit_exceeded: 'ホストが定める上限を超えています。', capacity_exceeded: 'Providerのセッション保持数が上限に達しました。',
  policy_denied: 'ホストのポリシーで許可されていない操作です。', unknown_node_type: 'ノード型が登録されていません。',
  document_mismatch: '対象のドキュメントが一致しません。', revision_conflict: 'ドキュメントが更新されました。事前確認をやり直してください。',
  digest_mismatch: '承認対象の入力または事前確認が変更されています。', unauthorized: 'この操作に対応する承認がありません。',
  already_committed: 'この事前確認は確定済みです。', expired: '事前確認の期限が切れています。',
  not_found: '対象が見つかりません。', unknown_method: '未対応のACXメソッドです。',
  host_unavailable: 'Rustのドキュメントサービスを利用できません。', recovery_unavailable: 'この編集は回復できません。',
  execution_uncertain: '実行は開始済みです。再試行する前に結果を確認してください。',
  execution_failed: 'ノードの実行に失敗しました。', invalid_policy: 'ホストのポリシーが無効です。',
};
const zh: Translations = {
  invalid_request: 'ACX请求无效。', invalid_graph: '节点图无效。',
  limit_exceeded: '请求超出宿主限制。', capacity_exceeded: 'Provider会话保留数量已达上限。',
  policy_denied: '宿主策略不允许此操作。', unknown_node_type: '节点类型尚未注册。',
  document_mismatch: '目标文档不匹配。', revision_conflict: '文档已更新，请重新预检。',
  digest_mismatch: '已批准的输入或预检内容已改变。', unauthorized: '此操作没有匹配的授权。',
  already_committed: '此预检已提交。', expired: '预检已过期。',
  not_found: '未找到目标。', unknown_method: '不支持此ACX方法。',
  host_unavailable: 'Rust文档服务不可用。', recovery_unavailable: '无法恢复此编辑。',
  execution_uncertain: '执行已开始，请先核实结果再重试。',
  execution_failed: '节点执行失败。', invalid_policy: '宿主策略无效。',
};
export const acxMessages = { en, ja, 'zh-CN': zh };
export function acxErrorMessage(locale: Locale, code: string): string {
  const table: Record<string, string> = acxMessages[locale];
  return table[code] ?? code;
}
