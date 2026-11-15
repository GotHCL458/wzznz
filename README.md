# 外置转内置（wzznz）

一个基于 **Rust + windui** 的 Windows 图形工具：读取配置文件，把**外置模块**（软件、驱动、脚本、命令）**转为内置** —— 即复制（或解压）到 PE 的 `X:\` 下并按类型执行。

> 适用场景：WinPE / 精简系统制作，把 U 盘或本地磁盘上的外置工具"搬"进内存盘，启动后即为内置状态。

[![Rust](https://img.shields.io/badge/Rust-2021-orange)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows-blue)](#)
[![GUI](https://img.shields.io/badge/GUI-windui%2020-9cf)](#)

---

## 特性

- **单文件静态 exe**（约 8 MB），静态链接 CRT，无运行库依赖
- **纯 Win32/GDI + 软件渲染**（windui），**不依赖 OpenGL / DirectX**，PE 环境兼容性好
- **GUI 勾选式操作**：每项右侧显示绿「可用」/ 红「缺失」/ 蓝「已内置」标记，并汇总占用大小
- **多类型支持**：软件 / 驱动 / bat / cmd / wcs / CMD（直接执行命令行）
- **压缩包自动解压**：`.zip` / `.7z`
- **同名合并**：同一个显示选项可写多行，勾选后按顺序依次执行
- **无人值守**：`--auto` 命令行模式，无界面直接全选转换
- **内嵌资源**：`dwrite.dll`、`PECMD.EXE`、`DrvIndex.exe`、`7z` 编译进 exe，运行时自动释放并做 CRC32 完整性校验
- **编码自适应**：配置文件自动识别 UTF-8（含 BOM）/ GBK；bat/cmd 落地前自动转 GBK + CRLF 并注入 `@chcp 936`
- **断点标记**：已处理项写入注册表 `HKLM\SOFTWARE\WZNZ\Installed`

---

## 快速开始

1. 下载 / 构建 `外置转内置.exe`
2. 将 `外置转内置.ini` 与 exe 放在**同一目录**（可参考 [示例配置.ini](示例配置.ini)）
3. 双击运行
4. 在界面中**勾选**需要的模块 → 点【开始转换】

---

## 配置文件格式

`外置转内置.ini`，每行一个动作，**英文逗号**分隔 5 个字段：

```
类型,源路径,显示选项,目标文件夹,占用大小
```

| 字段 | 必填 | 说明 |
|------|------|------|
| **类型** | 是 | `软件` / `驱动` / `bat` / `cmd` / `wcs` / `CMD` |
| **源路径** | 是 | 文件、文件夹或压缩包路径；`CMD` 类型时此字段为**命令行**本身 |
| **显示选项** | 是 | 界面中显示的勾选项名称 |
| **目标文件夹** | 否 | **绝对路径**；留空则用该类型的默认目录 |
| **占用大小** | 否 | 如 `50MB` / `2GB` / `800KB` / 纯数字（按 KB）；仅**软件/驱动**计入汇总 |

- **注释**：以 `;` 或 `#` 开头的行
- **同名合并**：同一「显示选项」写多行 → 界面合并为**一个**勾选项，勾选后按行的**上下顺序**依次执行；占用大小累加
- **编码**：UTF-8（含 BOM）/ GBK 自动识别

### 示例

```ini
; 软件：文件夹（递归拷贝到 X:\TOOL\Browser）
软件,D:\Apps\Browser,浏览器,,50MB

; 软件：单个文件 + 指定目标目录
软件,D:\Tools\MyApp.exe,我的程序,X:\Tools\MyApp,12MB

; 软件：压缩包（.zip / .7z 自动解压）
软件,D:\Packages\Tools.zip,工具包,,200MB

; 驱动：.inf 直接安装（需管理员权限）
驱动,D:\Drivers\NetCard.inf,网卡驱动,,8MB

; 驱动：压缩包 / 目录，交给 DrvIndex.exe
驱动,D:\Drivers\DriverPack.7z,驱动包,,1.5GB

; 脚本
wcs,D:\Scripts\NetSetup.wcs,网络设置,
bat,D:\Scripts\Clean.bat,系统清理,
cmd,D:\Scripts\Info.cmd,系统信息,

; CMD（大写）：直接执行命令行，不落文件
CMD,start notepad.exe,打开记事本,

; 同名合并：一个勾选项 = 驱动 + 脚本
驱动,D:\Drivers\NetCard.inf,网络支持,,100MB
wcs,D:\Scripts\NetSetup.wcs,网络支持,
```

---

## 各类型的处理动作

| 类型 | 动作 |
|------|------|
| **软件** | 拷贝 / 解压到目标目录（文件夹递归拷贝，`zip`/`7z` 自动解压） |
| **驱动** | `.inf` → 调用 `DiInstallDriverW` 直接安装（不拷贝）；压缩包 / 目录 → 交由同目录 `DrvIndex.exe` |
| **bat** | 拷到目标目录 → 转 GBK+CRLF、注入 `@chcp 936` → `cmd /c` 执行 |
| **cmd** | 同 `bat` |
| **wcs** | 拷到目标目录 → 首行注入 `#code=65001` → 同目录 `PECMD.EXE LOAD` 执行 |
| **CMD** | **直接执行命令行**（不落文件）；`start` 开头直接 spawn，否则 `cmd /c` |

