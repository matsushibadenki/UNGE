# Themes / ダーク・ライト / 深色与浅色

## 日本語

`unge-render::Theme` は `Dark` / `Light` を提供します。テーマはRustのView状態で管理します。変更はDocumentのrevisionやUndo履歴を増やさず、選択・Viewport・操作プレビューを維持します。

### Tauriへの取り込み

```rust
engine.dispatch("controls", unge_tauri::Request::SetTheme {
    theme: unge_render::Theme::Light,
})?;
let appearance = engine.appearance("controls")?;
// appearance.theme と appearance.colors（CSS変数名 → #rrggbb）
```

WebViewからはTypeScriptクライアントを使います。

```ts
await client.theme('light');
const appearance = await client.appearance();
document.documentElement.dataset.theme = appearance.theme;
for (const [token, value] of Object.entries(appearance.colors)) {
  document.documentElement.style.setProperty(`--${token}`, value);
}
```

既存の独自invoke handlerに組み込む場合は `unge_tauri::appearance` も登録します。組み込みの `unge_tauri::handler()` はdispatch・inspect・appearanceを含みます。

- wire形式は `{ kind: 'set_theme', theme: 'dark' | 'light' }`。戻り値は既存のSummaryです。`expected_revision` は不要です。
- `appearance` 命令は呼び出し元の登録済みViewだけを読みます。`Appearance { theme, colors }` にGraphやGPU画像は含みません。
- CSS tokensは `background` / `surface` / `border` / `accent` / `text` / `muted` / `hover`。操作画面で `var(--text)` 等として使います。
- IPC経由のテーマ変更時には、呼び出し元のウインドウへ `unge://appearance-changed`（Appearance）を通知します。`unge://changed`（Summary）も従来どおり通知します。イベントは補助通知です。再接続・再読み込み時はappearanceを取得します。
- Rustホストからの直接dispatchはイベントを発行しません。ホストで通知・再描画を要求します。
- View登録時の既定値はDarkです。ウインドウを閉じればView設定は破棄されます。ライブラリにはディスクへの設定保存はありません。

サンプルは「ダーク」「ライト」のボタンを提供し、Rustが返したCSS配色を表示します。成功した選択だけをWebViewのlocalStorage（`unge.theme`）に保存し、再起動時にRustへ再適用します。ストレージが利用できない場合はセッション中だけ有効です。OS配色への自動追従やSystemモードは未実装です。ホストがRust側で設定ファイルを管理する構成にも置き換えられます。

ネイティブのタイトルバーはホストの `Window::set_theme` / `WebviewWindow::set_theme` で更新します。Tauri 2のLinux/macOSではこのネイティブ設定はアプリ全体に適用されます。GPUとCSSのView別テーマは独立しています。複数Viewで異なるテーマを使うホストは、ネイティブ枠に適用するテーマをアプリ側で決めてください。

### TauriなしのGPU描画

```rust
let scene = index.scene_with_theme(
    viewport, &selection, &preview, &catalog,
    unge_core::Locale::Ja, unge_render::Theme::Light,
)?;
renderer.prepare_sized(&device, &queue, &scene, viewport, physical_size)?;
renderer.render(&mut encoder, &target);
queue.submit([encoder.finish()]);
```

既存のscene / scene_with_preview / scene_with_labelsはDarkを使います。SceneIndexはテーマ切り替えで再構築しません。背景、グリッド、ノード、枠、Port、文字、Edge、仮接続、選択矩形をテーマから解決します。文字のAtlasと整形キャッシュは色変更時にも再利用します。

配色の唯一の定義は `crates/unge-render/src/theme.rs` です。`Theme::palette()` のsRGB tokensをCSSに使い、GPUには `ThemeColor::linear()` で線形RGBへ変換します。GPU側では背景のclearとグリッドuniformもSceneに合わせて更新します。Sceneを直接構築するコードは新しいbackground / grid_colorを設定するか、`..Scene::default()` でDarkの既定値を補います。自作Sceneのbackground / grid_color / Quad色 / TextLabel色も線形RGBAで渡してください。再利用例のターゲットはsRGB Surfaceです。Rgba8Unormへのreadbackテストは線形値を検証します。

現在提供するのは2つの組み込み配色です。任意のテーマ登録、色設定の編集UI、OS高コントラスト連動は未実装です。通常テストは文字のコントラスト4.5以上、操作を示す枠・アクセント・Edgeのコントラスト3以上を確認します。これはアプリ全体のアクセシビリティ検証の完了を意味しません。

### 検証

```sh
cargo test --workspace --all-features --locked
cargo test --locked -p unge-render -- --ignored
```

テーマ間の同一形状、すべての配色、View分離、Document不変性、Undo/Redo維持、操作中の変更、未知View/テーマの拒否を確認します。実GPUテストはDark→Light→Darkの順で背景・グリッド・Node・Port・文字のピクセルとAtlas再利用を確認します。GPU不可の環境ではignoredテストを未検証と扱ってください。

## English

`unge-render::Theme::{Dark, Light}` controls a Rust-owned view. Use `client.theme('light')` and `client.appearance()` to obtain the authoritative theme and shared CSS tokens. IPC changes emit `unge://appearance-changed` to the calling window and the existing Summary event. Themes preserve Document revision, undo history, selection and active gestures; registered views can use different themes.

For standalone GPU hosts use `scene_with_theme`. Geometry indexes and text caches remain reusable. Palette definitions are sRGB; `ThemeColor::linear()` converts them for GPU rendering. Scene colours, including background and grid, are linear RGBA. Built-in palette tests check readable text and interaction contrast; full accessibility work remains planned.

The desktop example offers Dark/Light buttons and remembers successful choices in localStorage, reapplying them to Rust after launch. Storage failure falls back to session settings. Library views default to Dark; library persistence and automatic OS theme following are not provided. Hosts may implement their own Rust preference store. Tauri native window themes are app-wide on Linux/macOS; hosts decide which view controls native chrome.

Run the explicit ignored GPU tests with `cargo test --locked -p unge-render -- --ignored`. The tests exercise Dark→Light→Dark pixels and glyph atlas reuse. Custom theme registration and high-contrast OS integration remain planned.

## 简体中文

`unge-render::Theme::{Dark, Light}` 控制Rust持有的视图主题。使用 `client.theme('light')` 切换，并通过 `client.appearance()` 获取主题和共享CSS色值。IPC切换会向调用窗口发送 `unge://appearance-changed`，同时保留Summary通知。主题不增加Document版本或撤销历史，并保持选择与操作预览。各个注册视图可使用不同主题。

独立GPU宿主使用 `scene_with_theme`。几何索引、文字图集与排版缓存可继续复用。配色定义为sRGB，GPU使用 `ThemeColor::linear()` 转换后的线性RGBA。背景、网格、节点、文字、端口、连线与选择框全部切换。测试检查文字及交互颜色的对比度，完整无障碍支持仍待实现。

桌面示例提供“深色”“浅色”按钮，只在切换成功后将选择保存到localStorage，并在重启后重新应用到Rust。存储不可用时仅保持当前会话。库默认使用Dark，不含设置文件持久化或OS主题自动跟随；宿主可自行管理Rust设置文件。Tauri在Linux/macOS上的原生窗口主题作用于整个应用；多视图宿主需自行决定窗口边框的主题。

通过 `cargo test --locked -p unge-render -- --ignored` 显式运行GPU测试，验证Dark→Light→Dark的像素与图集复用。自定义主题注册及OS高对比度联动仍待实现。
