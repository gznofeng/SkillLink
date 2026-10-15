<div align="center">

# SkillLink — skill 仓库一键分发

**所有 AI 编程工具的 skills 集中存放，通过 Windows junction 一键分发。**

[![Release](https://img.shields.io/github/v/release/gznofeng/SkillLink?logo=github)](../../releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)
[![Homepage](https://img.shields.io/badge/%E5%9C%A8%E7%BA%BF%E4%BB%8B%E7%BB%8D-gznofeng.github.io%2FSkillLink-8A2BE2?logo=github)](https://gznofeng.github.io/SkillLink/)

[English](README.md) | 简体中文

</div>

---

所有 skill 存放在一个**中央仓库目录**（单一事实源），通过 Windows **junction**（`mklink /J`，无需管理员权限）
一键分发到 Claude Code、CodeBuddy、Codex、Cursor、Gemini CLI、OpenCode 等工具的 skills 目录。

纯 Rust + [Slint](https://slint.dev) 原生 UI，**单个 exe、零运行时依赖**（不需要 WebView2 / .NET），
Windows 10/11 x64 解压即用。

## 界面预览

**Skills** —— 左侧技能列表，右侧四态链接状态 + 勾选部署：

![Skills 页](screenshot/skills.png)

**配置** —— 中央仓库路径与 AI 工具目录管理：

![配置页](screenshot/setup.png)

## 功能

### 配置
- **skill 仓库路径**：设置中央仓库目录（可手动输入或点击文件夹图标选择）
- **AI 工具目录**：各工具 skills 目录的增删改、启用/停用，内置主流工具默认值
- 配置持久化于 `%USERPROFILE%\.skill-client\config.json`

### Skills
- 自动扫描仓库目录下每个含 `SKILL.md` 的直接子目录，解析 frontmatter 的 `name` / `description`
- 每个 skill × 每个 target 的链接状态四态检测：

| 状态 | 含义 |
|------|------|
| NotLinked | 目标目录中不存在该 skill |
| Linked | 是 junction 且指向中央仓库对应 skill |
| Conflict | 目标处已存在真实目录/文件 |
| Broken | 是链接但指向失效 |

- **部署**：创建 junction；冲突时默认备份为 `xxx.bak` 后覆盖
- **移除链接**：仅删除链接本身，绝不穿透删除真实目录
- **全部部署**：所有 skill × 所有启用 target 一键分发

## 下载使用

[![下载](https://img.shields.io/badge/下载-SkillLink--v0.1.0--win64.zip-blue?logo=github)](https://github.com/gznofeng/SkillLink/releases/download/v0.1.0/SkillLink-v0.1.0-win64.zip)

直链下载：**[SkillLink-v0.1.0-win64.zip](https://github.com/gznofeng/SkillLink/releases/download/v0.1.0/SkillLink-v0.1.0-win64.zip)** —— 解压后双击 `skill-link.exe` 即可。历史版本见 [Releases](../../releases) 页面。

项目介绍页：**https://gznofeng.github.io/SkillLink/**

## 从源码构建

依赖：Rust 1.85+（2024 edition）

```powershell
git clone https://github.com/gznofeng/SkillLink.git
cd SkillLink2

# 开发运行
cargo run -p skill-link-app

# 打包（自动读取 app/Cargo.toml 版本号，产物输出到 release/）
powershell -File scripts\package.ps1
```

## 打包脚本说明

`scripts/package.ps1` 完成以下工作：

1. `cargo build --release`（LTO + strip 优化）
2. 将 `skill-link.exe` + `使用说明.txt` 组装到 `package/SkillLink-v<版本>/`
3. 压缩为 `release/SkillLink-v<版本>-win64.zip`（按版本归档，保留历史版本）

发布新版本时：修改 `app/Cargo.toml` 的 `version` → 运行打包脚本 → 上传 `release/` 下对应 zip。
也可以直接 push `v*` tag —— [GitHub Actions](.github/workflows/release.yml) 会自动构建并发布 Release。

## 项目结构

```
├── app/            # Slint 原生 UI 桌面应用
│   ├── src/main.rs        # 应用层（模型组装、回调、对话框）
│   └── ui/app.slint       # 界面定义
├── core/           # 核心逻辑（纯 Rust 库，与 UI 解耦，含完整单元测试）
│   └── src/
│       ├── config.rs      # 配置读写（~/.skill-client/config.json）
│       ├── links.rs       # junction 四态检测 / 部署 / 移除 / 批量分发
│       └── skills.rs      # skill 扫描与 frontmatter 解析
├── scripts/package.ps1    # 打包脚本
├── screenshot/            # 界面截图（README 引用）
├── release/               # 版本化发行包（脚本生成，不入库）
└── package/使用说明.txt    # 随包分发的说明文档
```

## 设计要点

- **安全语义**：unlink 仅当目标是 junction 时用 `fs::remove_dir` 删除链接本身；
  真实目录拒绝删除；中央仓库永不受影响
- **冲突策略**：部署时若目标处已有真实目录，默认改名 `.bak` 备份（重名追加时间戳）
- **junction 创建**：`cmd /c mklink /J` 逐参数传参，引号由 std 保证；cmd 中文输出按 GBK 解码避免乱码
- **图标**：`icon.ico` 通过 `winresource` 嵌入 exe 资源，窗口图标使用 `ui/icon.png`
- **渲染**：Slint 软件渲染器（`winit-software`），文字抗锯齿质量好且无 GPU 依赖

## 范围外

- macOS / Linux 支持
- skill 内容编辑器、在线安装（占位）、自动更新、代码签名

## 许可证

[MIT](LICENSE)
