# Property validation and bounded history

[日本語](#日本語) · [English](#english) · [简体中文](#简体中文)

## 日本語

### ホストへの組み込み

```rust,ignore
use std::sync::Arc;
use unge_core::{Document, Editor, HistoryLimits};

let registry = Arc::new(unge_executor::math_registry());
let editor = Editor::with_history_limits(
    Document::default(),
    HistoryLimits { max_steps: 256, max_bytes: 16 * 1024 * 1024 },
)?.with_validator(registry.clone())?;
let engine = unge_tauri::Engine::from_editor(editor);
// GUI不要なら unge_acx::MemoryHost::from_editor(editor) に渡す。
// ACX Providerには同じregistryと、UIが使う同じengineを渡す。
```

`DocumentValidator` はcoreの独立した拡張点です。Registryが実装する検証は登録型・Portの一致とプロパティを検査します。
ロード直後と、すべてのCommand・Undo・Redoの確定時に実行します。未接続の必須入力は編集中には許可し、実行開始時に拒否します。
Batch内の一時的なプロパティ欠落は許可し、完成後の状態を検証します。エラー時はDocument・revision・Undo/Redo履歴を維持します。
validatorは副作用を持たず、決定的で、Editorへ再入しない実装にしてください。最初の編集後は交換できません。

`Editor::new` / `Engine::new` / `MemoryHost::new` はグラフの構造だけを検証する互換APIです。
編集時のプロパティ検証を有効にするには、上の設定を使います。3つのサンプルはこの設定済みです。
ACXのPreflightはホストの設定にかかわらず、ProviderのRegistryで編集後のプロパティを検証します。
ホストへ直接届くCommandも検証するため、共有Editorにも同じRegistryを設定してください。

### Definitionの追加契約

`Definition.property_schema` は、次のフィールドを持つUNGE固有の型付きスキーマです。汎用JSON Schemaのインタープリターではありません。

| フィールド | 意味 |
|---|---|
| `fields` | プロパティ名→PropertyDefinition |
| `additional_properties` | 未宣言のプロパティを許すか。互換性のため既定true |
| `name` / `description` | `en` / `ja` / `zh_cn` の文言 |
| `value_type` | `bool`、`int`、`float`、`string`、`json` |
| `required` | 欠落を拒否するか |
| `default` | 生成時の初期値。nullは初期値なし |

Intはi64範囲の整数、Floatは有限数を受け付けます。どちらも任意の `minimum` / `maximum` を含み、境界値は許可します。
Stringには `min_length`、`max_length`、`choices` を設定できます。長さはUnicode scalar value数で、バイト数や表示上の文字数ではありません。
JSON型は任意のJSON値を許します。画像やフレームの転送用には使わず、ホスト側のリソースIDを保存してください。

矛盾した範囲、空/重複した選択肢、範囲外の選択肢、不正な初期値はRegistry登録時に拒否します。
初期値は `Definition::instantiate()` で一度だけ適用します。ACXの `create_node.properties` は初期値へ上書きします。
検証時やロード時に欠落を自動補完しません。必須値を削除すると `invalid_properties` になります。
`set_property.value: null` は削除です。任意JSONのnull保存とは区別してください。

以前のJSON Definitionは `property_schema` がない場合、未宣言プロパティを許す空スキーマとして読み込みます。
RustのDefinitionリテラルには `property_schema: PropertySchema::default()` 等を追加してください。
`math.number` はversion 2になり、有限数の必須 `value` と初期値0を宣言します。以前の不正な保存値は補正せず拒否します。
スキーマを変えるホストはDefinitionのversionを更新し、必要ならロード前に明示的にデータを移行してください。

ACXの `observe(query: "definitions")` でスキーマを取得できます。TypeScriptには `definitions.ts` と `createAcxClient().definitions()` を追加しています。
整数をJavaScriptで扱う場合はsafe integer範囲に制限するか、精度を保つJSON処理を使ってください。

### 履歴容量

既定値は256ステップ・16MiBです。`Editor::new(document, count)` は指定件数と16MiBを使います。
`HistoryLimits` はUndoとRedoを合わせた件数・シリアライズ後の命令ペイロード容量を制限します。
実際のヒープ使用量、Document本体、処理中の一時コピー、GPUリソースの上限ではありません。
`history_stats()` で現在のUndo/Redo件数と保持バイト数を取得できます。

古いUndoから削除し、必要なら現在位置から遠いRedoを削除します。Undo/Redoで逆命令が大きくなる場合も再計量します。
1ステップが容量を超える場合、編集は成功させたうえで履歴全体を破棄します。上限0でも編集可能ですが履歴を保持しません。
途中の編集を飛ばして古いCommandだけを取り消すことはありません。新しい編集後には再び履歴を作れます。
ACXのRecoverは履歴が残っている場合だけ成功し、破棄済みなら `recovery_unavailable` を返します。

## English

Install a shared `Arc<Registry>` with `Editor::with_validator` before the first edit, then pass that editor to `Engine::from_editor` or `MemoryHost::from_editor`. Validation runs on the loaded document and on the final state of every command, undo and redo. Invalid edits preserve the document, revision and both history stacks. Editing allows unconnected required ports; execution checks them. The basic `new` constructors retain structural validation only; all three examples now install registry validation.

`Definition.property_schema` describes localized fields, required values, defaults, bool/int/float/string/JSON types, inclusive numeric bounds, Unicode scalar length limits and string choices. The registry rejects inconsistent constraints and invalid defaults. Defaults apply only during instantiation; ACX creation overlays supplied values. Validation never repairs missing properties. ACX preflight validates final properties even when a host has not installed a validator. `math.number` version 2 requires a finite `value`, defaulting to 0 at creation. Legacy serialized definitions without a schema stay open; Rust literals need the new field. This is UNGE's typed schema, not arbitrary JSON Schema. TypeScript definitions and a typed ACX definitions query are included.

History defaults to 256 steps / 16 MiB of serialized undo plus redo payloads. This does not bound total heap or GPU memory. Use `HistoryLimits` and `history_stats()` to configure/observe it. Oldest undo entries are evicted first, then farthest redo entries. Every inverse is measured, including on undo/redo. An oversized step succeeds but clears history to prevent skipping edits; zero limits disable retention. ACX recovery returns `recovery_unavailable` when the required undo step was discarded.

## 简体中文

首次编辑前，通过 `Editor::with_validator` 安装共享 `Arc<Registry>`，再将Editor传给 `Engine::from_editor` 或 `MemoryHost::from_editor`。加载时及每次Command、Undo、Redo完成时验证；无效编辑保留文档、版本和两侧历史。编辑时允许必填端口尚未连接，执行前才要求完整连接。基础 `new` 构造函数仅验证图结构，三个示例均已启用注册表验证。

`Definition.property_schema` 声明三语名称、必填值、初始值、布尔/整数/浮点/字符串/JSON类型、包含边界的数值范围、Unicode scalar长度及字符串选项。注册时拒绝矛盾约束和无效初始值。仅创建节点时应用初始值，ACX创建参数覆盖初始值，验证时不自动修复缺失属性。即使宿主未配置validator，ACX预检仍验证最终属性。`math.number`版本2要求有限数值 `value`，创建时默认为0。旧JSON定义未带schema时仍允许任意属性，Rust结构体字面量需补充该字段。这是UNGE的类型化schema，不是通用JSON Schema。包含TypeScript类型和定义查询接口。

历史默认限制为256步及16MiB，容量按Undo与Redo命令的序列化字节总和计算，不代表总堆内存或GPU内存上限。通过 `HistoryLimits` 配置、`history_stats()` 查询。先移除最旧Undo，再移除距离当前位置最远的Redo。撤销/重做时也重新计量逆命令。单步超过容量时仍完成编辑，但清空历史以免跳过中间操作；上限0禁止保留历史。ACX所需历史已丢弃时，Recover返回 `recovery_unavailable`。
