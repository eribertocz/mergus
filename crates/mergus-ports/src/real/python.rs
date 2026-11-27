//! A Python interpreter run as a child process, which parses files without importing them
//! (MG-REQ-0611) and runs programs (MG-REQ-0617.1). Python is never embedded.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::port_error;
use crate::{PortError, ProcessOutput, Python, PythonTree};

/// The script that prints the tree of the file read from standard input. It imports only
/// `ast`, `json` and `sys`.
const SCRIPT: &str = include_str!("python_tree.py");

/// The interpreter's options: `-I` ignores the `PYTHON*` variables, the user's site
/// directory and the current directory; `-S` does not import `site`.
const OPTIONS: [&str; 3] = ["-I", "-S", "-c"];

/// How long one call may run before its child is killed, unless
/// [`RealPython::with_timeout`] sets another limit.
pub const PYTHON_TIMEOUT: Duration = Duration::from_secs(60);

/// How often a running child is checked.
const POLL: Duration = Duration::from_millis(5);

/// A Python interpreter found on this machine, run with exactly the environment it was given.
#[derive(Clone, Debug)]
pub struct RealPython {
    interpreter: PathBuf,
    env: Vec<(String, String)>,
    timeout: Duration,
}

impl RealPython {
    /// Finds the interpreter of the project at `root`:
    ///
    /// 1. `configured`, the `[python] interpreter` setting, when there is one; a relative
    ///    path is taken from `root`;
    /// 2. else the project's virtual environment, `<root>/.venv/bin/python`
    ///    (`<root>\.venv\Scripts\python.exe` on Windows);
    /// 3. else `python3` on the search path of `env`, then, on Windows only, `python`.
    ///
    /// `env` is the child's whole environment: it inherits nothing from this process, and
    /// nothing is added to it. The caller builds it with `mergus_security::python_command`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::PythonNotFound`], listing what was tried, when no interpreter
    /// is found. A configured interpreter that does not exist is not replaced by another.
    pub fn locate(
        configured: Option<&Path>,
        root: &Path,
        env: Vec<(String, String)>,
    ) -> Result<Self, PortError> {
        let candidates = match configured {
            Some(path) => vec![root.join(path)],
            None => {
                let mut candidates = vec![root.join(VENV_PYTHON)];
                candidates.extend(on_search_path(&env));
                candidates
            }
        };
        match candidates.iter().find(|path| is_program(path)) {
            Some(interpreter) => Ok(Self {
                interpreter: interpreter.clone(),
                env,
                timeout: PYTHON_TIMEOUT,
            }),
            None => Err(PortError::PythonNotFound {
                searched: candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect(),
            }),
        }
    }

    /// Returns this interpreter with `timeout` as the limit of each call, in place of
    /// [`PYTHON_TIMEOUT`].
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }

    /// Returns the interpreter found.
    #[must_use]
    pub fn interpreter(&self) -> &Path {
        &self.interpreter
    }
}

impl Python for RealPython {
    fn parse(&self, source: &str) -> Result<PythonTree, PortError> {
        let shown = self.interpreter.display().to_string();
        let mut child = Command::new(&self.interpreter)
            .args(OPTIONS)
            .arg(SCRIPT)
            .env_clear()
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| port_error(&shown, &error))?;
        // Standard input is written, and the outputs read, on their own threads, so a child
        // that never reads or fills a pipe cannot block the wait for its deadline.
        let input = source.as_bytes().to_vec();
        let stdin = child.stdin.take();
        let writer = thread::spawn(move || {
            if let Some(mut stdin) = stdin {
                // A child that exits early closes the pipe; its exit code tells why.
                let _ignored = stdin.write_all(&input);
            }
        });
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let status = wait(&mut child, self.timeout, &shown)?;
        let _ignored = writer.join();
        let stdout = joined(stdout, &shown)?;
        let stderr = joined(stderr, &shown)?;
        if !status.success() {
            return Err(PortError::Failed(format!(
                "the Python interpreter `{shown}` failed: {}",
                String::from_utf8_lossy(&stderr).trim()
            )));
        }
        let json = String::from_utf8(stdout).map_err(|error| {
            PortError::Failed(format!("the Python interpreter `{shown}` printed {error}"))
        })?;
        PythonTree::from_json(&json)
    }

    fn run(
        &self,
        script: &str,
        args: &[String],
        current_dir: &Path,
    ) -> Result<ProcessOutput, PortError> {
        let shown = self.interpreter.display().to_string();
        if !current_dir.is_dir() {
            return Err(PortError::NotFound(current_dir.display().to_string()));
        }
        let mut child = Command::new(&self.interpreter)
            .args(OPTIONS)
            .arg(script)
            .args(args)
            .current_dir(current_dir)
            .env_clear()
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| port_error(&shown, &error))?;
        // The outputs are read on their own threads, so a child that fills one pipe while
        // this process reads the other cannot block.
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let status = child.wait().map_err(|error| port_error(&shown, &error))?;
        Ok(ProcessOutput {
            code: status.code(),
            stdout: joined(stdout, &shown)?,
            stderr: joined(stderr, &shown)?,
        })
    }
}

