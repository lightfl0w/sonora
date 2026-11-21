use std::process::ExitCode;

use sonora_app::cli::{help, parse, Action};
use sonora_app::{load_project, render_to_wav, summary};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    let action = match parse(args) {
        Ok(a) => a,
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
            Ok(p) => {
                print!("{}", summary(&p));
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
            Ok(p) => match render_to_wav(&p, &out, sample_rate) {
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
        Action::Open { project } => {
            if let Err(e) = load_project(&project) {
                eprintln!("错误：{e}");
                return ExitCode::FAILURE;
            }
            eprintln!("图形界面尚未实现（将在后续版本提供）");
            eprintln!("当前可用：--render、--inspect、--version、--help");
            ExitCode::FAILURE
        }
    }
}
