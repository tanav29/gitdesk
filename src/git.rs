use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

pub type Result<T> = std::result::Result<T, String>;

pub fn repository_path(path: &Path) -> Result<PathBuf> {
    let path = std::path::absolute(path).map_err(|e| e.to_string())?;
    if !path.is_dir() {
        return Err(format!("Folder does not exist: {}", path.display()));
    }
    for parent in path.ancestors() {
        if parent.join(".git").exists() || is_bare_layout(parent) {
            return Ok(parent.to_owned());
        }
    }
    Err(format!("No Git repository at {}", path.display()))
}

pub const REPO_ENV: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_SHALLOW_FILE",
];

fn is_bare_layout(path: &Path) -> bool {
    path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir()
}

fn git(repo: &Path, args: &[&OsStr]) -> Result<Vec<u8>> {
    let mut git = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        git.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    git.current_dir(repo).env("GIT_TERMINAL_PROMPT", "0");
    for key in REPO_ENV {
        git.env_remove(key);
    }
    if repo.join(".git").exists() {
        git.args(["--git-dir=.git", "--work-tree=."]);
    } else if is_bare_layout(repo) {
        git.arg("--git-dir=.");
    } else {
        return Err(format!("Repository unavailable at {}", repo.display()));
    }
    let output = git
        .arg("--literal-pathspecs")
        .args(args)
        .output()
        .map_err(|e| format!("Could not start Git: {e}"))?;
    if !output.status.success() {
        let message = if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        };
        return Err(String::from_utf8_lossy(message).trim().to_owned());
    }
    Ok(output.stdout)
}

fn output(repo: &Path, args: &[&str]) -> Result<String> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    Ok(String::from_utf8_lossy(&git(repo, &args)?)
        .trim_end()
        .to_owned())
}

#[derive(Debug)]
pub struct Change {
    pub index: u8,
    pub work: u8,
    pub path: OsString,
}

impl Change {
    pub fn staged(&self) -> bool {
        !matches!(self.index, b' ' | b'?' | b'!') && !self.conflict()
    }
    pub fn unstaged(&self) -> bool {
        self.work != b' ' || self.conflict()
    }
    pub fn conflict(&self) -> bool {
        self.index == b'U'
            || self.work == b'U'
            || matches!((self.index, self.work), (b'D', b'D') | (b'A', b'A'))
    }
}

#[cfg(unix)]
fn file_name(raw: &[u8]) -> Result<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(raw.to_vec()))
}
#[cfg(not(unix))]
fn file_name(raw: &[u8]) -> Result<OsString> {
    std::str::from_utf8(raw)
        .map(OsString::from)
        .map_err(|e| e.to_string())
}

#[derive(Default)]
pub struct Snapshot {
    pub branch: String,
    pub bare: bool,
    pub has_head: bool,
    pub changes: Vec<Change>,
    pub history: Vec<(String, String)>,
    pub branches: Vec<String>,
}

pub fn snapshot(repo: &Path) -> Result<Snapshot> {
    let bare = output(repo, &["rev-parse", "--is-bare-repository"])? == "true";
    let head = output(repo, &["rev-parse", "--verify", "--short", "HEAD"]).ok();
    let branch = output(repo, &["symbolic-ref", "--short", "-q", "HEAD"])
        .unwrap_or_else(|_| format!("Detached at {}", head.as_deref().unwrap_or("HEAD")));
    let mut changes = Vec::new();
    if !bare {
        let bytes = git(
            repo,
            &[
                "--no-optional-locks".as_ref(),
                "status".as_ref(),
                "--porcelain=v1".as_ref(),
                "-z".as_ref(),
                "--no-renames".as_ref(),
                "-uall".as_ref(),
            ],
        )?;
        for record in bytes.split(|&c| c == 0).filter(|r| !r.is_empty()) {
            if record.len() < 4 {
                return Err("Git returned an incomplete file status".into());
            }
            changes.push(Change {
                index: record[0],
                work: record[1],
                path: file_name(&record[3..])?,
            });
        }
    }
    let history = if head.is_some() {
        output(repo, &["log", "-40", "--format=%h%x09%s"])?
            .lines()
            .filter_map(|s| s.split_once('\t'))
            .map(|(id, title)| (id.into(), title.into()))
            .collect()
    } else {
        Vec::new()
    };
    let branches = output(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:lstrip=2)",
            "refs/heads/",
        ],
    )?
    .lines()
    .map(str::to_owned)
    .collect();
    Ok(Snapshot {
        branch,
        bare,
        has_head: head.is_some(),
        changes,
        history,
        branches,
    })
}

