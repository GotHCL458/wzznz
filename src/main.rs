#![windows_subsystem = "windows"]

use windui::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::os::windows::process::CommandExt;

// 强制子进程新建控制台/窗口（GUI 程序 spawn 时默认不弹窗）
const CREATE_NEW_CONSOLE: u32 = 0x00000010;

// 内嵌 dwrite.dll（PE 缺此 dll 时释放到 exe 同目录）
#[derive(rust_embed::RustEmbed)]
#[folder = "res"]
struct Res;

// 内嵌资源的预期哈希（build.rs 生成）
include!(concat!(env!("OUT_DIR"), "/res_hashes.rs"));

static CFG_DIR: OnceLock<PathBuf> = OnceLock::new();

// 默认目标目录
const TARGET: &str = "X:\\TOOL";
const BATDIR: &str = "X:\\TOOL\\BAT";
const CMDDIR: &str = "X:\\TOOL\\CMD";
const WCSDIR: &str = "X:\\TOOL\\WCS";

#[derive(Clone)]
struct OptItem {
    name: String,
    actions: Vec<(String, String, String)>, // (类型, 源相对路径, 目标)
    ok: bool,     // 源是否存在
    size_kb: u64, // 声明的占用大小（KB）
}

fn main() {
    // 反调试：检测到调试器则警告并退出
    if is_debugged() {
        anti_debug_warn();
        std::process::exit(0xDEAD);
    }

    // 释放内嵌资源（dwrite.dll / PECMD.EXE / DrvIndex.exe / 7z）到 exe 同目录
    release_assets();

    // 完整性校验：释放的资源哈希须与内嵌记录一致，否则弹窗退出
    if !verify_assets() {
        tamper_warn();
        std::process::exit(0xBADC0DE);
    }

    // --auto 命令行模式：无 GUI，直接全选并转换（用于自动化测试/无人值守）
    if std::env::args().any(|a| a == "--auto") {
        auto_run();
        return;
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = CFG_DIR.set(exe_dir.clone());

    // 读取配置
    let ini = exe_dir.join("外置转内置.ini");
    let (items, load_msg) = load_config(&ini, &exe_dir);

    // 每个选项一个 bool 信号
    let checks: Vec<Signal<bool>> = items.iter().map(|_| signal(false)).collect();
    let checks_for_list = checks.clone();
    let items_sig = signal(items.clone());
    let status = signal(load_msg);
    let progress = signal(0.0f32);
    let total_size = signal(String::from("已选 0 项 · 共 0 KB"));

    // 全选/反选/开始 按钮的闭包需要访问 checks
    let checks_select = checks.clone();
    let checks_invert = checks.clone();

    // 构建列表
    let items_for_rows = items_sig.clone();
    let checks_for_toggle = checks.clone();
    let total_for_toggle = total_size.clone();
    let items_for_toggle = items_sig.clone();
    let list = Element::list_signal(
        items_sig.clone(),
        |it: &OptItem| it.name.clone(),
        move |it: OptItem| {
            // 找到该行的索引对应的 check 信号
            let idx = items_for_rows
                .get()
                .iter()
                .position(|x| x.name == it.name)
                .unwrap_or(0);
            let chk = checks_for_list
                .get(idx)
                .cloned()
                .unwrap_or_else(|| signal(true));
            let ok_label = if it.ok { "可用" } else { "缺失" };
            let ok_role = if it.ok { Role::Success } else { Role::Danger };
            let inst = is_installed(&it.name);
            let inst_label = if inst { "已内置" } else { "未内置" };
            let inst_role = if inst { Role::Success } else { Role::TextMuted };
            let cks = checks_for_toggle.clone();
            let tot = total_for_toggle.clone();
            let its = items_for_toggle.clone();
            Element::row()
                .width_match()
                .height(34)
                .cross(Align::Center)
                .spacing(8)
                .child(
                    Element::checkbox(it.name.clone(), chk)
                        .weight(1.0)
                        .on_toggle({
                            let chk2 = chk.clone();
                            move |_| {
                                // 手动翻转（设了 on_toggle 会取代默认翻转）
                                chk2.set(!chk2.get());
                                // 重算勾选项总大小
                                let data = its.get();
                                let mut sum: u64 = 0;
                                let mut cnt = 0;
                                for (i, item) in data.iter().enumerate() {
                                    if cks.get(i).map(|c| c.get()).unwrap_or(false) {
                                        sum += item.size_kb;
                                        cnt += 1;
                                    }
                                }
                                tot.set(format!("已选 {} 项 · 共 {}", cnt, format_size(sum)));
                            }
                        }),
                )
                .child(
                    Element::label(ok_label)
                        .font_size(12.0)
                        .fg_role(ok_role)
                        .width(48),
                )
                .child(
                    Element::label(inst_label)
                        .font_size(12.0)
                        .fg_role(inst_role)
                        .width(56),
                )
                .child(
                    Element::label(if it.size_kb > 0 { format_size(it.size_kb) } else { String::new() })
                        .font_size(12.0)
                        .fg_role(Role::TextMuted)
                        .width(72),
                )
        },
    );

    let ui = Element::col()
        .fill()
        .bg(Color::hex(0xF5F6FA))
        .padding(16)
        .spacing(10)
        .child(
            Element::label("外置转内置 工具")
                .font_size(20.0)
                .fg(Color::hex(0x2C3E50))
                .height(28)
                .width_match(),
        )
        .child(
            Element::label("勾选要转为内置的模块")
                .font_size(12.0)
                .fg_role(Role::TextMuted)
                .height(18)
                .width_match(),
        )
        .child(list.weight(1.0).bg(Color::WHITE).corner(6.0))
        .child(Element::progress(progress.clone()).width_match())
        .child(
            Element::label(status.clone())
                .font_size(12.0)
                .fg_role(Role::TextMuted)
                .height(20)
                .width_match(),
        )
        .child(
            Element::label_signal(total_size.clone())
                .font_size(12.0)
                .fg_role(Role::TextMuted)
                .height(20)
                .width_match(),
        )
        .child(
            Element::row()
                .width_match()
                .height(38)
                .spacing(8)
                .child(Element::button("全选").on_click({
                    let cks = checks_select.clone();
                    let its = items_sig.clone();
                    let tot = total_size.clone();
                    move |_| {
                        for c in &cks {
                            c.set(true);
                        }
                        recalc_total(&its, &cks, &tot);
                    }
                }))
                .child(Element::button("反选").on_click({
                    let cks = checks_invert.clone();
                    let its = items_sig.clone();
                    let tot = total_size.clone();
                    move |_| {
                        for c in &cks {
                            c.set(!c.get());
                        }
                        recalc_total(&its, &cks, &tot);
                    }
                }))
                .child(
                    Element::button("开始转换")
                        .intent(Intent::Primary)
                        .on_click({
                            let checks = checks.clone();
                            let items = items_sig.clone();
                            let status = status.clone();
                            let progress = progress.clone();
                            move |ctx| {
                                run_convert(&items.get(), &checks, &status, &progress);
                                // 重设列表数据以刷新"已内置"标记
                                let cur = items.get();
                                items.set(cur.clone());
                                let done = status.get();
                                if done.contains("完成") {
                                    ctx.toast_ok(&done);
                                } else {
                                    ctx.toast(&done);
                                }
                            }
                        }),
                )
                .child(Element::button("关于").on_click(|ctx| {
                    ctx.open_window(
                        Window::new("关于", 400, 320)
                            .resizable(false)
                            .centered(true)
                            .single("about")
                            .content(about_page),
                    );
                }))
                .child(Element::button("退出").on_click(|_| std::process::exit(0))),
        );

    App::new("外置转内置", 460, 560)
        .bg(Color::hex(0xF5F6FA))
        .content(ui)
        .run();
}

// ============ 释放内嵌 dwrite.dll ============
fn release_assets() {
    if let Some(exe) = std::env::current_exe().ok() {
        if let Some(dir) = exe.parent() {
            // 遍历内嵌资源，释放到 exe 同目录（保留子目录结构）
            for path in Res::iter() {
                let target = dir.join(path.as_ref());
                if target.exists() {
                    continue;
                }
                if let Some(content) = Res::get(path.as_ref()) {
                    if let Some(parent) = target.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&target, content.data.as_ref());
                }
            }
        }
    }
}