/// Waits for `child` until `timeout` has passed, then kills it.
fn wait(child: &mut Child, timeout: Duration, shown: &str) -> Result<ExitStatus, PortError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| port_error(shown, &error))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            // Killing a child that has just exited fails harmlessly; `wait` reaps it.
            let _ignored = child.kill();
            let _ignored = child.wait();
            return Err(PortError::TimedOut {
                program: shown.to_owned(),
                after: timeout,
            });
        }
        thread::sleep(POLL);
    }
}

/// Reads all of `pipe` on its own thread.
fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            pipe.read_to_end(&mut bytes)?;
        }
        Ok(bytes)
    })
}

/// Returns what a [`drain`] thread read.
fn joined(thread: JoinHandle<std::io::Result<Vec<u8>>>, shown: &str) -> Result<Vec<u8>, PortError> {
    match thread.join() {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(error)) => Err(port_error(shown, &error)),
        Err(_) => Err(PortError::Failed(format!(
            "reading the output of `{shown}` failed"
        ))),
    }
}

/// The interpreter of a project's virtual environment, from the project root.
#[cfg(windows)]
const VENV_PYTHON: &str = ".venv\\Scripts\\python.exe";
#[cfg(not(windows))]
const VENV_PYTHON: &str = ".venv/bin/python";

/// The interpreter names looked for on the search path, in order.
#[cfg(windows)]
const ON_PATH: [&str; 2] = ["python3.exe", "python.exe"];
#[cfg(not(windows))]
const ON_PATH: [&str; 1] = ["python3"];

/// Returns, for each name of [`ON_PATH`] in order, its path in each directory of the search
/// path of `env`.
fn on_search_path(env: &[(String, String)]) -> Vec<PathBuf> {
    let search_path = env
        .iter()
        .rev()
        .find(|(name, _)| is_search_path(name))
        .map(|(_, value)| OsString::from(value));
    let directories: Vec<PathBuf> = search_path
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    ON_PATH
        .iter()
        .flat_map(|name| directories.iter().map(move |dir| dir.join(name)))
        .collect()
}

/// Whether the variable `name` is the search path; Windows ignores the case of names.
fn is_search_path(name: &str) -> bool {
    if cfg!(windows) {
        name.eq_ignore_ascii_case("PATH")
    } else {
        name == "PATH"
    }
}

