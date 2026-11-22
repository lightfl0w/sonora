use gpui::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("sonora {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Sonora {} —— 基于泛音的轻量 DAW", env!("CARGO_PKG_VERSION"));
        println!();
        println!("用法：sonora [工程文件]");
        println!();
        println!("  <工程文件>   打开 .sonora 工程");
        println!("  --version    显示版本");
        println!("  --help       显示本帮助");
        return;
    }
    let path = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(std::path::PathBuf::from);
    let (project, path) = match path {
        Some(path) => match Project::load(&path) {
            Ok(project) => {
                println!("已载入 {}", path.display());
                (project, Some(path))
            }
            Err(e) => {
                eprintln!("无法载入 {}：{e}", path.display());
                std::process::exit(1);
            }
        },
        None => (Project::default(), None),
    };
    gpui_platform::application().run(move |cx| {
        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(900.0), px(560.0))),
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..sonora_ui::titlebar::TitleBar::window_options()
        };
        cx.open_window(options, |_window, cx| {
            cx.new(|cx| {
                let mut app = SonoraApp::new(project, cx);
                if let Some(path) = path {
                    app = app.with_path(path);
                }
                app.enable_default_voice_persistence();
                app.start_audio();
                app
            })
        })
        .expect("无法创建窗口");
    });
}