// ============ 读取配置 ============
fn load_config(ini: &Path, base: &Path) -> (Vec<OptItem>, String) {
    if !ini.exists() {
        return (Vec::new(), format!("未找到配置文件：{}", ini.display()));
    }
    let bytes = match std::fs::read(ini) {
        Ok(b) => b,
        Err(e) => return (Vec::new(), format!("读取失败：{}", e)),
    };
    let text = decode_auto(&bytes);
    let mut items: Vec<OptItem> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            continue;
        }
        let typ = parts[0].trim().to_string();
        let src = parts[1].trim().to_string();
        let name = parts[2].trim().to_string();
        let dst = if parts.len() >= 4 { parts[3].trim().to_string() } else { String::new() };
        // 第 5 参数：占用大小（可空），如 50MB / 2GB / 800KB / 数字(KB)
        // 第 5 参数（仅软件/驱动计入大小）
        let size_kb = if (typ == "软件" || typ == "驱动") && parts.len() >= 5 {
            parse_size_kb(parts[4].trim())
        } else {
            0
        };
        if name.is_empty() || src.is_empty() {
            continue;
        }
        let ok = if typ == "CMD" { true } else { resolve(base, &src).exists() };
        if let Some(it) = items.iter_mut().find(|x| x.name == name) {
            it.actions.push((typ, src, dst));
            it.ok = it.ok && ok;
            it.size_kb += size_kb; // 累加（仅软件/驱动）
        } else {
            items.push(OptItem { name, actions: vec![(typ, src, dst)], ok, size_kb });
        }
    }
    let msg = format!("已加载 {} 个选项", items.len());
    (items, msg)
}

