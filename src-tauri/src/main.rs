// 禁用 Windows 终端控制台黑框 (无论是 debug 还是 release 均保持无缝原生桌面体验)
#![windows_subsystem = "windows"]

fn main() {
    glint_lib::run()
}
