use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod tags;
pub use tags::Tag;

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
    run_git(repo, args, None)
}

fn run_git(repo: &Path, args: &[&OsStr], input: Option<&[u8]>) -> Result<Vec<u8>> {
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
    git.arg("--literal-pathspecs").args(args);
    let (output, written) = if let Some(input) = input {
        let mut child = git
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start Git: {e}"))?;
        let mut stdin = child.stdin.take().unwrap();
        let (output, written) = std::thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(input));
            let output = child.wait_with_output();
            (output, writer.join().unwrap())
        });
        let output = output.map_err(|e| format!("Could not read Git result: {e}"))?;
        (output, written)
    } else {
        (
            git.output()
                .map_err(|e| format!("Could not start Git: {e}"))?,
            Ok(()),
        )
    };
    if !output.status.success() {
        let message = if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        };
        return Err(String::from_utf8_lossy(message).trim().to_owned());
    }
    written.map_err(|e| format!("Could not send file paths to Git: {e}"))?;
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
    pub history: Vec<Commit>,
    pub branches: Vec<String>,
    pub stashes: Vec<Stash>,
    pub tags: Vec<Tag>,
}

#[derive(Clone)]
pub struct Stash {
    pub reference: String,
    pub id: String,
    pub description: String,
}

fn stashes(repo: &Path) -> Result<Vec<Stash>> {
    output(
        repo,
        &[
            "stash",
            "list",
            "--no-color",
            "--no-show-signature",
            "--no-notes",
            "--format=%gd%x00%H%x00%gs",
        ],
    )?
    .lines()
    .map(|line| {
        let mut fields = line.splitn(3, '\0');
        match (fields.next(), fields.next(), fields.next()) {
            (Some(reference), Some(id), Some(description)) => Ok(Stash {
                reference: reference.into(),
                id: id.into(),
                description: description.into(),
            }),
            _ => Err("Git returned an incomplete stash record.".into()),
        }
    })
    .collect()
}

#[derive(Clone)]
pub struct Commit {
    pub id: String,
    pub short_id: String,
    pub parents: Vec<String>,
    pub refs: String,
    pub title: String,
}

fn history(repo: &Path, has_head: bool) -> Result<Vec<Commit>> {
    let mut args = vec![
        "log",
        "--topo-order",
        "--no-color",
        "--no-show-signature",
        "--no-notes",
        "--decorate=short",
        "--branches",
        "--remotes",
        "-40",
        "--format=%H%x00%h%x00%P%x00%D%x00%s",
    ];
    if has_head {
        args.push("HEAD");
    }
    args.push("--");
    output(repo, &args)?
        .lines()
        .map(|line| {
            let mut fields = line.splitn(5, '\0');
            let mut field = || {
                fields
                    .next()
                    .ok_or_else(|| "Git returned an incomplete commit record.".to_owned())
            };
            Ok(Commit {
                id: field()?.to_owned(),
                short_id: field()?.to_owned(),
                parents: field()?
                    .split_ascii_whitespace()
                    .map(str::to_owned)
                    .collect(),
                refs: field()?.to_owned(),
                title: field()?.to_owned(),
            })
        })
        .collect()
}

fn changes(repo: &Path) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
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
    Ok(changes)
}

fn local_branches(repo: &Path) -> Result<Vec<String>> {
    Ok(output(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:lstrip=2)",
            "refs/heads/",
        ],
    )?
    .lines()
    .map(str::to_owned)
    .collect())
}

pub fn snapshot(repo: &Path) -> Result<Snapshot> {
    let bare = output(repo, &["rev-parse", "--is-bare-repository"])? == "true";
    std::thread::scope(|scope| {
        let tags = scope.spawn(|| tags::list(repo));
        let branches = scope.spawn(|| local_branches(repo));
        let changes = (!bare).then(|| scope.spawn(|| changes(repo)));
        let stashes = (!bare).then(|| scope.spawn(|| stashes(repo)));
        let branch = scope.spawn(|| output(repo, &["symbolic-ref", "--short", "-q", "HEAD"]));
        let head = output(repo, &["rev-parse", "--verify", "--short", "HEAD"]).ok();
        let branch = branch
            .join()
            .unwrap()
            .unwrap_or_else(|_| format!("Detached at {}", head.as_deref().unwrap_or("HEAD")));
        let history = history(repo, head.is_some());
        Ok(Snapshot {
            branch,
            bare,
            has_head: head.is_some(),
            changes: changes.map_or_else(|| Ok(Vec::new()), |job| job.join().unwrap())?,
            history: history?,
            branches: branches.join().unwrap()?,
            tags: tags.join().unwrap()?,
            stashes: stashes.map_or_else(|| Ok(Vec::new()), |job| job.join().unwrap())?,
        })
    })
}

#[derive(Clone, Copy, Default)]
pub struct CommitOptions {
    pub sign_off: bool,
    pub sign: bool,
    pub amend: bool,
    pub allow_empty: bool,
}