// 解析大小字符串为 KB：50MB→51200, 2GB→2097152, 800KB→800, 500→500(KB)
fn parse_size_kb(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let up = s.to_uppercase();
    if let Some(n) = up.strip_suffix("GB") {
        n.trim().parse::<f64>().map(|v| (v * 1024.0 * 1024.0) as u64).unwrap_or(0)
    } else if let Some(n) = up.strip_suffix("MB") {
        n.trim().parse::<f64>().map(|v| (v * 1024.0) as u64).unwrap_or(0)
    } else if let Some(n) = up.strip_suffix("KB") {
        n.trim().parse::<f64>().map(|v| v as u64).unwrap_or(0)
    } else {
        // 纯数字按 KB
        up.parse::<f64>().map(|v| v as u64).unwrap_or(0)
    }
}

// 格式化大小为易读字符串
fn format_size(kb: u64) -> String {
    if kb == 0 {
        "0 KB".to_string()
    } else if kb >= 1024 * 1024 {
        format!("{:.2} GB", kb as f64 / (1024.0 * 1024.0))
    } else if kb >= 1024 {
        format!("{:.2} MB", kb as f64 / 1024.0)
    } else {
        format!("{} KB", kb)
    }
}

// CMD 类型：直接执行命令行（start → 直接 spawn，其他 → cmd /c）
fn dispatch_cmd(cmdline: &str) {
    let trimmed = cmdline.trim_start();
    let lower = trimmed.to_lowercase();
    if lower.starts_with("start") {
        let mut prog = trimmed[5..].trim_start();
        if prog.starts_with("\"\"") {
            prog = prog[2..].trim_start();
        }
        let _ = std::process::Command::new(prog).spawn();
    } else {
        let _ = std::process::Command::new("cmd")
            .arg("/c").arg(cmdline)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn();
    }
}

// "关于"独立窗口内容
fn about_page() -> Element {
    Element::col()
        .fill()
        .padding(24)
        .spacing(10)
        .cross(windui::spec::Align::Center)
        .child(Element::stack().size(72, 72).icon_bytes(include_bytes!("../assets/icon.ico")))
        .child(Element::label("外置转内置").font_size(22.0).font_weight(700))
        .child(Element::label("版本 1.0.0").font_size(13.0).fg_role(Role::TextMuted))
        .child(Element::label("作者：GotHCL458").font_size(13.0))
        .child(Element::label("将外置模块（软件/驱动/脚本/命令）转为内置").font_size(13.0).fg_role(Role::TextMuted))
        .child(Element::label("© 2026 GotHCL458").font_size(12.0).fg_role(Role::TextSubtle))
        .child(Element::flex_spacer())
        .child(Element::button("关闭").on_click(|ctx| ctx.request_close()))
}

// 简单 CRC32（与 build.rs 一致）
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// 校验释放的资源：每个文件的 CRC32 须等于内嵌记录
fn verify_assets() -> bool {
    let dir = match std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        Some(d) => d,
        None => return true,
    };
    for (rel, expected) in RES_HASHES {
        let p = dir.join(rel);
        let bytes = match std::fs::read(&p) {
            Ok(b) => b,
            Err(_) => return false,
        };
        if crc32(&bytes) != *expected {
            return false;
        }
    }
    true
}