#[derive(Clone)]
pub enum Action {
    Stage(OsString),
    Unstage(OsString, bool),
    Commit(String),
    Switch(String),
    Fetch,
    Pull,
    Push,
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stage(_) => "Stage",
            Self::Unstage(..) => "Unstage",
            Self::Commit(_) => "Commit",
            Self::Switch(_) => "Switch branch",
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
        }
    }
    pub fn apply(&self, repo: &Path) -> Result<()> {
        let args: Vec<&OsStr> = match self {
            Self::Stage(path) => vec!["add".as_ref(), "--".as_ref(), path],
            Self::Unstage(path, true) => vec![
                "reset".as_ref(),
                "-q".as_ref(),
                "HEAD".as_ref(),
                "--".as_ref(),
                path,
            ],
            Self::Unstage(path, false) => vec![
                "rm".as_ref(),
                "--cached".as_ref(),
                "-f".as_ref(),
                "--".as_ref(),
                path,
            ],
            Self::Commit(message) => vec!["commit".as_ref(), "-m".as_ref(), message.as_ref()],
            Self::Switch(branch) => vec!["switch".as_ref(), "--".as_ref(), branch.as_ref()],
            Self::Fetch => vec!["fetch".as_ref()],
            Self::Pull => vec!["pull".as_ref(), "--ff-only".as_ref()],
            Self::Push => vec!["push".as_ref()],
        };
        git(repo, &args).map(|_| ())
    }
}

pub fn diff(repo: &Path, staged: bool) -> Result<String> {
    diff_output(repo, if staged { &["--cached"] } else { &[] })
}

pub fn comparison_branches(repo: &Path) -> Result<Vec<String>> {
    let refs = output(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname)%09%(symref)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    Ok(refs
        .lines()
        .filter_map(|line| {
            let (name, target) = line.split_once('\t').unwrap_or((line, ""));
            target
                .is_empty()
                .then(|| name.trim_start_matches("refs/").to_owned())
        })
        .collect())
}

pub fn compare_branches(repo: &Path, base: &str, head: &str) -> Result<String> {
    let base = format!("refs/{base}");
    let head = format!("refs/{head}");
    diff_output(repo, &[&base, &head, "--"])
}

fn diff_output(repo: &Path, extra: &[&str]) -> Result<String> {
    let mut args = vec!["diff", "--no-ext-diff", "--no-textconv", "--no-color"];
    args.extend_from_slice(extra);
    output(repo, &args)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Local,
    Global,
    Worktree,
    System,
}
impl Scope {
    pub fn flag(self) -> &'static str {
        match self {
            Self::Local => "--local",
            Self::Global => "--global",
            Self::Worktree => "--worktree",
            Self::System => "--system",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Repository",
            Self::Global => "Global",
            Self::Worktree => "Worktree",
            Self::System => "System",
        }
    }
}
pub type Entries = Vec<(String, String)>;
pub fn settings(repo: &Path, scope: Scope) -> Result<Entries> {
    let data = git(
        repo,
        &[
            "config".as_ref(),
            scope.flag().as_ref(),
            "--null".as_ref(),
            "--no-includes".as_ref(),
            "--list".as_ref(),
        ],
    )?;
    let mut entries = Vec::new();
    for record in data.split(|&b| b == 0).filter(|r| !r.is_empty()) {
        let line = std::str::from_utf8(record).map_err(|e| e.to_string())?;
        let (key, value) = line.split_once('\n').unwrap_or((line, ""));
        entries.push((key.to_owned(), value.to_owned()));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

#[derive(Clone, Copy)]
pub enum ConfigEdit {
    Set,
    Add,
    Remove,
}
pub fn edit_setting(
    repo: &Path,
    scope: Scope,
    edit: ConfigEdit,
    key: &str,
    value: &str,
) -> Result<()> {
    let mut args = vec!["config", scope.flag()];
    match edit {
        ConfigEdit::Set => {}
        ConfigEdit::Add => args.push("--add"),
        ConfigEdit::Remove => args.extend(["--fixed-value", "--unset"]),
    }
    args.extend(["--", key, value]);
    output(repo, &args).map(|_| ())
}
