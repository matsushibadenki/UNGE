# ノード移動の確定と検証

## 変更範囲

Editorは生成時にDocument全体を検証し、ノードの欠落したplacementを補完します。その後は外部へ可変Documentを公開せず、Commandだけで状態を更新します。

この不変条件を使い、`MoveNode` と、再帰的にMoveNodeだけを含むBatchでは、変更するノードの存在と矩形の妥当性を検査します。接続・型・循環・Group・保存versionは変更しないため、coreのDocument全体の再検証を省略します。空のBatchもこの範囲です。新しいキャッシュや公開の「検証を無効化する」フラグは追加しません。

- MoveNodeは位置だけでなく幅・高さも変更できます。既存の有限値・座標範囲・正のサイズの制約は維持します。
- 移動以外の命令が1つでも含まれれば、Batchの最終状態でDocument全体を検証します。途中で変更を相殺する命令も同様です。
- 独自の `DocumentValidator::validate` は、移動だけの場合も確定後の状態に対して従来どおり実行します。位置依存のホストルールを省略しません。Registryの検証も維持します。
- 適用途中や最終検証で失敗した場合は逆命令で戻し、revision・Undo／Redo・履歴容量を変えません。
- Undo／Redoも同じ判定を使います。移動だけの逆命令は対象矩形を検査し、ホスト検証を通ってから履歴を移します。
- JSON読込／保存とEditor生成時の全体検証は維持します。UI・ポインター・ACXは同じEditorの改善を利用し、IPCの変更はありません。

## 確定処理を測る

```rust,ignore
let (revision, sample) = editor.execute_timed(command)?;
println!("apply={} validation={} host={} history={} total={} ms",
    sample.apply_ms, sample.core_validation_ms,
    sample.host_validation_ms, sample.history_ms, sample.total_ms);
```

`execute_timed` は通常のexecuteと同じ編集を行い、成功した場合だけrevisionと `EditTimings` を返します。失敗時はErrで、部分的な計測結果は返しません。計測はRust内で完結します。通常のexecute／Undo／Redoでは時計を取得しません。

| フィールド | 範囲 |
|---|---|
| apply_ms | revisionの上限確認、命令の分類、適用、逆命令の作成、対象矩形の検査 |
| core_validation_ms | coreの最終状態検証。移動だけなら全体検証を行わない分岐と計時の費用 |
| host_validation_ms | 任意のDocumentValidatorの呼び出し。未登録なら分岐と計時のみ |
| history_ms | Redoの破棄、逆命令のJSONバイト計量、上限による古い履歴の破棄、Undoへの記録 |
| total_ms | execute_timedの開始から履歴更新終了まで |
| full_document_validation | この確定でcoreのDocument全体を検証したか。ホスト検証の有無を表す値ではない |

SceneChangesの作成、Index更新、Scene生成、共有Engineのロック待ち、IPC、GPUはtotal_msに含みません。サンプルの保持・集計はホストが有界に管理してください。微小な時間は時計の分解能と計測自体の費用に左右されます。

```sh
cargo run --release --locked -p unge-render --example edit_benchmark
```

10,000ノード／30,000接続で1ノードと100ノードの移動Batchを計測します。各条件5回の予熱と30回の計測。16MiB／64ステップの履歴を使用し、毎回Undoで元のDocumentに戻して照合します。全体検証を測定区間外の検証手段として使います。Index更新と通常Undoも別に測定します。ホスト検証を登録していないため、Registryを含むアプリの確定時間そのものではありません。

Registryの `validate_edit` は依然としてGraphの全体検証とDefinition照合を行います。独自検証が大きな費用を持つホストでは、coreの改善後もその費用が残ります。多数ノードの移動では、関連EdgeがIndex差分の上限を超え、全体再構築の費用が残る場合もあります。[実測と制限](PERFORMANCE.md)。

## English

Editor fully validates and normalizes the loaded Document, retains exclusive mutation through Command, and exposes only immutable access. MoveNode and recursively move-only batches preserve graph/schema invariants. They check each target node and rectangle during application, so core skips the redundant whole-Document validation. Empty batches qualify; any other command retains full final-state validation. Loading, saving and Editor construction still perform full validation.

Host DocumentValidator callbacks, including Registry validation, always run as before. Position-dependent rules, atomic rollback, revision checks and history accounting remain intact. Undo/redo use the same classification on their inverse commands. There is no public validation-disable flag or IPC change.

`execute_timed` returns `(revision, EditTimings)` only on success, separating application/local checks, core validation, host validation and history costs. Normal execute/undo/redo do not read the clock. `full_document_validation` describes core validation only. Index/scene/GPU work, locks and IPC are excluded. Run `cargo run --release --locked -p unge-render --example edit_benchmark` for five warm-ups and 30 samples of 1-node and 100-node move batches on 10,000 nodes / 30,000 edges. Undo restores and verifies the document outside execute timings. This fixture has no host validator; Registry's full graph/definition validation and large Index rebuild costs remain in real hosts.

## 简体中文

Editor在创建时完整验证并规范化Document，仅通过Command修改状态，对外提供不可变访问。MoveNode及递归嵌套的纯移动Batch保持图和版本约束不变。应用命令时检查目标节点及矩形，因此省略core重复的Document全量验证。空Batch也适用；只要包含其他命令，仍在最终状态执行全量验证。加载、保存及Editor创建时的完整验证不变。

宿主DocumentValidator（包括Registry）仍按原规则执行。位置相关规则、原子回滚、revision及历史容量保证保持不变。Undo／Redo对逆命令使用相同分类。不增加公开的关闭验证开关，也不改变IPC。

`execute_timed` 仅成功时返回 `(revision, EditTimings)`，分别记录应用／局部检查、core验证、宿主验证及历史成本。普通execute／Undo／Redo不读取时钟。`full_document_validation` 仅描述core是否全量验证，不代表宿主检查。Index／场景／GPU、锁等待和IPC不计入。通过 `cargo run --release --locked -p unge-render --example edit_benchmark` 可在10,000节点／30,000连线上计测1节点与100节点的移动Batch，每组预热5次后计测30次。每次Undo还原并验证文档。这一数据集未注册宿主验证器；实际宿主中Registry的完整图／Definition验证以及大范围Index重建的成本仍然存在。
