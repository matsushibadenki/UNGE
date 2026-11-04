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
個別ノードの値マップSchemaは [PORT_VALUE_SCHEMAS.md](PORT_VALUE_SCHEMAS.md) のDefinition APIで生成できます。

## English

Run `python3 scripts/generate_contracts.py` after changing Rust contracts; use `--check` in CI.
Committed Draft 7 schemas describe deserialization. `generated.ts` contains input types;
`generated-output.ts` describes the selected types' serialization, which emits every field.
Consumers need no generation dependency unless they enable the optional `schema` feature.
`PropertySchema::json_schema()` exports node property value constraints and localized metadata.
Rust remains authoritative for graph semantics, permissions and numeric representation.
JavaScript callers must respect safe integer limits. ACX protocol schemas remain separate. Per-node value maps use the Definition APIs in PORT_VALUE_SCHEMAS.md.

## 简体中文

修改Rust契约后运行 `python3 scripts/generate_contracts.py`，CI使用 `--check` 检测生成文件是否过期。
Draft 7 Schema描述反序列化，`generated.ts` 提供输入类型，`generated-output.ts` 描述当前类型完整字段的序列化输出。
使用已生成文件不需要启用 `schema` feature。启用后可用 `PropertySchema::json_schema()` 导出节点属性值约束和三语言元数据。
图语义、权限和数值表示仍由Rust验证。JavaScript调用方必须遵守安全整数范围。
ACX协议Schema仍为独立契约。按节点Port名称生成值映射Schema见 PORT_VALUE_SCHEMAS.md。

## schemars 1.2.3への取り込み

直接依存は1.2.3に固定しています。生成器とDefinitionのPort値Schemaは、`SchemaSettings::draft07().into_generator().into_root_schema_for::<T>()`を明示してDraft 7と`definitions`を維持します。1.xの`schema_for!`既定値をそのまま代用しないでください。`RootSchema`の代わりに`schemars::Schema`を使用し、`schema_name()`はCow文字列です。

1.xの再帰root参照`#`は、型生成の前に`#/definitions/<root名>`とその定義へ正規化します。同名の不一致は拒否し、外部参照や未知キーワードの拒否も維持します。単一値enumはconstになり、Port値のタグ取得もconstを読みます。SchemaのJSONバイト列は変わりますが、入力・出力のTypeScript生成物は同一です。検証の最終判断は引き続きRustです。Tauriビルド依存が使うschemars 0.8.22は共存します。

English: Direct dependencies are pinned to schemars 1.2.3. Explicit `SchemaSettings::draft07()` preserves Draft 7/definitions; do not substitute the 1.x default schema_for macro. RootSchema becomes Schema and schema_name returns Cow. Recursive root `#` references are normalized to named definitions before TypeScript generation; conflicts/external references/unsupported keywords still fail. Single-value enums become const, including Port value tags. Schema bytes change while generated input/output TypeScript stays identical. Tauri's transitive 0.8.22 remains alongside it.

简体中文：直接依赖固定schemars 1.2.3，显式使用`SchemaSettings::draft07()`维持Draft 7／definitions，不直接替换为1.x默认schema_for。RootSchema改为Schema，schema_name返回Cow。生成TypeScript前将递归root的`#`转换为具名definition；同名冲突、外部引用和未知关键字仍拒绝。单值enum改为const，Port值标签也读取const。Schema字节变化，输入／输出TypeScript生成物保持一致。Tauri间接依赖的0.8.22仍共存。

参照：[公式移行手順](https://graham.cool/schemars/migrating/)、[1.2.3 API](https://docs.rs/schemars/1.2.3/schemars/)。
