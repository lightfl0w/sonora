use std::path::PathBuf;
use std::process::ExitCode;

use gpui::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use sonora_app::cli::{Action, help, parse};
use sonora_app::{load_project, render_to_wav, summary};
use sonora_project::Project;
use sonora_ui::app::SonoraApp;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let action = match parse(args) {
        Ok(action) => action,
        Err(e) => {
            eprintln!("错误：{e}");
            eprintln!();
            eprintln!("{}", help());
            return ExitCode::from(2);
        }
    };

    match action {
        Action::Help => {
            print!("{}", help());
            ExitCode::SUCCESS
        }
        Action::Version => {
            println!("{} {}", sonora_app::cli::NAME, sonora_app::cli::VERSION);
            ExitCode::SUCCESS
        }
        Action::Inspect { project } => match load_project(&project) {
            Ok(project) => {
                print!("{}", summary(&project));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("错误：{e}");
                ExitCode::FAILURE
            }
        },
        Action::Render {
            project,
            out,
            sample_rate,
        } => match load_project(&project) {
            Ok(project) => match render_to_wav(&project, &out, sample_rate) {
                Ok(frames) => {
                    let secs = frames as f32 / sample_rate as f32;
                    println!(
                        "已渲染 {:.1} 秒到 {}（{} Hz，{} 帧）",
                        secs,
                        out.display(),
                        sample_rate,
                        frames
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("错误：{e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("错误：{e}");
                ExitCode::FAILURE
            }
        },
        Action::Open { project } => run_gui(project),
    }
}

fn run_gui(path: Option<PathBuf>) -> ExitCode {
    let project = match &path {
        Some(path) => match load_project(path) {
            Ok(project) => {
                println!("已载入 {}", path.display());
                project
            }
            Err(e) => {
                eprintln!("无法载入 {}：{e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => Project::default(),
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
                if let Some(path) = &path {
                    app = app.with_path(path.clone());
                }
                app.enable_default_voice_persistence();
                app.start_audio();
                app
            })
        })
        .expect("无法创建窗口");
    });

    ExitCode::SUCCESS
}
