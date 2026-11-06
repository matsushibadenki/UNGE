# Persistent storage counters and profiling / 永続化の保持量と計測 / 持久化用量与计测

## 日本語

### 保持量API

`PersistentHost::storage_usage()`、`PersistentEditor::storage_usage()`、`Engine::storage_usage()`（acx feature）で、保存済み状態ログの使用量を照会します。Engineの戻り値が`None`ならメモリー内モードです。保存障害は`None`に置き換えず、既存のstorageエラーを返します。

```rust,ignore
if let Some(usage) = engine.storage_usage()? {
    // ホストの管理画面や監視へ渡す。通知のしきい値はホストが決める。
    println!("{} / {} bytes, {} records, revision {}",
        usage.bytes, usage.max_bytes, usage.records, usage.revision);
}
```

`StateLogUsage`はRustホスト専用の小さな値型です。新しいIPCやSchema型ではありません。

| フィールド | 意味 |
|---|---|
| bytes / max_bytes | 成功して同期・公開した状態ログのバイト数／設定上限 |
| records | header・編集・Undo/Redo・Receipt prefix確認を含む保持レコード数 |
| revision | 公開済みEditorのrevision |
| undo_steps / redo_steps | 現在の履歴件数 |
| history_bytes | coreが数えるシリアライズ後の保持命令量 |

照会は同じmutex内の数値を読むだけで、ログ追記や全Document複製を行いません。編集前の予約APIではなく、照会後に別の編集が容量を使うことがあります。実際の容量検査は追記時に維持します。ディスクの空き容量・Receiptログの使用量・総ヒープ／RSS／GPUメモリーは含みません。Receiptログは既存の`Provider::journal_usage()`で別に照会します。Undoでは保持する命令が変わるため、history_bytesも変わる場合があります。

### 再現できるベンチマーク

```sh
cargo run --release --locked -p unge-acx --example persistence_benchmark -- 30 > persistence.json
```

引数は条件ごとの成功サンプル数（3〜100、既定30）。releaseを必須とし、100／1,000／10,000ノード、事前の履歴0／128ステップ、移動／プロパティ編集の12条件を実行します。信頼済みmath Registryに登録したmath.number、接続0・Group0・外部資産なしの合成データです。Command構築後に計時し、編集の実行順は交互、Undo／Redoはメモリー内→永続化の順です。

各条件で5回予熱し、編集・Undo・Redoをメモリー内EditorとPersistentEditorで比較します。永続化には候補Editor／Document／履歴のコピー、検証、JSON/hash、追記、sync_all、公開を含みます。各操作後にDocument・revision・履歴件数／保持量・全体検証・Registry検証を照合します。最後のUndoは復元確認用で計時対象外です。

復元後のRedo／Undoも照合した後、同じログを5回開いて再生と同期を計時します。全ログがOSキャッシュに載り得る条件で、cold boot／キャッシュ破棄後の復元ではありません。復元時間にはseedのコピーも含み、seedファイルの読込は含みません。初期作成時間は別に1回だけ記録します。中央値は整列後の上側中央、p95は最近順位法です。5回の復元のp95は最大標本値であり、SLOの認定には使いません。ログの使用量と実ファイル長も照合します。一時ログは正常終了時に削除し、生データだけをstdoutへ出力します。

**範囲**：Editor保存経路の比較です。Engineのロック競合／SceneIndex、WebView、Providerの開始guard／Receipt保存と相互確認、GPU／IPC、外部副作用、巨大プロパティ、接続やGroupの多いGraphを含みません。history_bytesは総メモリー使用量ではありません。圧縮による改善やUI遅延をこの結果から認定しません。

実測と検証範囲は [PERFORMANCE.md](PERFORMANCE.md) を参照してください。保存の取り込みは [TAURI_PERSISTENCE.md](TAURI_PERSISTENCE.md)、[ACX_EDITOR_PERSISTENCE.md](ACX_EDITOR_PERSISTENCE.md) を参照してください。

## English

`PersistentHost::storage_usage()`, `PersistentEditor::storage_usage()` and `Engine::storage_usage()` (acx feature) return Rust-only `StateLogUsage` counters. Engine returns None only for memory mode; storage failure stays an error. Bytes include all successfully synced state-log frames and their header. Records include edits/history operations and replay acknowledgments, so records are not the revision. Undo/redo steps and history_bytes describe core's retained serialized-command budget, not heap/RSS/GPU memory. Undo may change that byte count. Counters read under the same lock without cloning Document or appending. They are a point-in-time observation, not a capacity reservation. Disk-free space and replay-journal bytes are separate; use Provider::journal_usage for the latter.

The release command above runs 12 synthetic cases: 100/1,000/10,000 math.number nodes, zero edges/groups/resources, zero/128 priming history steps, move/property edits. After five warmups, collect 3–100 samples (default 30) for memory versus durable edit/undo/redo. Edit order alternates; undo/redo order is fixed memory-first. Timed durable operations include candidate cloning, validation, serialization/hash, append, sync_all and publication. Assertions outside timing compare Document, revision, history and full core/Registry validity after each transition. Reopened history is exercised, then five warm-cache reopen samples include replay/sync and seed cloning; seed-file reads are excluded. Initial creation is measured once separately. Median uses the upper middle sample; p95 uses nearest rank. For five reopens, p95 is the sample maximum, not an SLO certification. Temporary files are removed after the run.

This excludes Engine contention/SceneIndex, WebView, Provider guards/receipts/pair acknowledgments, GPU/IPC, external effects, large properties and edge/group-heavy graphs. It does not certify UI latency, cold recovery, total memory or compaction gains. See PERFORMANCE.md for actual results and the persistence guides for integration.

## 简体中文

`PersistentHost::storage_usage()`、`PersistentEditor::storage_usage()`及`Engine::storage_usage()`（acx feature）返回仅供Rust宿主使用的StateLogUsage。Engine只有内存模式返回None，存储故障仍返回错误。bytes包含成功同步的状态日志header和全部记录；records包含编辑、撤销／重做及重放确认，不等于revision。undo_steps／redo_steps与history_bytes是core保留的序列化命令预算，不是堆／RSS／GPU内存，撤销时字节数可能变化。照会在同一锁下读取标量，不复制Document或追加日志；不预留后续容量。磁盘剩余空间及Receipt日志另计，后者使用Provider::journal_usage。

上述release命令运行12组合成数据：100／1,000／10,000个math.number，0连接／Group／外部资源，预置0／128步历史，移动／属性编辑。5次预热后采集3〜100个样本（默认30），比较内存与持久化的编辑／撤销／重做。编辑次序交替，撤销／重做固定先内存。持久化计时包含候选复制、验证、序列化／hash、追加、sync_all和公开。每次转换后在计时外核对Document、revision、历史及core／Registry验证。重新打开后实际测试Redo／Undo，再采集5次热缓存恢复（含日志重放、同步、seed复制，不含seed文件读取）。中位数取排序后较高的中间样本，p95采用最近秩；5次重开的p95即样本最大值，不用于SLO认证。初始创建另计1次，临时日志在结束后删除。

不包含Engine竞争／SceneIndex、WebView、Provider开始guard／Receipt／相互确认、GPU／IPC、外部副作用、大属性或连接／Group密集图。不据此认证UI延迟、冷恢复、总内存或压缩收益。实测见PERFORMANCE.md，接入见持久化指南。
