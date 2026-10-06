# Port名ごとの実行値Schema

## Rustで使う

`Definition` から、そのノードのPort名・型・必須性・入力個数を反映したDraft 7 Schemaを生成できます。任意の `schema` featureを有効にしてください。

```toml
unge-executor = { path = "../vendor/unge/crates/unge-executor", features = ["schema"] }
```

```rust,ignore
let definition = registry.definition("math.add").expect("registered");
let inputs_schema = definition.input_values_schema()?;
let outputs_schema = definition.output_values_schema()?;
// schema featureなしでも使えるRust側検証
 definition.validate_inputs(&inputs)?;
 definition.validate_outputs(&outputs)?;
```

Schema生成はUI/GPUに依存しません。ホスト固有Definitionにも使えます。空/重複Port名は生成・検証時に拒否します。定義の入力と出力の名前空間は別です。

## 値の契約

| 対象 | 形 |
|---|---|
| Inputs | Port名→Value配列。Singleでも配列 |
| Outputs | Port名→単一Value。出力Cardinalityは配列化しない |
| 必須入力 | キー必須、配列が1件以上 |
| 任意入力 | キー省略または空配列を許可 |
| Single入力 | 最大1件 |
| Multiple入力 | 追加の件数上限なし |
| 必須出力 | キー必須 |
| 未知のPort名 | 入力/出力とも拒否 |

Valueは既存の `kind` / `value` 形式です。Float PortでIntを受け付ける等の暗黙変換はありません。Anyは全Value型を許可します。ResourceはIDとdata_typeだけを持ち、data_typeの一致で判定します。実行時の既存契約ではprimitive型を名乗るResourceも許可されるため、Schemaも同じ扱いです。ホストのResourceStoreで内容・権限・存在を検証してください。

SchemaのValue構造はSchemarsから生成し、Port型に合うvariantへ絞ります。i64の範囲と有限f64の範囲を付けます。ただしJSON数値の字句表現、Rustのデシリアライズ、JavaScriptの安全整数、任意精度のSchema validatorは完全同値ではありません。大きい整数をJavaScriptへ渡す場合は既存の安全整数制約を守ってください。UUIDのformat検証はvalidator側でも有効化してください。Value内の未知フィールドは既存Serdeと同様に許容します。

Schemaは実行値マップの検査用です。DAG・接続数・必須接続・DefinitionとInstanceの一致はRegistry/Graphの検証を使います。Schedulerの入力組み立て・実行・キャッシュ・IPC/ACXのwire形式は変更していません。

## 生成済み例をAIへ渡す

`python3 scripts/generate_contracts.py` は `bindings/schema/math-port-values.json` も更新します。type_id→version/inputs/outputsのマップです。必要なinputsまたはoutputsを取り出して、独立したSchemaとしてvalidatorへ渡してください。ファイル全体を1つのSchemaとして扱わないでください。

```sh
cargo run --locked -p unge-contracts -- --port-schemas
python3 scripts/generate_contracts.py --check
python3 scripts/test_contracts.py -v
```

アプリ固有Registryでは、各Definitionの2つの生成メソッドで同じ形式を書き出します。登録型が動的なので、Port名のTypeScript型をビルド時に固定生成する機能は今回追加していません。ホストはこのSchemaを能力公開に添付できます。既存ACX仕様・Manifestへ自動追加することはありません。

## 検証

13種のDataType、Single/Multiple、必須/任意を組み合わせた52定義で、実Rust Valueをシリアライズした3,068件の入力/出力判定をPython Draft7Validatorと照合しました。未知Port、欠損/空入力、多重入力、primitiveとResource、Customの一致/不一致を含みます。別Rustテストで非有限Floatと重複Portを拒否します。

[Draft 7の検証仕様](https://json-schema.org/draft-07/draft-handrews-json-schema-validation-01) を使用しています。すべてのvalidatorや数値表現における同値性を保証するものではありません。

## English

Enable the optional executor `schema` feature to generate per-definition `input_values_schema()` and `output_values_schema()`. Runtime `validate_inputs()`/`validate_outputs()` require no schema feature. Inputs are arrays even for Single ports; required inputs are nonempty, Single has maxItems 1, outputs are individual Values, and unknown port keys are rejected. Resource data types follow existing runtime rules; resource bytes never cross the schema boundary. `math-port-values.json` is a type-ID map of independent input/output schemas, not a single schema. Dynamic TypeScript port-name types and automatic ACX publication are not included. Numeric representation and resource existence remain host responsibilities.

## 简体中文

启用executor的可选 `schema` feature，即可通过 `input_values_schema()` / `output_values_schema()` 生成每个Definition的Schema。Rust `validate_inputs()` / `validate_outputs()` 无需此feature。Single输入也使用数组，必填输入非空，Single最多1项，输出为单个Value，未知Port名拒绝。Resource按已有运行时的data_type规则判断，不传输资源内容。`math-port-values.json` 按type_id保存独立输入/输出Schema，整个文件不是单一Schema。动态Port名的TypeScript类型生成及ACX自动发布尚未加入。数值表示及资源存在性仍由宿主验证。
