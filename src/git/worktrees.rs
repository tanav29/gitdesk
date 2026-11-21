use super::*;

#[derive(Clone, Default)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
    pub bare: bool,
    pub main: bool,
    pub current: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum WorktreeMode {
    #[default]
    NewBranch,
    Existing,
    Detached,
}

pub(super) fn list(repo: &Path) -> Result<Vec<Worktree>> {
    let bytes = git(
        repo,
        &[
            "worktree".as_ref(),
            "list".as_ref(),
            "--porcelain".as_ref(),
            "-z".as_ref(),
        ],
    )?;
    let current = repo.canonicalize().map_err(|e| e.to_string())?;
    let mut trees = Vec::new();
    for field in bytes.split(|&b| b == 0).filter(|f| !f.is_empty()) {
        if let Some(path) = field.strip_prefix(b"worktree ") {
            let path = PathBuf::from(file_name(path)?);
            trees.push(Worktree {
                current: path.canonicalize().as_ref().ok() == Some(&current),
                path,
                main: trees.is_empty(),
                ..Default::default()
            });
            continue;
        }
        let tree = trees.last_mut().ok_or("Incomplete Git worktree record.")?;
        let field = String::from_utf8_lossy(field);
        let (key, value) = field.split_once(' ').unwrap_or((&field, ""));
        match key {
            "HEAD" => tree.head = value.to_owned(),
            "branch" => tree.branch = Some(value.trim_start_matches("refs/heads/").to_owned()),
            "bare" => tree.bare = true,
            "locked" => tree.locked = Some(value.to_owned()),
            "prunable" => tree.prunable = Some(value.to_owned()),
            _ => {}
        }
    }
    Ok(trees)
}

pub(super) fn add(
    repo: &Path,
    path: &Path,
    mode: WorktreeMode,
    branch: &str,
    start: &str,
) -> Result<()> {
    let mut args: Vec<&OsStr> = vec!["worktree".as_ref(), "add".as_ref()];
    let target;
    if mode == WorktreeMode::Existing {
        output(
            repo,
            &["show-ref", "--verify", &format!("refs/heads/{branch}")],
        )?;
        target = branch.to_owned();
    } else {
        let start = if start.is_empty() { "HEAD" } else { start };
        target = output(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{start}^{{commit}}"),
            ],
        )?;
        if mode == WorktreeMode::Detached {
            args.push("--detach".as_ref());
        } else {
            output(repo, &["check-ref-format", "--branch", branch])?;
            args.extend([OsStr::new("-b"), branch.as_ref()]);
        }
    }
    args.extend(["--".as_ref(), path.as_os_str(), target.as_ref()]);
    git(repo, &args).map(drop)
}

pub(super) fn manage(repo: &Path, command: &str, path: &Path) -> Result<()> {
    let tree = list(repo)?
        .into_iter()
        .find(|t| t.path == path)
        .ok_or("This worktree is no longer registered.")?;
    if tree.main || (command == "remove" && tree.current) {
        return Err("Select a different linked worktree.".into());
    }
    git(
        repo,
        &[
            "worktree".as_ref(),
            command.as_ref(),
            "--".as_ref(),
            path.as_os_str(),
        ],
    )
    .map(drop)
}
