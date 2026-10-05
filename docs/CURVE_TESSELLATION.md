# 曲線の適応分割 / Adaptive curves / 曲线自适应细分

## 日本語

固定の8/24分割を、ズームと曲線の形に応じた適応分割へ変更しました。
ノード間のEdgeと、一時的な接続プレビューで同じ処理を使います。
Graph/Document/IPCの契約は変更せず、分割とQuad生成はRust側で行います。

制御点が端点間の線分に十分近ければ1本の線分を出力し、遠ければ曲線を半分に分けて再判定します。
分割は [de Casteljau法](https://pomax.github.io/bezierinfo/#decasteljau) を使います。
無限に延びる直線ではなく有限の線分までの距離を使うため、逆向き接続、同一直線上のはみ出し、
端点が一致する曲線でも、必要な折り返しや膨らみを残します。

- 目標の幾何誤差: 0.75論理ピクセル。world側の閾値は `0.75 / zoom`。
- 分割の上限: 深さ7、1曲線につき最大128線分。
- 数値計算: Rustのf64で分割し、GPUへの端点は従来どおりf32。
- 一時領域: 分割用のヒープ配列を作らず、上限付き再帰で順にQuadを出力。
- 描画順、色、線幅、曲線のカリング、端点、変更時のIndex更新は既存の契約を維持。

閾値は形状の目標値です。上限に達した極端な曲線では0.75pxを超えることがあります。
大きなworld座標ではf32へ戻す際の丸め誤差も残ります。HiDPIでの物理ピクセル誤差はscale factorに比例します。
曲がりの強い接続では従来の24分割より増えることもあり、常に生成量が減る保証はありません。
GPUバッファ上限は既存のRendererが検査します。Scene全体の生成量・FPSの保証は追加していません。

テストでは独立した3次多項式の4,097サンプルと生成線分の距離を比較し、
通常の曲線、逆向き、端点一致、直線、ズーム範囲0.02〜16の誤差を確認します。
極端な座標では有限値・端点一致・連続性・最大128線分を確認します。
実GPUテストにも、曲線の中点ピクセル検証を追加しました。

再利用する場合は通常の `SceneIndex::scene*()` を呼ぶだけです。
新しいJavaScriptコード、フレーム転送、追加の依存ライブラリは不要です。
大規模グラフの実測は [PERFORMANCE.md](PERFORMANCE.md)。60FPSは未検証です。

## English

Edges and temporary connection previews now share adaptive cubic subdivision in Rust.
The target error is 0.75 logical pixels, adjusted by zoom. De Casteljau subdivision emits straight spans directly.
Distance to the finite endpoint segment preserves collinear overshoot, backward links and coincident endpoints.
Recursion is capped at depth 7 (128 segments). Extreme curves may exceed the error target at this cap;
f32 GPU endpoint rounding and HiDPI scaling also affect physical pixel error.
Complex curves may use more segments than before. No frame-rate guarantee or IPC change is introduced.
Analytic curve sampling checks approximation error, and a real GPU test checks a curved stroke pixel.

## 简体中文

Edge和临时连接预览共用Rust侧的自适应三次曲线细分。
目标误差为0.75逻辑像素，按zoom换算到world坐标；直线段可直接输出。
de Casteljau细分使用到有限端点线段的距离，保留反向连接、共线超出部分和端点重合曲线的形状。
递归深度上限7，最多128线段。极端曲线到达上限时可能超过误差目标，f32端点舍入及HiDPI缩放也会影响物理像素误差。
复杂曲线可能比之前生成更多线段，此功能不保证FPS，也不改变IPC契约。
测试比较解析曲线采样误差，并在实GPU测试中验证曲线像素。
