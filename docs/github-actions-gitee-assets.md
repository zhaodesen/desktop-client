# GitHub Actions 构建 win/mac 并只推安装包到 Gitee

当前仓库已经有 `win/mac` 的 Tauri 构建矩阵，现在发布阶段改成只把构建出的安装包推到 Gitee 仓库分支，不再创建 GitHub Release，也不再调用 Gitee Release API。

## 工作流行为

- 触发方式：
  - 推送版本标签，例如 `v0.1.16`
  - 或在 GitHub Actions 手动运行 `Release Desktop App`
- 构建平台：
  - macOS Apple Silicon
  - macOS Intel
  - Windows x64
- 发布目标：
  - 只把安装包推到 Gitee 仓库分支
  - 默认分支名：`release-assets`
  - 默认目录：`muyu/<版本号>` 和 `muyu/latest`

## 需要配置的 GitHub Secrets

- `GITEE_USERNAME`
  - Gitee 用户名，用于 HTTPS 推送。
- `GITEE_ACCESS_TOKEN`
  - Gitee 个人访问令牌，作为 Git 推送密码使用。
- `GITEE_REPOSITORY`
  - Gitee 仓库地址，格式必须是 `owner/repo`。
  - 例如：`zhaodesen/desktop-client`

## 实现方式

工作流发布阶段会执行两件事：

1. 下载矩阵构建上传的所有安装包和签名文件。
2. 调用 `scripts/publish-gitee-assets.sh`：
   - 拉取 Gitee 仓库
   - 切换或创建 `release-assets` 分支
   - 把当前构建产物复制到 `muyu/<tag>/`
   - 同步一份到 `muyu/latest/`
   - 提交并推送到 Gitee

这样你不需要依赖 Gitee Release 页面，安装包本身就保存在 Gitee 仓库里。

## 注意事项

- Gitee 仓库必须已存在，并且 token 对这个仓库有推送权限。
- 第一次执行时，工作流会自动创建 `release-assets` 分支。
- 如果同一个 tag 重跑，`muyu/<tag>/` 和 `muyu/latest/` 会被当前构建结果覆盖。

## 对应文件

- 工作流：[.github/workflows/release.yml](/Users/zhaodesen/Desktop/desktop-client/.github/workflows/release.yml)
- Gitee 上传脚本：[scripts/publish-gitee-assets.sh](/Users/zhaodesen/Desktop/desktop-client/scripts/publish-gitee-assets.sh)
