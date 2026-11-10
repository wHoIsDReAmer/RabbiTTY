use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::LaunchSpec;

const ZSHENV: &str = include_str!("shell_integration/zshenv.zsh");
const BASH_RC: &str = include_str!("shell_integration/rabbitty.bash");
const LOGIN_FLAGS: [&str; 2] = ["-l", "--login"];
const INTERACTIVE_FLAG: &str = "-i";

static INSTALLED: LazyLock<Option<Scripts>> = LazyLock::new(|| {
    let root = dirs::data_dir()?.join("rabbitty").join("shell-integration");
    match Scripts::install(&root) {
        Ok(scripts) => Some(scripts),
        Err(err) => {
            eprintln!("shell integration unavailable: {err}");
            None
        }
    }
});

pub(super) struct Scripts {
    zdotdir: String,
    bash_rc: String,
}

impl Scripts {
    pub(super) fn install(root: &Path) -> std::io::Result<Self> {
        let zdotdir = root.join("zsh");
        write_if_changed(&zdotdir.join(".zshenv"), ZSHENV)?;
        let bash_rc = root.join("bash").join("rabbitty.bash");
        write_if_changed(&bash_rc, BASH_RC)?;
        Ok(Self {
            zdotdir: utf8(zdotdir)?,
            bash_rc: utf8(bash_rc)?,
        })
    }
}

pub(super) fn apply(spec: LaunchSpec) -> LaunchSpec {
    if !cfg!(unix) || cfg!(test) {
        return spec;
    }
    match INSTALLED.as_ref() {
        Some(scripts) => prepare(scripts, spec, std::env::var("ZDOTDIR").ok()),
        None => spec,
    }
}

pub(super) fn prepare(
    scripts: &Scripts,
    mut spec: LaunchSpec,
    inherited_zdotdir: Option<String>,
) -> LaunchSpec {
    let interactive_only = spec
        .args
        .iter()
        .all(|arg| LOGIN_FLAGS.contains(&arg.as_str()) || arg == INTERACTIVE_FLAG);
    if !interactive_only {
        return spec;
    }
    let shell = Path::new(&spec.program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    match shell {
        "zsh" => {
            let original = spec
                .env
                .iter()
                .find(|(key, _)| key == "ZDOTDIR")
                .map(|(_, value)| value.clone())
                .or(inherited_zdotdir)
                .filter(|value| !value.is_empty());
            spec.env
                .retain(|(key, _)| key != "ZDOTDIR" && key != "RABBITTY_ZDOTDIR");
            if let Some(original) = original {
                spec.env.push(("RABBITTY_ZDOTDIR".to_string(), original));
            }
            spec.env
                .push(("ZDOTDIR".to_string(), scripts.zdotdir.clone()));
        }
        "bash" => {
            let login = spec
                .args
                .iter()
                .any(|arg| LOGIN_FLAGS.contains(&arg.as_str()));
            spec.args.retain(|arg| !LOGIN_FLAGS.contains(&arg.as_str()));
            spec.args
                .splice(0..0, ["--init-file".to_string(), scripts.bash_rc.clone()]);
            if login {
                spec.env
                    .push(("RABBITTY_BASH_LOGIN".to_string(), "1".to_string()));
            }
        }
        _ => {}
    }
    spec
}

fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<()> {
    if std::fs::read_to_string(path).is_ok_and(|current| current == contents) {
        return Ok(());
    }
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("script path has no directory"))?;
    std::fs::create_dir_all(dir)?;
    let staging = dir.join(format!(".staging-{}", std::process::id()));
    std::fs::write(&staging, contents)?;
    std::fs::rename(&staging, path)
}

