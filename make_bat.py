exe = "\u5916\u7f6e\u8f6c\u5185\u7f6e.exe"
sample = "\u793a\u4f8b\u914d\u7f6e.ini"
title = "\u5916\u7f6e\u8f6c\u5185\u7f6e"
lines = [
    "@echo off",
    'cd /d "%~dp0"',
    "",
    "echo ============================================",
    "echo   " + title + " - build",
    "echo ============================================",
    "",
    "echo [1/3] cargo build --release",
    "cargo build --release",
    "if errorlevel 1 (",
    "    echo BUILD FAILED",
    "    pause",
    "    exit /b 1",
    ")",
    "",
    "echo [2/3] prepare dist...",
    "if exist dist rmdir /s /q dist",
    "mkdir dist",
    "",
    "echo [3/3] copy files...",
    'copy /y "target\\release\\' + exe + '" "dist\\" >nul',
    'copy /y "README.md" "dist\\" >nul',
    'copy /y "' + sample + '" "dist\\' + exe.replace(".exe", ".ini") + '" >nul',
    "",
    "echo.",
    "echo ============================================",
    "echo   DONE! Output in dist\\",
    "echo ============================================",
    "dir /b dist",
    "pause",
]
with open(r"D:\\PECMD\\wind\\build.bat", "wb") as f:
    f.write("\r\n".join(lines).encode("gbk"))
print("ok")