// 完整性校验失败提示
fn tamper_warn() {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let text = HSTRING::from("程序完整性校验失败（资源被篡改），程序将退出。");
    let title = HSTRING::from("安全警告");
    unsafe {
        MessageBoxW(None, &text, &title, MB_OK | MB_ICONERROR);
    }
}

// 反调试检测：IsDebuggerPresent + CheckRemoteDebuggerPresent
fn is_debugged() -> bool {
    use windows::Win32::System::Diagnostics::Debug::{CheckRemoteDebuggerPresent, IsDebuggerPresent};
    use windows::Win32::Foundation::*;
    unsafe {
        if IsDebuggerPresent().as_bool() {
            return true;
        }
        let mut present = BOOL(0);
        let me = windows::Win32::System::Threading::GetCurrentProcess();
        let _ = CheckRemoteDebuggerPresent(me, &mut present);
        present.as_bool()
    }
}

// 检测到调试器时的警告（Win32 MessageBoxW）
fn anti_debug_warn() {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let text = HSTRING::from("检测到调试器，程序将退出。");
    let title = HSTRING::from("安全警告");
    unsafe {
        MessageBoxW(None, &text, &title, MB_OK | MB_ICONERROR);
    }
}

// 弹出"关于"对话框（Win32 MessageBoxW）
fn show_about() {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
    let text = "外置转内置  工具\n\
                版本：1.0.0\n\
                作者：GotHCL458\n\
                说明：将外置模块（软件/驱动/脚本/命令）转为内置\n\
                版权：© 2026 GotHCL458";
    let text_w = HSTRING::from(text);
    let title_w = HSTRING::from("关于");
    unsafe {
        MessageBoxW(None, &text_w, &title_w, MB_OK | MB_ICONINFORMATION);
    }
}

// 读取某选项是否已内置（HKLM\SOFTWARE\WZZNZ\Installed\<name> 存在则为真）
fn is_installed(name: &str) -> bool {
    use windows::Win32::System::Registry::*;
    unsafe {
        let mut hkey = HKEY::default();
        let sub = format!("SOFTWARE\\WZZNZ\\Installed");
        let sub_w: Vec<u16> = sub.encode_utf16().chain(std::iter::once(0)).collect();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(sub_w.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        ).is_err() {
            return false;
        }
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut ty = REG_NONE;
        let r = RegQueryValueExW(
            hkey,
            windows::core::PCWSTR(name_w.as_ptr()),
            None,
            Some(&mut ty),
            None,
            None,
        );
        let _ = RegCloseKey(hkey);
        r.is_ok()
    }
}

// 标记某选项为已内置
fn mark_installed(name: &str) {
    use windows::Win32::System::Registry::*;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    unsafe {
        let sub = "SOFTWARE\\WZZNZ\\Installed";
        let sub_w: Vec<u16> = sub.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = HKEY::default();
        let rc = RegCreateKeyW(
            HKEY_LOCAL_MACHINE,
            windows::core::PCWSTR(sub_w.as_ptr()),
            &mut hkey,
        );
        if rc != windows::Win32::Foundation::ERROR_SUCCESS {
            return;
        }
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let data: u32 = 1;
        let _ = RegSetValueExW(
            hkey,
            windows::core::PCWSTR(name_w.as_ptr()),
            0,
            REG_DWORD,
            Some(&data.to_le_bytes()),
        );
        let _ = RegCloseKey(hkey);
    }
}

// 检测文本编码：返回 (是否UTF8, 是否有BOM)
fn detect_encoding(bytes: &[u8]) -> (bool, bool) {
    // UTF-8 BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (true, true);
    }
    // UTF-16 BOM → 不处理（罕见）
    // 尝试 UTF-8 解码
    if std::str::from_utf8(bytes).is_ok() {
        return (true, false);
    }
    // 回退 GBK
    (false, false)
}