### 默认目标目录（第 4 字段留空时）

| 类型 | 默认目录 |
|------|---------|
| 软件 | `X:\TOOL\` |
| bat / 驱动 | `X:\TOOL\BAT\` |
| cmd | `X:\TOOL\CMD\` |
| wcs | `X:\TOOL\WCS\` |

### 源路径支持的形式

| 形式 | 处理 |
|------|------|
| 单个文件 | 拷贝到目标 |
| 文件夹 | 递归拷贝（保持文件夹名） |
| 压缩包（`.zip` / `.7z`） | 解压到目标（保持包名子目录） |
| 驱动 `.inf` | 直接安装 |
| 驱动压缩包 / 目录 | 交给 `DrvIndex.exe` |

---

## 命令行模式

带 `--auto` 参数运行，**不显示界面**，读取配置、全选并转换（适合无人值守 / 自动化）：

```bat
外置转内置.exe --auto
```

输出示例：

```
[auto] 已加载 8 个选项
[auto] 转换完成，共处理 8 项。
```

---

## 界面说明

| 区域 | 说明 |
|------|------|
| 勾选列表 | 复选框 + 选项名 + 可用性标记（绿「可用」/ 红「缺失」/ 蓝「已内置」）+ 占用大小 |
| 进度条 | 转换进度 |
| 状态栏 | 当前状态；下方汇总「已选 N 项 · 共 X MB」 |
| 按钮 | 全选 / 反选 / 开始转换 / 关于 / 退出 |
| 提示 | 转换完成后弹出 toast |

窗口默认尺寸 460 × 560，可上下滚动列表。

---

## 运行时依赖（同目录文件）

工具会把内嵌资源释放到**自身所在目录**，也优先在该目录查找：

| 文件 | 用途 | 何时需要 |
|------|------|---------|
| `dwrite.dll` | 文字渲染 | 内嵌，PE 缺此 DLL 时释放 |
| `PECMD.EXE` | 执行 `wcs` 类型 | 有 wcs 项时 |
| `DrvIndex.exe` | 安装驱动包 | 有驱动压缩包 / 目录项时 |
| `7z/7z.exe`、`7z/7z.dll` | 解压 `.7z` | 有 7z 压缩包时 |

---

## 从源码构建

环境要求：**Rust 1.75+**（`x86_64-pc-windows-msvc` 工具链）+ Visual Studio Build Tools。

```bash
# 开发构建
cargo build --release
# 产物：target/release/外置转内置.exe

# 一键构建发布包（编译 + 复制 exe / README / 配置到 dist/）
build.bat
```

- 静态链接：`.cargo/config.toml` 中 `+crt-static`
- 图标：`assets/icon.ico`（`build.rs` 用 `winres` 嵌入）
- 内嵌资源：`res/` 目录（`rust-embed`），`build.rs` 生成 CRC32 哈希用于运行时校验
- **修改 `res/` 后需重新构建**，否则哈希校验不通过

### 项目结构

```
wind/
├── .cargo/config.toml      静态链接配置
├── assets/icon.ico         exe 图标
├── res/                    内嵌资源（运行时释放到 exe 同目录）
│   ├── dwrite.dll
│   ├── PECMD.EXE
│   ├── DrvIndex.exe
│   └── 7z/{7z.exe,7z.dll}
├── src/main.rs             全部逻辑
├── build.rs                图标嵌入 + res/ 哈希生成
├── Cargo.toml
├── build.bat               一键构建 → dist/
├── 示例配置.ini            带注释的配置模板
└── README.md
```

---

## 注意事项

1. `外置转内置.ini` 必须与 `外置转内置.exe` **同目录**。
2. 第 4 字段「目标文件夹」填**绝对路径**；留空才用默认 `X:\TOOL\...`。正常 Windows 下没有 `X:` 盘，本地测试请显式指定目标目录。
3. **驱动安装需管理员权限**；`.inf` 走 Win32 API，压缩包 / 目录走 `DrvIndex.exe`。
4. `bat` / `cmd` 若含中文，工具会自动转 GBK 并注入 `@chcp 936`，一般无需手动处理。
5. 在真实 PE 中运行前，请确认 `X:\`（内存盘）有足够空间。
6. 本工具为**纯 GDI 界面**，可在精简 PE 中运行（无需 OpenGL / DirectX）。
7. 频繁运行 `build.bat` 会用 `示例配置.ini` 覆盖 `dist\外置转内置.ini`，注意备份。

---

## 致谢

- [windui](https://crates.io/crates/windui) — Win32/GDI 即时模式 GUI
- [PECMD](http://www.pecmd.net/) — WinPE 脚本解释器
- `rust-embed`、`encoding_rs`、`zip`、`sevenz-rust`、`windows` crate