/// Whether `path` is a file this process may run. An empty file never is: on Windows, the
/// zero-byte App Execution Alias `python3.exe` of the Microsoft Store is skipped, so a real
/// `python.exe` after it is found.
fn is_program(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.len() == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::RealPython;
    use crate::{PortError, Python, PythonTree};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("mergus-ports-tests")
            .join(format!("python-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes the executable file `path` with `contents`.
    fn program(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Parses `source`, retrying while the program is "Text file busy": a test running in
    /// parallel may fork while another one still holds a script it just wrote open, and the
    /// forked child keeps that descriptor until it runs its own program.
    fn parse(python: &RealPython, source: &str) -> Result<PythonTree, PortError> {
        for _ in 0..100 {
            match python.parse(source) {
                Err(PortError::Failed(message)) if message.contains("os error 26") => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                result => return result,
            }
        }
        python.parse(source)
    }

    fn search_path() -> Vec<(String, String)> {
        std::env::var("PATH")
            .map(|path| vec![("PATH".to_owned(), path)])
            .unwrap_or_default()
    }

    #[test]
    fn real_python_prefers_the_configured_one_then_the_venv() {
        let root = scratch("order");
        let venv = root.join(".venv").join("bin").join("python");
        program(&venv, "#!/bin/sh\n");
        program(&root.join("configured"), "#!/bin/sh\n");
        let found = RealPython::locate(None, &root, search_path()).unwrap();
        assert_eq!(found.interpreter(), venv);
        let configured =
            RealPython::locate(Some(Path::new("configured")), &root, Vec::new()).unwrap();
        assert_eq!(configured.interpreter(), root.join("configured"));
        // A file that cannot be run is not an interpreter.
        std::fs::write(root.join("plain"), "").unwrap();
        assert!(matches!(
            RealPython::locate(Some(Path::new("plain")), &root, Vec::new()),
            Err(PortError::PythonNotFound { .. })
        ));
    }

    #[test]
    fn real_python_skips_empty_files() {
        // An empty file, like the Store's `python3.exe` alias on Windows, is not run.
        let root = scratch("empty");
        program(&root.join(".venv").join("bin").join("python"), "");
        program(&root.join("first").join("python3"), "");
        program(&root.join("second").join("python3"), "#!/bin/sh\n");
        let path = std::env::join_paths([root.join("first"), root.join("second")]).unwrap();
        let env = vec![("PATH".to_owned(), path.into_string().unwrap())];
        let found = RealPython::locate(None, &root, env).unwrap();
        assert_eq!(found.interpreter(), root.join("second").join("python3"));
    }

    #[test]
    fn real_python_kills_a_child_that_does_not_answer() {
        let root = scratch("timeout");
        let hung = root.join("hung-python");
        program(&hung, "#!/bin/sh\nexec sleep 30\n");
        let python = RealPython::locate(Some(&hung), &root, Vec::new())
            .unwrap()
            .with_timeout(Duration::from_millis(200));
        let started = Instant::now();
        assert_eq!(
            parse(&python, "x = 1\n"),
            Err(PortError::TimedOut {
                program: hung.display().to_string(),
                after: Duration::from_millis(200),
            })
        );
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn real_python_child_gets_only_the_given_environment() {
        let root = scratch("environment");
        let found = match RealPython::locate(None, &root, search_path()) {
            Ok(found) => found,
            Err(PortError::PythonNotFound { .. }) if std::env::var_os("CI").is_none() => return,
            Err(error) => panic!("no Python interpreter in CI: {error}"),
        };
        // An interpreter that records its arguments and environment, then prints an empty
        // tree. Python itself sets `LC_CTYPE` when the locale is C (PEP 538): it is left out.
        let seen = root.join("seen.json");
        let fake = root.join("fake-python");
        program(
            &fake,
            &format!(
                "#!{}\nimport json, os, sys\n\
                 env = {{k: v for k, v in os.environ.items() if k != 'LC_CTYPE'}}\n\
                 json.dump([sys.argv[1:4], env], open({:?}, 'w'))\n\
                 print('{{\"version\":1}}')\n",
                found.interpreter().display(),
                seen.display().to_string()
            ),
        );
        let env = vec![("MERGUS_GIVEN".to_owned(), "1".to_owned())];
        let python = RealPython::locate(Some(&fake), &root, env).unwrap();
        assert_eq!(parse(&python, "x = 1\n").unwrap(), PythonTree::default());
        #[cfg_attr(
            not(target_os = "macos"),
            expect(unused_mut, reason = "changed on macOS only")
        )]
        let mut seen: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&seen).unwrap()).unwrap();
        // On macOS, the child's own CoreFoundation runtime adds this variable at startup.
        #[cfg(target_os = "macos")]
        seen[1]
            .as_object_mut()
            .unwrap()
            .remove("__CF_USER_TEXT_ENCODING");
        assert_eq!(seen[0], serde_json::json!(["-I", "-S", "-c"]));
        assert_eq!(seen[1], serde_json::json!({"MERGUS_GIVEN": "1"}));
    }
}
