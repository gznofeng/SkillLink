fn main() {
    slint_build::compile("ui/app.slint").unwrap();

    // Windows 下把 icon.ico 嵌入 exe 资源（资源管理器/任务栏显示图标）
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.compile().expect("嵌入图标失败");
    }
}
