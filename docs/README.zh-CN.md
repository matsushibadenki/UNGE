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
示例支持添加节点、撤销、重做和缩放。节点文字及鼠标直接编辑尚未实现。

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
