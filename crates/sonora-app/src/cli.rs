use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = "sonora";

pub enum Action {
    Help,
    Version,
    Open {
        project: Option<PathBuf>,
    },
    Render {
        project: PathBuf,
        out: PathBuf,
        sample_rate: u32,
    },
    Inspect {
        project: PathBuf,
    },
}

pub enum CliError {
    UnknownFlag(String),
    MissingValue(&'static str),
    NoInput,
    InvalidNumber { flag: &'static str, value: String },
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::UnknownFlag(flag) => write!(f, "未知参数 {flag}"),
            CliError::MissingValue(flag) => write!(f, "参数 {flag} 缺少取值"),
            CliError::NoInput => write!(f, "未指定工程文件"),
            CliError::InvalidNumber { flag, value } => {
                write!(f, "参数 {flag} 的值 {value} 不是有效数字")
            }
        }
    }
}

pub fn help() -> String {
    format!(
        "{NAME} {VERSION}
基于正弦波加法合成的音乐工作站。

用法:
  {NAME} [工程文件]                      打开图形界面（省略则新建空白工程）
  {NAME} --render <工程文件> [-o 输出]   离线渲染为 WAV
  {NAME} --inspect <工程文件>            打印工程概要
  {NAME} --version                       显示版本
  {NAME} --help                          显示本帮助

选项:
  -o, --out <路径>     输出文件（默认与工程同名的 .wav）
  -r, --rate <Hz>      采样率（默认 44100）
"
    )
}

pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Action, CliError> {
    let mut args = args.into_iter().skip(1).peekable();

    let mut render = false;
    let mut inspect = false;
    let mut input: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut rate: u32 = 44100;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(Action::Help),
            "--version" | "-V" => return Ok(Action::Version),
            "--render" => render = true,
            "--inspect" => inspect = true,
            "-o" | "--out" => {
                let value = args.next().ok_or(CliError::MissingValue("--out"))?;
                out = Some(PathBuf::from(value));
            }
            "-r" | "--rate" => {
                let value = args.next().ok_or(CliError::MissingValue("--rate"))?;
                rate = value.parse().map_err(|_| CliError::InvalidNumber {
                    flag: "--rate",
                    value,
                })?;
                if !(8000..=192_000).contains(&rate) {
                    return Err(CliError::InvalidNumber {
                        flag: "--rate",
                        value: rate.to_string(),
                    });
                }
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(CliError::UnknownFlag(other.to_string()));
            }
            other => {
                if input.is_some() {
                    return Err(CliError::UnknownFlag(other.to_string()));
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    if inspect {
        let project = input.ok_or(CliError::NoInput)?;
        return Ok(Action::Inspect { project });
    }

    if render {
        let project = input.ok_or(CliError::NoInput)?;
        let out = out.unwrap_or_else(|| project.with_extension("wav"));
        return Ok(Action::Render {
            project,
            out,
            sample_rate: rate,
        });
    }

    Ok(Action::Open { project: input })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Action, CliError> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn help_and_version() {
        assert!(matches!(run(&["sonora", "--help"]), Ok(Action::Help)));
        assert!(matches!(run(&["sonora", "-h"]), Ok(Action::Help)));
        assert!(matches!(run(&["sonora", "--version"]), Ok(Action::Version)));
        assert!(matches!(run(&["sonora", "-V"]), Ok(Action::Version)));
    }

    #[test]
    fn render_defaults_to_same_name() {
        match run(&["sonora", "--render", "a.sonora"]) {
            Ok(Action::Render {
                project,
                out,
                sample_rate,
            }) => {
                assert_eq!(project, PathBuf::from("a.sonora"));
                assert_eq!(out, PathBuf::from("a.wav"));
                assert_eq!(sample_rate, 44100);
            }
            _ => panic!("期望 Render"),
        }
    }

    #[test]
    fn explicit_output_and_rate() {
        match run(&[
            "sonora", "--render", "a.sonora", "-o", "b.wav", "-r", "48000",
        ]) {
            Ok(Action::Render {
                out, sample_rate, ..
            }) => {
                assert_eq!(out, PathBuf::from("b.wav"));
                assert_eq!(sample_rate, 48000);
            }
            _ => panic!("期望 Render"),
        }
    }

    #[test]
    fn inspect_mode() {
        assert!(matches!(
            run(&["sonora", "--inspect", "a.sonora"]),
            Ok(Action::Inspect { .. })
        ));
    }

    #[test]
    fn missing_input_opens_empty_gui() {
        assert!(matches!(
            run(&["sonora"]),
            Ok(Action::Open { project: None })
        ));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            run(&["sonora", "--nope"]),
            Err(CliError::UnknownFlag(_))
        ));
        assert!(matches!(
            run(&["sonora", "--render"]),
            Err(CliError::NoInput)
        ));
        assert!(matches!(
            run(&["sonora", "--render", "a", "-r", "abc"]),
            Err(CliError::InvalidNumber { .. })
        ));
        assert!(matches!(
            run(&["sonora", "--render", "a", "-r", "10"]),
            Err(CliError::InvalidNumber { .. })
        ));
        assert!(matches!(
            run(&["sonora", "--render", "a", "-o"]),
            Err(CliError::MissingValue(_))
        ));
    }

    #[test]
    fn bare_path_opens_project() {
        match run(&["sonora", "a.sonora"]) {
            Ok(Action::Open {
                project: Some(project),
            }) => assert_eq!(project, PathBuf::from("a.sonora")),
            _ => panic!("期望 Open"),
        }
    }

    #[test]
    fn help_text_mentions_both_modes() {
        let text = help();
        assert!(text.contains("--render"));
        assert!(text.contains("--inspect"));
        assert!(text.contains("--version"));
    }
}