// 确保 wcs 首行有 #code 编码声明（UTF-8→65001，GBK→936），不改变原编码
fn inject_wcs_code(path: &Path) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let (is_utf8, _) = detect_encoding(&bytes);
    // 若已有 #code 声明则不处理
    let head_text = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
    if head_text.contains("#code=") {
        return;
    }
    let code_line: &str = if is_utf8 { "#code=65001\r\n" } else { "#code=936\r\n" };
    let mut out = Vec::with_capacity(bytes.len() + code_line.len() + 3);
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // 有 BOM：BOM + code 声明 + 原内容
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        out.extend_from_slice(code_line.as_bytes());
        out.extend_from_slice(&bytes[3..]);
    } else {
        out.extend_from_slice(code_line.as_bytes());
        out.extend_from_slice(&bytes);
    }
    let _ = std::fs::write(path, &out);
}

// 重算勾选项总大小并更新显示
fn recalc_total(items: &Signal<Vec<OptItem>>, checks: &[Signal<bool>], total: &Signal<String>) {
    let data = items.get();
    let mut sum: u64 = 0;
    let mut cnt = 0;
    for (i, item) in data.iter().enumerate() {
        if checks.get(i).map(|c| c.get()).unwrap_or(false) {
            sum += item.size_kb;
            cnt += 1;
        }
    }
    total.set(format!("已选 {} 项 · 共 {}", cnt, format_size(sum)));
}

// 把 bat/cmd 副本转为 GBK 编码，并在首行插入 @chcp 936（确保 cmd 用 GBK 读）
fn inject_chcp(path: &Path) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return,
    };
    let (is_utf8, _) = detect_encoding(&bytes);
    // 解码为字符串（去 BOM）
    let s = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { &bytes[..] };
    let text = if is_utf8 {
        match std::str::from_utf8(s) {
            Ok(t) => t.to_string(),
            Err(_) => {
                let (t, _, _) = encoding_rs::GBK.decode(&bytes);
                t.into_owned()
            }
        }
    } else {
        let (t, _, _) = encoding_rs::GBK.decode(&bytes);
        t.into_owned()
    };
    // 首行插入 @chcp 936（若未插入过）
    let with_chcp = if text.trim_start().to_lowercase().starts_with("@chcp") {
        text
    } else {
        format!("@chcp 936 >nul\r\n{}", text)
    };
    // 统一换行为 CRLF（Windows bat/cmd 必须 CRLF，否则 @echo off 失效）
    let normalized = with_chcp.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n");
    // 统一 GBK 编码保存
    let (gbk, _, _) = encoding_rs::GBK.encode(&normalized);
    let _ = std::fs::write(path, &gbk);
}

// 带重试的拷贝/解压：失败等 500ms 重试，最多 3 次
fn copy_with_retry(src: &Path, dst_dir: &Path) -> Result<(), String> {
    let mut last_err = String::new();
    for attempt in 1..=3 {
        match copy_or_extract(src, dst_dir) {
            Ok(_) => return Ok(()),
            Err(e) => {
                last_err = e.to_string();
                if attempt < 3 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                }
            }
        }
    }
    Err(last_err)
}

fn copy_or_extract(src: &Path, dst_dir: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        let bn = src.file_name().unwrap_or_default();
        copy_dir(src, &dst_dir.join(bn))
    } else {
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "zip" => {
                let bn = src.file_stem().unwrap_or_default();
                extract_zip(src, &dst_dir.join(bn))
            }
            "7z" => {
                let bn = src.file_stem().unwrap_or_default();
                let target = dst_dir.join(bn);
                let _ = std::fs::create_dir_all(&target);
                sevenz_rust::decompress_file(src, &target)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
            }
            _ => std::fs::copy(src, dst_dir.join(src.file_name().unwrap_or_default())).map(|_| ()),
        }
    }
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let p = e.path();
        let t = dst.join(e.file_name());
        if p.is_dir() { copy_dir(&p, &t)?; } else { std::fs::copy(&p, &t)?; }
    }
    Ok(())
}

fn extract_zip(src: &Path, dst: &Path) -> std::io::Result<()> {
    let f = std::fs::File::open(src)?;
    let mut a = zip::ZipArchive::new(f)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    a.extract(dst)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn install_driver(inf: &Path) -> std::io::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{DiInstallDriverW, DIINSTALLDRIVER_FLAGS};
    let p = HSTRING::from(inf.as_os_str());
    let mut reboot = windows::Win32::Foundation::BOOL(0);
    unsafe {
        let _ = DiInstallDriverW(None, &p, DIINSTALLDRIVER_FLAGS(0), Some(&mut reboot));
    }
    Ok(())
}
 


