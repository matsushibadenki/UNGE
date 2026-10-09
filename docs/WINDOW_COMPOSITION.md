# 同一ウインドウのグラフと設定 / Window composition / 单窗口布局

## 日本語

macOSのTauriサンプルは、1つのネイティブウインドウにwgpuのグラフと右側の設定WebViewを配置します。既定の設定幅は380論理px、ウインドウ最小サイズは720×400です。幅が狭い場合、設定はウインドウ幅の半分を上限にします。設定内容はスクロールできます。

```sh
cargo run --locked -p unge-tauri-host
# 従来の2ウインドウ構成
cargo run --locked -p unge-tauri-host -- --separate-windows
```

Windows/Linuxは実機検証待ちのため、従来の別ウインドウ構成を既定にしています。右側の設定パネルは[ボタンで切り離し／再ドッキング](PANEL_DOCKING.md)できます。配置は通常終了時に保存し、次回起動時に復元します。ドラッグによる任意配置は未実装です。

### 取り込み方

1. `examples/tauri-host/src/workspace.rs` のCompositionをホスト側へ取り込む。配置はRustで計算する。Documentには保存しない。
2. WebViewを持たないTauri Windowを作り、そこからSurfaceRendererを生成する。続いて `Window::add_child` で不透明な設定WebViewを右側へ追加する。サンプルの親Windowは `workspace`、子WebViewは `controls`。
3. Engineに**子WebViewのlabel**をregister_viewし、そのViewへRendererをattachする。Tauri命令は `Webview` 引数から呼び出し元を識別する。トップレベルのWebviewWindowも引き続き利用できる。親Window名で子WebViewを一括認可しない。
4. Capabilityの `webviews: ["controls"]` でローカルの設定WebViewを対象にする。既存invokeの名前・JSON・TypeScript型は変わらない。独自Rust commandラッパーはWebview引数へ合わせる。`execution_observer` はWebviewとWebviewWindowの両方を受け付ける。
5. リサイズ・DPI変更・描画前に、同じLayoutから子WebViewの物理boundsとグラフサイズを求める。`Engine::draw_scaled_region(view, window_physical_size, graph_physical_size, scale)` を呼ぶ。Surface自体はウインドウ全体のサイズを保つ。
6. Taoの入力を同じLayoutで判定し、グラフ内の座標だけを論理pxへ変換する。設定パネルへ出るドラッグはCancelし、途中の座標をDocumentへ確定しない。フォーカス喪失・Resize・DPI変更でもCancelする。
7. 終了時はremove_viewで共有Engine内のRendererを解放する。WebViewへDocumentやGPUフレームを複製しない。

`SurfaceRenderer::draw_region` は左上原点のグラフ領域を検証し、`GpuRenderer::render_in_region` はviewportとscissorを指定して描画します。Glyph準備にもグラフ領域の物理サイズを使うので、文字やノードをウインドウ全体へ引き伸ばしません。領域外は背景色でclearされ、不透明WebViewがその上に配置されます。独立したTextureViewへ直接render_in_regionするホストは、prepare_sizedと同じ非ゼロサイズを渡し、Texture内に収まることを保証してください。

既存draw / draw_scaledは全面描画の互換APIです。今回のAPIはRust内だけの追加です。プロパティ保存、実行、Undo/Redo、ACXは従来の共有Engineとrevision契約を使います。選択ノードへの設定表示の追従は [PROPERTY_INSPECTOR.md](PROPERTY_INSPECTOR.md) を参照してください。ノード内へのHTMLコントロールの重ね合わせは未実装です。

### 検証範囲

macOS実画面で合成、ドラッグ→Undo、パネルへまたがるドラッグのキャンセル、パネル独立スクロール、プロパティ保存、6ノードの実行終了、2テーマと3言語、ウインドウ拡大時の再配置を確認。別ウインドウモードでもノード追加とUndoを確認しました。実GPUテストでは領域描画と全面描画の左側ピクセルが一致し、設定領域にグリッド・文字・形状が描かれないことを検証します。DPI1/1.25/2の入力境界はRustテストで検証します。

⭕️ [Pending] Windows/Linuxの合成、複数モニター間の実DPI移動、最小化からの復帰、OS読み上げは未検証です。通常の非同期描画FPSも別の検証項目です。

## English

The macOS example now places a native wgpu graph and an opaque child WebView inspector in one window. Rust computes the split (380 logical pixels for the inspector, capped at half the window width) and retains Document, View and Renderer ownership. `--separate-windows` restores the previous layout; it remains the default on Windows/Linux pending native verification.

Register the child WebView label with Engine and scope capabilities to that WebView. Commands now receive `tauri::Webview`, so both child views and top-level WebviewWindows work with the same JSON contract. Rust wrappers calling these commands must adapt their argument type; execution_observer accepts either emitter. Use `draw_scaled_region` with the full Surface size, top-left graph size and DPI scale. Compute pointer hit boundaries from the same layout; cancel drags crossing into the inspector. No GPU frames cross IPC.

Native macOS checks covered rendering, drag/undo, boundary cancellation, inspector scrolling, property save, execution, both themes, all three languages and window enlargement. GPU pixel tests verify unscaled content and clipping; Rust tests cover DPI 1/1.25/2. [Button-based docking and floating](PANEL_DOCKING.md) are available for the settings panel. [Automatic inspector selection](PROPERTY_INSPECTOR.md) is available. Drag docking remains unfinished. Other operating systems, physical multi-monitor DPI changes, minimize/restore and screen readers remain unverified.

## 简体中文

macOS示例将原生wgpu节点图和不透明的子WebView设置面板放在同一个窗口。Rust计算左右布局：设置面板380逻辑像素，最多占窗口宽度的一半。Document、View和Renderer仍由共享Engine持有。`--separate-windows` 可恢复双窗口；Windows/Linux在原生验证前仍默认使用双窗口。

用子WebView的label注册Engine，并将Capability限定到该WebView。命令接收 `tauri::Webview`，因此子视图和顶层WebviewWindow可使用相同JSON接口。直接调用这些命令的Rust封装需调整参数类型；execution_observer兼容两种发送者。通过draw_scaled_region传入整个Surface尺寸、左上角图形区域尺寸及DPI。输入边界使用同一布局，拖动进入设置面板时取消。GPU帧不经过IPC。

已在macOS实测合成、拖动／撤销、越界取消、面板滚动、属性保存、执行、两种主题、三种语言和窗口放大。GPU像素测试验证内容不拉伸且不会越界，Rust测试验证DPI 1/1.25/2。设置面板已支持[按钮停靠与浮动](PANEL_DOCKING.md)；已支持[属性面板自动跟随选择](PROPERTY_INSPECTOR.md)，拖拽停靠尚未实现。其他系统、跨显示器DPI切换、最小化恢复及屏幕阅读器尚未验证。
