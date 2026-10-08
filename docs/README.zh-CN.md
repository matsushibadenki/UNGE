# UNGE

面向 Tauri 2、Rust 与 wgpu/WGSL 的可复用节点图基础库。
v0.1 实现了核心基础功能，完整状态请参阅[路线图](ROADMAP.md)。

## 快速开始

```sh
cargo test --locked
cargo run --locked -p unge-headless
cargo run --locked -p unge-tauri-host
```

桌面示例需要安装 [Tauri 所需依赖](https://v2.tauri.app/start/prerequisites/)。
WebView 提供操作界面，独立原生窗口负责 GPU 绘制。
示例支持操作面板与GPU的深色/浅色切换并保存选择；见[主题集成](THEMES.md#简体中文)。还支持添加节点、撤销、重做、拖动、框选、端口连接、平移和缩放。节点与端口文字已使用Rust持有的字形图集渲染，宿主可提供三语名称；见[GPU文字集成](GPU_TEXT.md#简体中文)。逻辑坐标和宿主设置见[指针集成](POINTER_INPUT.md#简体中文)。

## 集成

将整个目录复制到目标项目，通过 Cargo path 依赖引入所需 crate。
core 和 executor 不依赖 Tauri。render 接收 Rust 管理的纹理或原生 Surface。
`bindings/typescript` 通过注入 invoke 函数连接 Tauri，并提供英语、日语和简体中文错误文案。

文档、历史、视图状态及 GPU 缓冲区均由 Rust 持有。WebView 仅发送带版本号的命令，接收简短状态。
图像、张量和视频帧留在宿主资源存储中，执行值只传递不可变资源 ID。

向 AI 交接时，从 [AGENTS.md](../AGENTS.md)、[AI_INTEGRATION.md](AI_INTEGRATION.md) 和 [ARCHITECTURE.md](ARCHITECTURE.md) 开始。
无界面示例计算 `20 + 22 = 42`，也可以导出带版本号的图 JSON。
当前仅支持可信的 Rust 执行器。WASM 沙箱、流式执行、子图、协作和浏览器独立运行仍在规划中。
尚未验证一万个节点、三万条边及 60 FPS 的性能目标。

## 通过ACX由AI操作节点

运行 `cargo build -p unge-acx-provider` 和 `python3 examples/acx-provider/agent.py`，完成能力发现、授权编辑、执行、回执验证和恢复。相同适配器可连接正在运行的Tauri Engine。请参阅[ACX集成指南](ACX_INTEGRATION.md)。运行时不需要ACX符号链接。

## 编辑验证和历史容量

Definition支持属性类型、范围、选项和初始值。共享Editor可安装Registry验证器，原子验证编辑、撤销及重做。历史按Undo与Redo合计步数和序列化字节限制。所有示例均已启用验证。参阅[集成与兼容性](PROPERTY_VALIDATION.md#简体中文)。

执行缓存支持完整键哈希查找及条目数、字节容量限制。主机配置与纯节点约定见[缓存集成](EXECUTION_CACHE.md#简体中文)。

Rust主机可以接收节点执行进度，并通过Future设置整个run的期限。参见[执行进度与取消](EXECUTION_PROGRESS.md#简体中文)。

Tauri与ACX可共享RunService，通过执行ID、冻结revision及有限状态记录管理进度和主机取消。参见[共享执行服务](EXECUTION_SERVICE.md#简体中文)。

操作窗口新增执行、取消按钮及进度显示，支持查询恢复，并提示执行开始后文档的版本变化。

使用 `python3 examples/acx-provider/agent.py --async-run` 验证可选异步任务流程，AI可在同一pipe查询、取消获批执行。参见[任务扩展](ACX_ASYNC_JOBS.md#简体中文)。

Rust契约可生成TypeScript类型和JSON Schema。重新生成及属性值约束见 [契约生成](CONTRACT_GENERATION.md)。

已添加10,000节点、30,000连线的可重复CPU计测和BVH中位数分区优化。见 [性能测量](PERFORMANCE.md)；60FPS仍未验证。

小规模节点移动可增量更新绘制Index，UI、指针操作及ACX共用此路径。见 [Index更新](INDEX_UPDATES.md)。

三次曲线按形状及zoom自适应细分，减少直线和远景连线的几何生成量。误差目标及上限见 [曲线细分](CURVE_TESSELLATION.md)。

GPU Group框和标题可跟随成员几何与拖动预览更新。见 [Group绘制](GROUP_RENDERING.md)。

新增基于Rust分页节点概要的HTML控件，支持键盘选择、移动和删除节点。 [ACCESSIBILITY.md](ACCESSIBILITY.md)。

三语言UI支持创建组、改名、编辑成员、删除组和选中组成员。 [GROUP_EDITING.md](GROUP_EDITING.md)。

新增每个Definition的输入/输出值Schema及Rust验证API。 [PORT_VALUE_SCHEMAS.md](PORT_VALUE_SCHEMAS.md)。

节点新增点阵背景、角色颜色、可选符号/说明及清晰的选择外框。 [NODE_APPEARANCE.md](NODE_APPEARANCE.md)。

[节点属性界面及API](PROPERTY_INSPECTOR.md)
