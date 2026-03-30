# GitHub Actions 构建 win/mac 并上传到 Gitee Release

当前仓库已经有 `win/mac` 的 Tauri 构建矩阵，发布阶段会把安装包上传到 `Gitee Release`，不再把大二进制文件塞进 Git 分支。

## 工作流行为

- 触发方式：
  - 推送版本标签，例如 `v0.1.16`
  - 或在 GitHub Actions 手动运行 `Release Desktop App`
- 构建平台：
  - macOS Apple Silicon
  - macOS Intel
  - Windows x64
- 发布目标：
  - 上传到 Gitee Release
  - 重跑同一个 tag 时会覆盖同名附件

## 需要配置的 GitHub Secrets

- `GITEE_ACCESS_TOKEN`
  - Gitee 个人访问令牌，用于调用 Gitee Release OpenAPI。
- `GITEE_REPOSITORY`
  - Gitee 仓库地址，格式必须是 `owner/repo`。
  - 例如：`strawberry_milk/desktop-client`

## 实现方式

工作流发布阶段会执行两件事：

1. 下载矩阵构建上传的所有安装包和签名文件。
2. 调用 `scripts/publish-gitee-release.sh`：
   - 按 tag 查询 Gitee Release 是否已存在
   - 存在则更新 Release 标题和说明
   - 不存在则创建 Release
   - 列出已有附件并删除同名文件
   - 重新上传当前构建产物

这样你既能保留 Gitee 上的版本页，也不会因为把大安装包推到 Git 分支而卡住。

## 注意事项

- Gitee 仓库必须已存在。
- `GITEE_REPOSITORY` 必须写成 `owner/repo`，不要写完整 URL。
- 如果 Release 创建失败，优先检查 token 是否有仓库访问权限。

## 对应文件

- 工作流：[.github/workflows/release.yml](/Users/zhaodesen/Desktop/desktop-client/.github/workflows/release.yml)
- Gitee 发布脚本：[scripts/publish-gitee-release.sh](/Users/zhaodesen/Desktop/desktop-client/scripts/publish-gitee-release.sh)
