# ACXからUNGEを操作する

[English](#english) · [日本語](#日本語) · [简体中文](#简体中文)

## 日本語

`unge-acx` がACX 0.1のManifest・Preflight・Policy承認・Commit・Execute・Receipt・Recoverを実装します。
AIからはJSON Linesの標準入出力で操作します。対象のノードグラフはRustの共有Documentに保持されます。

### すぐに試す

UNGEルートで実行します。Python 3.10以上が必要です。AI APIキーは不要です。

```sh
cargo build --locked -p unge-acx-provider
python3 examples/acx-provider/agent.py
```

PythonのクライアントがRustプロセスを起動し、次の操作を実行して終了します。

1. 能力とノード定義を取得する。
2. 20・22・加算の3ノードと2本の接続を事前確認する。この時点でDocumentは変わらない。
3. ホストのポリシー承認を取得して確定し、1つのCommand Batchとして適用する。
4. 指定リビジョンのグラフを実行し、42を確認する。
5. Python側でReceiptと正確なJSONバイト列のSHA-256を照合する。
6. Recoverで元のグラフに戻す。再送しても二重にUndoしない。

### Tauriの画面と同じグラフを操作する

```sh
cargo build --locked -p unge-tauri-host
python3 examples/acx-provider/agent.py --binary target/debug/unge-tauri-host --desktop
```

TauriのGPUウインドウを起動して同じ操作を行います。初期の12ノードを保持したまま3ノードを追加し、最後に元の12ノードへ戻ります。
テストクライアント終了時にアプリも終了します。Windowsでは実行ファイル名に `.exe` を付けてください。

AI側で常時接続する場合は `target/debug/unge-tauri-host --acx-stdio` を子プロセスとして起動し、そのstdin/stdoutを保持します。
通常起動ではACXの入力スレッドを開きません。既に起動した別プロセスへ接続する方式ではありません。

### 最小リクエスト

```json
{"id":"1","method":"discover","params":{}}
{"id":"2","method":"observe","params":{"query":"summary"}}
{"id":"3","method":"observe","params":{"query":"definitions","limit":20}}
```

`summary` の `documentId` と `revision` を使って、次のintentを `preflight.params.input` に渡します。
UUIDは例の値を固定利用せず、作成する要素ごとに新しく発行してください。

```json
{
  "kind": "edit",
  "document_id": "<summary.documentId>",
  "expected_revision": 0,
  "operations": [{
    "kind": "create_node",
    "id": "<new UUID>",
    "type_id": "math.number",
    "properties": {"value": 42},
    "rect": {"x": 40, "y": 40, "width": 180, "height": 90}
  }]
}
```

その後は `authorize → commit → execute → receipt` を順に呼びます。
完全な通信例は `examples/acx-provider/agent.py`、各フィールドは[Node Graph Profile](ACX_NODE_GRAPH_PROFILE.md)を参照してください。

### ホストへ取り込む

```toml
[dependencies]
unge-acx = { path = "../vendor/unge/crates/unge-acx" }
unge-tauri = { path = "../vendor/unge/crates/unge-tauri", features = ["acx"] }
```

```rust,ignore
let registry = Arc::new(registry);
let editor = unge_core::Editor::new(document, 256)?.with_validator(registry.clone())?;
let engine = unge_tauri::Engine::from_editor(editor);
let provider = unge_acx::Provider::new(
    Arc::new(engine.clone()), // 画面が使う同じEngine
    registry,
    unge_acx::Policy::math_demo(), // 例。製品ではホストが許可範囲を決める
);
```

`GraphHost` はsnapshot・同一ロック内のrevision検査とapply・条件付きundoの3メソッドです。
TauriのEngineは `acx` featureでこのtraitを実装します。GUI不要なら `MemoryHost` を使います。
`Provider::dispatch` は同期APIです。Tauriの描画スレッドではなくworkerで実行します。
`serve` の変更通知コールバックから `unge://changed` を配信する統合例をTauriサンプルに含めています。

デフォルトPolicyは読み取り専用です。編集には `allow_edit` と作成可能type ID、実行には `allow_run` と実行可能type IDをホスト側で設定します。
このProfileの実行対象はpureとして登録した信頼済みノードに限定します。pureフラグだけでは許可せず、型の許可リストも照合します。
画像やTensorの処理に拡張する場合も、データはホストのResourceStoreに保持し、ACXには参照IDを渡します。

TypeScriptクライアントは `bindings/typescript/acx.ts` です。pipe送受信関数を注入します。
通常のWebView向け `dispatch` 命令とは別の契約です。クライアントは承認や編集の再試行を自動では行いません。
エラー翻訳は `acx-i18n.ts` に英語・日本語・简体中文を揃えています。

### 仕様と運用の境界

- ACX側に `docs/NODE-GRAPH-PROFILE.md`、Intent Schema、入力例、テストを追加しました。既存のManifest/Receipt schemaは変更していません。
- コピーして使えるよう、参照SchemaとProfileはUNGE側にも同梱しました。ビルド・実行に `acx` シンボリックリンクは不要です。
- このbindingは実験段階のACX JSON Lines契約です。MCPのwire protocolや自動ツール登録は実装していません。
- 承認・Receipt・再送記録はメモリ上です。Providerの再起動をまたぐ永続保証はありません。
- 既定では120秒、128 preflight、編集256操作、実行256ノード、グラフ10000ノードです。件数上限に達したProviderは新しいpreflightを拒否し、既存Receiptを保護します。
- Recoverは対象編集後のrevisionが変わっていない場合だけ許可します。途中の人間の操作を誤ってUndoしません。
- 署名・多ユーザー認証・課金・外部副作用の標準Backendは今後の機能です。

## English

`unge-acx` exposes graph discovery, bounded observations, transactional editing, revision-bound execution and conditional undo through ACX 0.1. Run the two commands in the first example to launch the Rust provider from an independent Python agent. It creates/connects three arithmetic nodes, computes 42, verifies receipt hashes and restores the original graph.

Use the desktop command to operate the same Engine that the Tauri UI uses. `--acx-stdio` opts into a dedicated parent-owned pipe; closing it ends the sample process. There is no network listener and no automatic MCP registration.

For embedding, use `MemoryHost` or enable `unge-tauri`'s `acx` feature and pass the shared Engine to `Provider::new`. Policy is host-owned and read-only by default. Creation/execution types are allowlisted; execution also requires trusted pure definitions. Run the synchronous provider on a worker thread and forward its change callback to the UI.

The [profile](ACX_NODE_GRAPH_PROFILE.md) specifies exact messages, digest byte strings, rejection rules, expiry and recovery. Schema snapshots are bundled, so the optional `acx` symlink is not needed in a copied project. State and unsigned receipts are in memory, with no restart persistence or durable exactly-once guarantee.

## 简体中文

`unge-acx` 通过ACX 0.1提供能力发现、分页查询、事务编辑、绑定版本的执行和有条件的撤销。运行第一组命令，Python代理会启动Rust Provider，创建并连接三个算术节点，得到42，验证回执摘要，然后恢复原图。

桌面命令操作与Tauri界面完全相同的Engine。`--acx-stdio` 显式启用父进程管理的专用管道；关闭管道会结束示例进程。不会开放网络端口，也不会自动注册MCP工具。

集成时使用MemoryHost，或开启 `unge-tauri` 的 `acx` feature并传入共享Engine。策略由宿主设置，默认只读；创建和执行需匹配类型白名单，执行还要求可信pure定义。在worker线程运行同步Provider，并将变化通知转发给界面。

完整消息、摘要字节格式、拒绝规则、有效期和恢复条件见[Profile](ACX_NODE_GRAPH_PROFILE.md)。Schema快照已随UNGE提供，复制项目后无需 `acx` 符号链接。状态和未签名回执仅在内存中，没有跨重启持久化保证。

## 編集制約 / Editing constraints / 编辑约束

`observe(query: "definitions")` は `property_schema` を返します。AIは制約・初期値を参照でき、不正なプロパティはPreflightで `invalid_properties` として拒否されます。必須入力の接続は実行前に検証します。共有Editorの検証と履歴容量の設定は [PROPERTY_VALIDATION.md](PROPERTY_VALIDATION.md) を参照してください。

Definition observations include property constraints and defaults. Invalid properties fail preflight with `invalid_properties`; required connections are checked before execution. Use a validated shared Editor to enforce the same rules for UI edits. History eviction can make recovery unavailable.

定义查询包含属性约束与初始值。无效属性在预检时以 `invalid_properties` 拒绝，必填连接在执行前检查。请为共享Editor配置验证器，使界面与AI遵循相同规则。历史被移除后可能无法恢复。

## 共有実行サービス / Shared execution service / 共享执行服务

TauriとACXに同じRunServiceとRegistry Arcを渡すと、承認済みrunの二重実行を防ぎ、ホストへRunSummaryを通知できます。既存execute/Receiptは同期形式を維持し、同じcommitの再送で再実行しません。busy解消後の実行も新しいpreflight/承認/commitで行います。同期execute中の照会には非同期job拡張を使います。設定は [EXECUTION_SERVICE.md](EXECUTION_SERVICE.md)。

Share RunService and the exact Registry Arc with Tauri/Provider to avoid overlapping UI/AI runs and notify a host observer. Existing synchronous execute/Receipt and replay contracts are preserved. A busy failure requires a new approved commit; it is not retried automatically. Use the optional async job methods for queries/cancellation instead of blocking execute. See [execution service](EXECUTION_SERVICE.md#english).

Tauri与Provider共享RunService及同一Registry Arc可防止UI/AI同时重复执行，并通知主机。既有同步execute/Receipt及重放约定保持不变。busy失败后的执行需要重新获批的commit，不会自动重试。执行期间的AI查询与取消请使用可选异步任务方法。详见[共享服务](EXECUTION_SERVICE.md#简体中文)。

## 非同期job拡張 / Async jobs / 异步任务

[ACX_ASYNC_JOBS.md](ACX_ASYNC_JOBS.md) の任意拡張に対応しました。発見情報のasyncJobsを確認し、承認済みrun commitをrun_startで開始、run_statusで照会、run_cancelでキャンセルします。executionがnullでなくなるまで照会し、既存Receiptを照合します。同期executeも維持します。Python例は `agent.py --async-run`、TypeScriptはstartRun/runStatus/cancelRunです。

Discover asyncJobs before calling startRun/runStatus/cancelRun for an approved run commit. Poll until execution is non-null, then verify the existing Receipt and exact result bytes. Synchronous execute remains available. See [the optional job profile](ACX_ASYNC_JOBS.md#english).

先发现asyncJobs，再对获批run commit调用startRun/runStatus/cancelRun。轮询至execution非null后验证原Receipt及准确结果字节。同步execute仍可用。参见[可选任务扩展](ACX_ASYNC_JOBS.md#简体中文)。
