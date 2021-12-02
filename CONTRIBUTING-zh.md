# 贡献

Monoio 目前仍处于较不完善，我们欢迎任何人对 Monoio 进行贡献：）

如果你不知道从何开始，可以试着使用 Monoio 写一个具体的 example 项目，或许你可以从中发现需要改进的地方。目前 Monoio 只提供了部分网络组件，它们在现有生态的兼容性上也有较大问题。你可以对 Monoio 主项目进行贡献，也可以基于 Monoio 的 IO 接口对齐一些现有组件。

## 行为准则
Monoio 项目遵守 [Rust 行为准则](https://www.rust-lang.org/zh-CN/policies/code-of-conduct)。违规情况可以通过邮件报告给管理员: chihai.hain@bytedance.com / john.xu@bytedance.com。

## Pull Requests
我们欢迎任何代码贡献，请尽量使它们遵循以下规则：

1. 测试通过并格式化。虽然我们有 CI 来自动化测试，但是推荐你在提交之前本地通过 `cargo test` 并 `cargo fmt`。很多时候你可能需要带上 `--all-features` 确保在不同 feature 下都能正常工作。
2. 在 PR 中尽量详细地描述你 **解决的问题**、**解决问题的思路** 和 **架构设计**。并且代码中尽量也通过丰富的文档描述代码的工作逻辑。
3. 在 git message 中简洁清晰地描述你的 commit。

推荐使用 GPG 签署你的 commit。

## 讨论群
如果你有任何需要，欢迎加入我们的讨论群。

飞书群可以扫描下面的二维码：

<img src=".github/resources/monoio-lark.png" height="310px" width="274px" >

加入 Telegram 讨论群请使用链接：[Link](https://t.me/+zVUaFzxnmK43Yzk1)。