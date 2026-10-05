# Rust契約からの型・Schema生成

## 日本語

グラフ、Command、ノード定義、実行値・進捗、Tauri Request/レスポンスの型をRustから生成します。
通常の利用では `schema` featureは不要です。生成済みファイルをそのまま取り込めます。

```sh
python3 scripts/generate_contracts.py
python3 scripts/generate_contracts.py --check
python3 scripts/test_contracts.py -v
```

生成にはPython 3.9以降とRust、TauriライブラリのOS別前提条件が必要です。
テストには `jsonschema>=4.18,<5` が必要です。CIはmacOSで再生成差分と検証を実行します。

- `bindings/schema/*.schema.json`: Draft 7の入力形状。Serdeのtag/rename/default/Option/未知フィールド規則を反映します。
- `bindings/typescript/generated.ts`: 入力型。SerdeのdefaultやOptionに応じて省略を許します。
- `bindings/typescript/generated-output.ts`: 出力型。対象のRust型は全フィールドを出力します。将来 `skip_serializing_if` 等を追加する場合は生成器も更新してください。
- 既存の `index.ts`、`definitions.ts`、`execution.ts` は生成型を公開します。invokeのクライアントや翻訳辞書は手書きです。

対象型の宣言に `#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]` を付け、
`crates/unge-contracts/src/main.rs` のroot一覧に追加して再生成します。
新しいSchema形状、外部参照、同名で異なる定義は生成時に拒否します。
生成ファイルを直接編集せず、Rust側を変更してください。

アプリ固有のPropertySchemaは `schema` featureを有効にして `json_schema()?` を呼ぶと、
プロパティ値の型・範囲・文字数・選択肢・必須値・追加キー制限・初期値・3言語メタデータを公開できます。
初期値は検証時には適用しません。変更時は既存のRegistry/Editor検証が最終判断します。

```toml
unge-executor = { path = "../vendor/unge/crates/unge-executor", features = ["schema"] }
```

```rust,ignore
let metadata = registry.definition("app.node").expect("registered");
let properties_schema = metadata.property_schema.json_schema()?;
```

### 境界

Schemaは構造を説明します。循環、Port間の接続可否、revision、座標の妥当性、ホスト権限、
プロパティ宣言自体の整合性はRustで検証します。JSON Schemaのintegerは数学的な整数を扱うため、
`2.0` のようなJSON数値を受け付ける場合がありますが、Rustのi64プロパティ検証は整数表現を要求します。
JavaScriptではi64/u64/usizeもnumberになります。安全な整数範囲を越える値は損失のない通信方式が必要です。
リソース型はIDとDataTypeだけを公開し、GPU/画像の内容を転送しません。

ACX Core/Intent/Jobの規範SchemaとACXのTypeScriptライフサイクル型は今回の生成対象ではありません。
既存のACX適合テストを維持し、別の契約として扱います。
個別ノードのPort名に対応する値マップのSchema生成は今後の項目です。

## English

Run `python3 scripts/generate_contracts.py` after changing Rust contracts; use `--check` in CI.
Committed Draft 7 schemas describe deserialization. `generated.ts` contains input types;
`generated-output.ts` describes the selected types' serialization, which emits every field.
Consumers need no generation dependency unless they enable the optional `schema` feature.
`PropertySchema::json_schema()` exports node property value constraints and localized metadata.
Rust remains authoritative for graph semantics, permissions and numeric representation.
JavaScript callers must respect safe integer limits. ACX protocol schemas and per-node port value maps remain separate work.

## 简体中文

修改Rust契约后运行 `python3 scripts/generate_contracts.py`，CI使用 `--check` 检测生成文件是否过期。
Draft 7 Schema描述反序列化，`generated.ts` 提供输入类型，`generated-output.ts` 描述当前类型完整字段的序列化输出。
使用已生成文件不需要启用 `schema` feature。启用后可用 `PropertySchema::json_schema()` 导出节点属性值约束和三语言元数据。
图语义、权限和数值表示仍由Rust验证。JavaScript调用方必须遵守安全整数范围。
ACX协议Schema和按节点Port名称生成值映射Schema尚未纳入此功能。

生成は [Schemars 0.8.22の公式API](https://docs.rs/schemars/0.8.22/schemars/) を使用しています。バージョンを固定し、Serde属性との互換性を実際のJSONで検証しています。