fn resolve(base: &Path, rel: &str) -> PathBuf {
    let p = PathBuf::from(rel);
    if p.is_absolute() { p } else { base.join(p) }
}

fn decode_auto(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (s, _, _) = encoding_rs::UTF_8.decode(&bytes[3..]);
        return s.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (s, _, _) = encoding_rs::GBK.decode(bytes);
    s.into_owned()
}

fn pecmd_path() -> PathBuf {
    if let Some(dir) = CFG_DIR.get() {
        let p = dir.join("PECMD.EXE");
        if p.exists() { return p; }
    }
    PathBuf::from("PECMD.EXE")
}

fn drvindex_path() -> PathBuf {
    if let Some(dir) = CFG_DIR.get() {
        let p = dir.join("DrvIndex.exe");
        if p.exists() { return p; }
    }
    PathBuf::from("DrvIndex.exe")
}

fn run_convert(items: &[OptItem], checks: &[Signal<bool>], status: &Signal<String>, progress: &Signal<f32>) {
    let base = CFG_DIR.get().cloned().unwrap_or_else(|| PathBuf::from("."));
    let selected: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(i, _)| checks.get(*i).map(|c| c.get()).unwrap_or(false))
        .map(|(i, _)| i)
        .collect();
    if selected.is_empty() {
        status.set("未勾选任何选项".into());
        return;
    }
    let total = selected.len();
    for (n, &idx) in selected.iter().enumerate() {
        let it = &items[idx];
        status.set(format!("正在处理：{}（{}/{}）", it.name, n + 1, total));
        progress.set((n as f32) / (total as f32));
        for (typ, rel, dst) in &it.actions {
            if typ == "CMD" {
                dispatch_cmd(rel);
                continue;
            }
            let full = resolve(&base, rel);
            if !full.exists() {
                continue;
            }
            dispatch(typ, &full, dst);
        }
        mark_installed(&it.name);
    }
    progress.set(1.0);
    status.set(format!("转换完成，共处理 {} 项。", total));
}

fn auto_run() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let _ = CFG_DIR.set(exe_dir.clone());
    let ini = exe_dir.join("外置转内置.ini");
    let (items, msg) = load_config(&ini, &exe_dir);
    println!("[auto] {}", msg);
    if items.is_empty() {
        eprintln!("[auto] 无可转换项，退出");
        std::process::exit(2);
    }
    let checks: Vec<Signal<bool>> = items.iter().map(|_| signal(true)).collect();
    let status = signal(String::new());
    let progress = signal(0.0f32);
    run_convert(&items, &checks, &status, &progress);
    println!("[auto] {}", status.get());
    std::process::exit(0);
}

fn dispatch(typ: &str, full: &Path, dst_override: &str) {
    let default_dir = match typ {
        "软件" => TARGET,
        "bat" => BATDIR,
        "cmd" => CMDDIR,
        "wcs" => WCSDIR,
        "驱动" => BATDIR,
        _ => TARGET,
    };
    let dst_dir = if dst_override.is_empty() {
        PathBuf::from(default_dir)
    } else {
        PathBuf::from(dst_override)
    };
    let _ = std::fs::create_dir_all(&dst_dir);

    match typ {
        "软件" => { let _ = copy_with_retry(full, &dst_dir); }
        "驱动" => {
            let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            if ext == "inf" {
                let _ = install_driver(full);
            } else {
                let drv = drvindex_path();
                let _ = std::process::Command::new(&drv)
                    .arg("-b").arg(full).arg("-hide")
                    .current_dir(CFG_DIR.get().map(|p| p.as_path()).unwrap_or(Path::new(".")))
                    .spawn();
            }
        }
        "wcs" => {
            let _ = copy_with_retry(full, &dst_dir);
            let d = dst_dir.join(full.file_name().unwrap_or_default());
            // 确保 wcs 首行有 #code 编码声明（否则 PECMD 按默认编码读会乱码）
            inject_wcs_code(&d);
            let pecmd = pecmd_path();
            let _ = std::process::Command::new(&pecmd)
                .arg("LOAD").arg(&d)
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn();
        }
        "bat" | "cmd" => {
            let _ = copy_with_retry(full, &dst_dir);
            let d = dst_dir.join(full.file_name().unwrap_or_default());
            inject_chcp(&d);
            let _ = std::process::Command::new("cmd")
                .arg("/c").arg(&d)
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn();
        }
        _ => {}
    }
}

