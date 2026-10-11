# 构建与验证

Windows，Rust 1.92 或更新版本，MSVC 与 Windows SDK。

```powershell
cargo build --release --locked --workspace -j 1
cargo test --release --locked -p fluent-netease-plugin -j 1
```

插件文件为 `target/release/netease.dll`；将其与根目录 `plugin.json`、`LICENSE`、`LICENSE-NETEASE` 放入播放器的 `plugins/netease`。播放器使用匹配 API / ABI 2 的 `pluginloader.exe`。

本仓库附带固定版本的 MIT SDK 源码，不依赖另一个私有仓库或原工作区。默认测试使用合成数据；公开网络测试默认忽略，不需要真实账号。

独立迁移验证：32项单元测试、2项实际 DLL / ABI 集成测试及1项结构测试通过，2项需要显式公开网络授权的测试保持忽略。DLL / AppContainer / FileApi / DPAPI 与注销回归均不依赖真实账号；未因此声称验证了本人扫码或会员音质。
