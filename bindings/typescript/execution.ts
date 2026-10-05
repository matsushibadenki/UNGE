import type { Locale } from './index';

import type { Status as ExecutionStatus, StopReason as ExecutionStopReason, RunState } from './generated-output';
export type { Status as ExecutionStatus, StopReason as ExecutionStopReason, ProgressEvent, RunSummary, RunState } from './generated-output';

export const executionStatusMessages: Record<Locale, Record<ExecutionStatus, string>> = {
  en: { completed: 'Completed', cached: 'Cached', failed: 'Failed', blocked: 'Blocked by an upstream failure', cancelled: 'Cancelled' },
  ja: { completed: '完了', cached: 'キャッシュを使用', failed: '失敗', blocked: '上流の失敗により実行不可', cancelled: 'キャンセル' },
  'zh-cn': { completed: '已完成', cached: '使用缓存', failed: '失败', blocked: '上游失败导致无法执行', cancelled: '已取消' },
};
export const executionStopMessages: Record<Locale, Record<ExecutionStopReason, string>> = {
  en: { completed: 'Execution finished', cancelled: 'Execution cancelled', deadline_exceeded: 'Execution deadline exceeded' },
  ja: { completed: '実行終了', cancelled: '実行をキャンセル', deadline_exceeded: '実行期限を超過' },
  'zh-cn': { completed: '执行结束', cancelled: '执行已取消', deadline_exceeded: '执行超时' },
};

export const runStateMessages: Record<Locale, Record<RunState, string>> = {
  en: { queued: 'Queued', running: 'Running', finished: 'Execution finished', failed: 'Execution interrupted by a host error' },
  ja: { queued: '開始待ち', running: '実行中', finished: '実行終了', failed: 'ホストのエラーで実行中断' },
  'zh-cn': { queued: '等待开始', running: '执行中', finished: '执行结束', failed: '主机错误导致执行中断' },
};