fn utf8(path: PathBuf) -> std::io::Result<String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| std::io::Error::other("shell integration path is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "rabbitty-shell-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("temp dir");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn spec(program: &str, args: &[&str]) -> LaunchSpec {
        LaunchSpec {
            program: program.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
            cwd: None,
            shell_integration: true,
        }
    }

    fn env_of<'a>(spec: &'a LaunchSpec, key: &str) -> Option<&'a str> {
        spec.env
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    #[test]
    fn zsh_starts_from_rabbittys_directory_and_is_handed_the_users_back() {
        let dir = TempDir::new("zsh-env");
        let scripts = Scripts::install(&dir.0).expect("install");

        let custom = prepare(
            &scripts,
            spec("/bin/zsh", &["-l"]),
            Some("/home/me/.config/zsh".to_string()),
        );
        assert_eq!(env_of(&custom, "ZDOTDIR"), Some(scripts.zdotdir.as_str()));
        assert_eq!(
            env_of(&custom, "RABBITTY_ZDOTDIR"),
            Some("/home/me/.config/zsh"),
            "the user's own ZDOTDIR must be restored, or their config never loads"
        );
        assert_eq!(custom.args, vec!["-l"]);

        let plain = prepare(&scripts, spec("zsh", &[]), None);
        assert_eq!(env_of(&plain, "RABBITTY_ZDOTDIR"), None);
    }

    #[test]
    fn bash_reads_rabbittys_rc_but_still_behaves_as_a_login_shell() {
        let dir = TempDir::new("bash-args");
        let scripts = Scripts::install(&dir.0).expect("install");

        let login = prepare(&scripts, spec("/bin/bash", &["-l"]), None);
        assert_eq!(login.args, vec!["--init-file", scripts.bash_rc.as_str()]);
        assert_eq!(env_of(&login, "RABBITTY_BASH_LOGIN"), Some("1"));

        let plain = prepare(&scripts, spec("bash", &["-i"]), None);
        assert_eq!(
            plain.args,
            vec!["--init-file", scripts.bash_rc.as_str(), "-i"]
        );
        assert_eq!(env_of(&plain, "RABBITTY_BASH_LOGIN"), None);
    }

    #[test]
    fn other_shells_and_scripted_launches_are_left_alone() {
        let dir = TempDir::new("untouched");
        let scripts = Scripts::install(&dir.0).expect("install");

        for (program, args) in [
            ("/usr/bin/fish", vec!["-l"]),
            ("zsh", vec!["-c", "make"]),
            ("bash", vec!["deploy.sh"]),
        ] {
            let prepared = prepare(&scripts, spec(program, &args), None);
            assert_eq!(prepared.args, args, "{program} {args:?}");
            assert!(prepared.env.is_empty(), "{program} {args:?}");
        }
    }

    #[cfg(unix)]
    mod live {
        use super::*;
        use crate::session::{OutputEvent, Session};
        use crate::terminal::{TerminalEngine, TerminalSize, TerminalTheme};
        use iced::futures::channel::mpsc;
        use std::time::{Duration, Instant};

        const PATIENCE: Duration = Duration::from_secs(10);

        fn find(program: &str) -> Option<PathBuf> {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|dir| dir.join(program))
                .find(|path| path.is_file())
        }

        struct Shell {
            session: Session,
            output: mpsc::UnboundedReceiver<OutputEvent>,
            engine: TerminalEngine,
        }

        impl Shell {
            fn start(spec: LaunchSpec) -> Self {
                let (tx, output) = mpsc::unbounded();
                let session = Session::spawn(spec, 1, tx).expect("shell starts");
                let engine = TerminalEngine::new(
                    TerminalSize::new(80, 24),
                    1000,
                    session.writer(),
                    TerminalTheme::default(),
                );
                Self {
                    session,
                    output,
                    engine,
                }
            }

            fn pump(&mut self) {
                while let Ok(event) = self.output.try_recv() {
                    if let OutputEvent::Data { bytes, .. } = event {
                        self.engine.feed_bytes(&bytes);
                    }
                }
            }

            fn wait_for<T>(
                &mut self,
                what: &str,
                mut found: impl FnMut(&mut TerminalEngine) -> Option<T>,
            ) -> T {
                let deadline = Instant::now() + PATIENCE;
                loop {
                    self.pump();
                    if let Some(value) = found(&mut self.engine) {
                        return value;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "timed out waiting for {what}; screen:\n{}",
                        self.engine.recent_lines(0, 24).join("\n")
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }

            fn prompt(&mut self, expected: &str) {
                self.wait_for(&format!("prompt input {expected:?}"), |engine| {
                    (engine.prompt_input().as_deref() == Some(expected)).then_some(())
                });
            }

            fn type_text(&self, text: &str) {
                self.session.send_bytes(text.as_bytes()).expect("typed");
            }

            fn finished(&mut self) -> (Option<u32>, Vec<String>) {
                let command = self.wait_for("a finished command", |engine| {
                    engine.take_finished_commands().pop()
                });
                let output = self
                    .engine
                    .recent_lines(command.output.from, command.output.count);
                (command.exit, output)
            }
        }

        fn launch(program: PathBuf, home: &Path, scripts: &Scripts) -> LaunchSpec {
            let mut spec = spec(&program.to_string_lossy(), &["-l"]);
            spec.env
                .push(("HOME".to_string(), home.to_string_lossy().into_owned()));
            spec.cwd = Some(home.to_path_buf());
            let mut prepared = prepare(scripts, spec, None);
            prepared.shell_integration = false;
            prepared
        }

        #[test]
        fn zsh_marks_its_prompt_and_commands_under_a_theme_without_losing_the_users_config() {
            let Some(zsh) = find("zsh") else {
                return;
            };
            let dir = TempDir::new("live-zsh");
            let scripts = Scripts::install(&dir.0.join("integration")).expect("install");
            let home = dir.0.join("home");
            std::fs::create_dir_all(&home).unwrap();
            std::fs::write(home.join(".zshenv"), "export USER_ENV=env-ok\n").unwrap();
            std::fs::write(
                home.join(".zshrc"),
                "export USER_RC=rc-ok\n\
                 precmd() { true }\n\
                 _theme() { PS1='theme%# ' }\n\
                 precmd_functions+=(_theme)\n\
                 RPROMPT='[right]'\n",
            )
            .unwrap();

            let mut shell = Shell::start(launch(zsh, &home, &scripts));
            shell.prompt("");
            shell.type_text("echo $USER_ENV $USER_RC");
            shell.prompt("echo $USER_ENV $USER_RC");

            shell.type_text("\r");
            let (exit, output) = shell.finished();
            assert_eq!(exit, Some(0));
            assert_eq!(output, vec!["env-ok rc-ok"]);

            shell.prompt("");
            shell.type_text("false\r");
            assert_eq!(
                shell.finished().0,
                Some(1),
                "a user precmd that returns 0 must not hide the real status"
            );
        }

        #[test]
        fn bash_marks_its_prompt_under_a_prompt_command_theme_without_losing_the_users_profile() {
            let Some(bash) = find("bash") else {
                return;
            };
            let version = std::process::Command::new(&bash)
                .args(["-c", "echo $((BASH_VERSINFO[0] * 100 + BASH_VERSINFO[1]))"])
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .and_then(|text| text.trim().parse::<u32>().ok())
                .unwrap_or_default();
            let dir = TempDir::new("live-bash");
            let scripts = Scripts::install(&dir.0.join("integration")).expect("install");
            let home = dir.0.join("home");
            std::fs::create_dir_all(&home).unwrap();
            std::fs::write(
                home.join(".bash_profile"),
                "export USER_RC=rc-ok\nPROMPT_COMMAND='PS1=\"theme\\$ \"; true'\n",
            )
            .unwrap();

            let mut spec = launch(bash, &home, &scripts);
            spec.env.push((
                "BASH_SILENCE_DEPRECATION_WARNING".to_string(),
                "1".to_string(),
            ));
            let mut shell = Shell::start(spec);
            shell.prompt("");
            shell.type_text("echo $USER_RC");
            shell.prompt("echo $USER_RC");
            shell.type_text("\r");
            shell.wait_for("the login profile's variable", |engine| {
                engine
                    .recent_lines(0, 24)
                    .iter()
                    .any(|line| line == "rc-ok")
                    .then_some(())
            });
            if version < 404 {
                return;
            }

            let (exit, output) = shell.finished();
            assert_eq!(exit, Some(0));
            assert_eq!(output, vec!["rc-ok"]);

            shell.prompt("");
            shell.type_text("false\r");
            assert_eq!(shell.finished().0, Some(1));
        }
    }
}
