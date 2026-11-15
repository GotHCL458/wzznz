use std::io::{self, Read, Write};
use winres::WindowsResource;

// 简单 CRC32（无需外部依赖）
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

fn main() -> io::Result<()> {
    // 嵌入图标
    WindowsResource::new()
        .set_icon("assets/icon.ico")
        .compile()?;

    // 遍历 res/ 算每个文件的 CRC32，生成常量
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = std::path::Path::new(&out_dir).join("res_hashes.rs");
    let mut f = std::fs::File::create(&dest)?;

    writeln!(f, "// 自动生成：内嵌资源哈希")?;
    writeln!(f, "pub const RES_HASHES: &[(&str, u32)] = &[")?;

    fn walk(dir: &std::path::Path, base: &std::path::Path, f: &mut std::fs::File) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                walk(&p, base, f)?;
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                let mut buf = Vec::new();
                std::fs::File::open(&p)?.read_to_end(&mut buf)?;
                let h = crc32(&buf);
                writeln!(f, "    (\"{}\", {:#010x}),", rel, h)?;
            }
        }
        Ok(())
    }

    let res_dir = std::path::Path::new("res");
    if res_dir.is_dir() {
        walk(res_dir, res_dir, &mut f)?;
    }
    writeln!(f, "];")?;

    println!("cargo:rerun-if-changed=res");
    Ok(())
}
