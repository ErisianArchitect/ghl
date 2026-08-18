
use ::std::{
    path::{Path, PathBuf},
};

use ::clap::{
    Parser,
};

use ::serde::{
    Serialize,
    Deserialize,
};

fn default_format() -> String {
    String::from("{}")
}

fn default_username() -> String {
    String::from("ErisianArchitect")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default = "default_username")]
    username: String,
    #[serde(default = "default_format")]
    format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            username: default_username(),
            format: default_format(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("TOML Serialize Error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("TOML Deserialize Error: {0}")]
    TomlDe(#[from] toml::de::Error),
    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("The format string was invalid.")]
    InvalidFormat,
}

type Result<T = (), E = Error> = std::result::Result<T, E>;

impl Config {
    fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir().expect("Failed to find the config directory.");
        config_dir.join("ghl/config.toml")
    }
    
    fn load_from<P: AsRef<Path>>(path: P, default_if_not_found: bool) -> Result<Self> {
        let path = path.as_ref();
        if default_if_not_found && !path.is_file() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    fn load(default_if_not_found: bool) -> Result<Self> {
        let config_path = Self::config_path();
        Self::load_from(config_path, default_if_not_found)
    }

    fn save_to<P: AsRef<Path>>(&self, path: P) -> Result {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    fn save(&self) -> Result {
        self.save_to(Self::config_path())
    }
}

#[derive(Debug, Parser)]
struct Cmd {
    #[arg(short, long)]
    user: Option<String>,
    #[arg(short, long)]
    format: Option<String>,
    #[arg(short, long="no-format")]
    no_format: bool,
    #[arg(short, long)]
    copy: bool,
    #[arg(short, long)]
    git: bool,
    repo: Vec<String>,
}

fn format_string(format: &str, replacement: &str) -> Result<String> {
    const FORMAT_SPECIFIER: &'static str = "{}";
    let mut buffer = String::with_capacity(format.len() + replacement.len() - FORMAT_SPECIFIER.len());
    let mut i = 0usize;
    let mut raw_start = 0usize;
    let mut push_raw = move |i: usize, next: usize, src: &str, buf: &mut String| {
        if raw_start != i {
            buf.push_str(&src[raw_start..i]);
        }
        raw_start = next;
    };
    while i < format.len() {
        let src_at = &format[i..];
        match src_at.as_bytes()[0] {
            b'{' if src_at.len() >= 2 => match src_at.as_bytes()[1] {
                b'{' => {
                    let next_index = i + 2;
                    push_raw(i, next_index, format, &mut buffer);
                    buffer.push('{');
                    i = next_index;
                },
                b'}' => {
                    if src_at.len() >= 3 && src_at.as_bytes()[2] == b'}' {
                        return Err(Error::InvalidFormat);
                    }
                    let next_index = i + 2;
                    push_raw(i, next_index, format, &mut buffer);
                    buffer.push_str(replacement);
                    i = next_index;
                },
                _ => return Err(Error::InvalidFormat),
            }
            b'}' => {
                if src_at.len() >= 2 && src_at.as_bytes()[1] == b'}' {
                    let next_index = i + 2;
                    push_raw(i, next_index, format, &mut buffer);
                    buffer.push('}');
                    i = next_index;
                    continue;
                }
                return Err(Error::InvalidFormat);
            }
            _ => i += 1,
        }
    }
    push_raw(i, i, format, &mut buffer);
    Ok(buffer)
}

struct GhLink<'a, R = ()> {
    username: &'a str,
    repo: R,
}

impl<'a> GhLink<'a, ()> {
    #[must_use]
    #[inline(always)]
    const fn user(username: &'a str) -> Self {
        Self {
            username,
            repo: (),
        }
    }

    #[must_use]
    #[inline(always)]
    const fn repo(self, repo: &'a str) -> GhLink<'a, &'a str> {
        GhLink {
            username: self.username,
            repo,
        }
    }

    fn link(self) -> String {
        format!("https://github.com/{}", self.username)
    }
}

impl<'a> GhLink<'a, &'a str> {
    fn git(self) -> String {
        format!("https://github.com/{}/{}.git", self.username, self.repo)
    }

    fn link(self) -> String {
        format!("https://github.com/{}/{}", self.username, self.repo)
    }
}

fn map_format(format: String) -> String {
    match format.as_str() {
        "backticks" | "bt" => String::from("`{}`"),
        "quotes" | "q" => String::from("\"{}\""),
        "single" | "s" => String::from("'{}'"),
        "parens" | "p" => String::from("({})"),
        "braces" | "b" => String::from("{{{}}}"),
        "squares" | "sq" => String::from("[{}]"),
        "angles" | "a" => String::from("<{}>"),
        "markdown" | "md" => String::from("[{}]({})"),
        _ => format,
    }
}

fn main() -> Result<()> {
    // let mut clip = arboard::Clipboard::new().unwrap();
    let cmd = Cmd::parse();
    let config = Config::load(true)?;
    let format = cmd.format.unwrap_or(config.format);
    let username = config.username;
    let writer = if cmd.copy {
        |text: &str| {
            // let mut clip = arboard::Clipboard::new().expect("No clipboard, sorry.");
            // clip.set().text(text).expect("Failed to set clipboard text.");
            use std::process::Command;
            let mut command = Command::new("/usr/bin/env");
            command.args(["wl-copy", text]);
            command.spawn().unwrap().wait().unwrap();
        }
    } else {
        |text: &str| {
            println!("{text}");
        }
    };
    if cmd.repo.is_empty() {
        let link = GhLink::user(&username).link();
        if cmd.no_format {
            writer(link.as_str());
        } else {
            let formatted = format_string(&format, &link)?;
            writer(formatted.as_str());
        }
    } else {
        if cmd.copy {
            let mut build = String::with_capacity(1024);
            for repo in cmd.repo {
                let repo = GhLink::user(&username).repo(&repo);
                if cmd.git {
                    let link = repo.git();
                    if cmd.no_format {
                        build.push_str(&link);
                        build.push('\n');
                    } else {
                        let formatted = format_string(&format, &link)?;
                        build.push_str(&formatted);
                        build.push('\n')
                    }
                } else {
                    let link = repo.link();
                    if cmd.no_format {
                        build.push_str(&link);
                        build.push('\n');
                    } else {
                        let formatted = format_string(&format, &link)?;
                        build.push_str(&formatted);
                        build.push('\n');
                    }
                }
            }
            writer(&build);
        } else {
            for repo in cmd.repo {
                let repo = GhLink::user(&username).repo(&repo);
                if cmd.git {
                    let link = repo.git();
                    if cmd.no_format {
                        writer(&link);
                    } else {
                        let formatted = format_string(&format, &link)?;
                        writer(&formatted);
                    }
                } else {
                    let link = repo.link();
                    if cmd.no_format {
                        writer(&link);
                    } else {
                        let formatted = format_string(&format, &link)?;
                        writer(&formatted);
                    }
                }
            }
        }
    }
    Ok(())
}