#[derive(Clone)]
pub enum Action {
    Stage(Vec<OsString>),
    Unstage(Vec<OsString>, bool),
    Commit(String, CommitOptions),
    Stash(String, bool),
    RestoreStash {
        stash: Stash,
        pop: bool,
        restore_index: bool,
    },
    Switch(String),
    CreateBranch(String),
    CreateBranchFrom(String, String),
    RenameBranch(String, String),
    DeleteBranch(String),
    Merge(String),
    Rebase(String),
    CreateTag {
        name: String,
        target: String,
        message: String,
    },
    DeleteTag(Tag),
    Fetch,
    Pull,
    Push,
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stage(_) => "Stage",
            Self::Unstage(..) => "Unstage",
            Self::Commit(_, options) if options.amend => "Amend commit",
            Self::Commit(..) => "Commit",
            Self::Stash(..) => "Stash changes",
            Self::RestoreStash { pop: true, .. } => "Pop stash",
            Self::RestoreStash { pop: false, .. } => "Apply stash",
            Self::Switch(_) => "Switch branch",
            Self::CreateBranch(_) => "Create branch",
            Self::CreateBranchFrom(..) => "Create branch",
            Self::RenameBranch(..) => "Rename branch",
            Self::DeleteBranch(_) => "Delete branch",
            Self::Merge(_) => "Merge branch",
            Self::Rebase(_) => "Rebase branch",
            Self::CreateTag { .. } => "Create tag",
            Self::DeleteTag(_) => "Delete tag",
            Self::Fetch => "Fetch",
            Self::Pull => "Pull",
            Self::Push => "Push",
        }
    }
    pub fn apply(&self, repo: &Path) -> Result<()> {
        let (command, paths): (&[&str], &[OsString]) = match self {
            Self::Stage(paths) => (&["add"], paths),
            Self::Unstage(paths, true) => (&["reset", "-q", "HEAD"], paths),
            Self::Unstage(paths, false) => (&["rm", "--cached", "-f"], paths),
            Self::Commit(message, options) => {
                let mut args = vec!["commit"];
                args.extend(
                    [
                        (options.sign_off, "--signoff"),
                        (options.sign, "--gpg-sign"),
                        (options.amend, "--amend"),
                        (options.allow_empty, "--allow-empty"),
                    ]
                    .into_iter()
                    .filter_map(|(enabled, flag)| enabled.then_some(flag)),
                );
                if options.amend && message.trim().is_empty() {
                    args.push("--no-edit");
                } else {
                    args.extend(["-m", message]);
                }
                return output(repo, &args).map(|_| ());
            }
            Self::Stash(message, include_untracked) => {
                // Stash builds its own pathspec for cleaning up the saved untracked files.
                let mut args = vec!["--no-literal-pathspecs", "stash", "push"];
                if *include_untracked {
                    args.push("--include-untracked");
                }
                if !message.trim().is_empty() {
                    args.extend(["--message", message]);
                }
                return output(repo, &args).map(|_| ());
            }
            Self::RestoreStash {
                stash,
                pop,
                restore_index,
            } => {
                if output(
                    repo,
                    &[
                        "rev-parse",
                        "--verify",
                        "--end-of-options",
                        &stash.reference,
                    ],
                )
                .as_deref()
                    != Ok(stash.id.as_str())
                {
                    return Err(
                        "The stash list changed. Select the stash again from the updated list."
                            .into(),
                    );
                }
                let commit = format!("{}^{{commit}}", stash.id);
                return output(
                    repo,
                    &[
                        "--no-literal-pathspecs",
                        "stash",
                        if *pop { "pop" } else { "apply" },
                        if *restore_index {
                            "--index"
                        } else {
                            "--no-index"
                        },
                        "--",
                        if *pop { &stash.reference } else { &commit },
                    ],
                )
                .map(|_| ());
            }
            Self::Switch(branch) => return output(repo, &["switch", "--", branch]).map(|_| ()),
            Self::CreateBranch(branch) => {
                output(repo, &["check-ref-format", "--branch", branch])?;
                return output(repo, &["branch", "--", branch]).map(|_| ());
            }
            Self::CreateBranchFrom(branch, source) => {
                output(repo, &["check-ref-format", "--branch", branch])?;
                return output(
                    repo,
                    &["branch", "--", branch, &format!("refs/heads/{source}")],
                )
                .map(|_| ());
            }
            Self::RenameBranch(old, new) => {
                output(repo, &["check-ref-format", "--branch", new])?;
                return output(repo, &["branch", "-m", "--", old, new]).map(|_| ());
            }
            Self::DeleteBranch(branch) => {
                return output(repo, &["branch", "-d", "--", branch]).map(|_| ());
            }
            Self::Merge(branch) => {
                return output(
                    repo,
                    &["merge", "--no-edit", "--", &format!("refs/heads/{branch}")],
                )
                .map(|_| ());
            }
            Self::Rebase(branch) => {
                return output(repo, &["rebase", "--", &format!("refs/heads/{branch}")])
                    .map(|_| ());
            }
            Self::CreateTag {
                name,
                target,
                message,
            } => return tags::create(repo, name, target, message),
            Self::DeleteTag(tag) => return tags::delete(repo, tag),
            Self::Fetch => return output(repo, &["fetch"]).map(|_| ()),
            Self::Pull => return output(repo, &["pull", "--ff-only"]).map(|_| ()),
            Self::Push => return output(repo, &["push"]).map(|_| ()),
        };
        if paths.is_empty() {
            return Ok(());
        }
        let args = command
            .iter()
            .copied()
            .chain(["--pathspec-from-file=-", "--pathspec-file-nul"])
            .map(OsStr::new)
            .collect::<Vec<_>>();
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(path.as_encoded_bytes());
            input.push(0);
        }
        run_git(repo, &args, Some(&input)).map(|_| ())
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
    patch_output(repo, "diff", extra)
}

pub fn commit_diff(repo: &Path, id: &str) -> Result<String> {
    patch_output(
        repo,
        "show",
        &[
            "--format=",
            "--patch",
            "--root",
            "--diff-merges=first-parent",
            id,
            "--",
        ],
    )
}

fn patch_output(repo: &Path, command: &str, extra: &[&str]) -> Result<String> {
    let mut args = vec![
        command,
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "--output-indicator-new=+",
        "--output-indicator-old=-",
        "--output-indicator-context= ",
    ];
    args.extend_from_slice(extra);
    let args: Vec<_> = args.into_iter().map(OsStr::new).collect();
    Ok(String::from_utf8_lossy(&git(repo, &args)?).into_owned())
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
