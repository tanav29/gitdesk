use super::*;

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Checkout {
    pub id: Option<String>,
    pub reference: Option<String>,
}

impl Checkout {
    pub(super) fn read(repo: &Path) -> Self {
        Self {
            id: output(repo, &["rev-parse", "--verify", "HEAD"]).ok(),
            reference: output(repo, &["symbolic-ref", "-q", "HEAD"]).ok(),
        }
    }

    pub(super) fn verify(&self, repo: &Path) -> Result<()> {
        if self.id.is_none() || *self != Self::read(repo) {
            return Err("The checkout changed. Refresh and review this action again.".into());
        }
        if operation(repo)?.is_some() {
            return Err("Finish the current Git operation first.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operation {
    Merge,
    Rebase,
    Revert,
    CherryPick,
    Am,
}

pub(super) fn operation(repo: &Path) -> Result<Option<Operation>> {
    let bytes = git(repo, &["rev-parse".as_ref(), "--absolute-git-dir".as_ref()])?;
    let dir = PathBuf::from(file_name(bytes.strip_suffix(b"\n").unwrap_or(&bytes))?);
    for (marker, op) in [
        ("rebase-apply/applying", Operation::Am),
        ("rebase-apply", Operation::Rebase),
        ("rebase-merge", Operation::Rebase),
        ("MERGE_HEAD", Operation::Merge),
        ("REVERT_HEAD", Operation::Revert),
        ("CHERRY_PICK_HEAD", Operation::CherryPick),
    ] {
        if dir.join(marker).exists() {
            return Ok(Some(op));
        }
    }
    Ok(None)
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum ResetMode {
    #[default]
    Soft,
    Mixed,
    Hard,
}

#[derive(Clone)]
pub enum HistoryAction {
    Revert { mainline: Option<usize> },
    Reset(ResetMode),
    Squash(String),
}

impl HistoryAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Revert { .. } => "Revert commit",
            Self::Reset(_) => "Reset to commit",
            Self::Squash(_) => "Squash commits",
        }
    }

    pub(super) fn apply(&self, repo: &Path, checkout: &Checkout, target: &str) -> Result<()> {
        checkout.verify(repo)?;
        let id = output(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--end-of-options",
                &format!("{target}^{{commit}}"),
            ],
        )?;
        match self {
            Self::Reset(mode) => {
                let flag = match mode {
                    ResetMode::Soft => "--soft",
                    ResetMode::Mixed => "--mixed",
                    ResetMode::Hard => "--hard",
                };
                output(repo, &["reset", flag, &id, "--"]).map(drop)
            }
            Self::Revert { mainline } => {
                let parent;
                let mut args = vec!["revert", "--no-edit"];
                if let Some(n) = mainline {
                    parent = n.to_string();
                    args.extend(["-m", &parent]);
                }
                args.extend(["--", &id]);
                output(repo, &args).map(drop)
            }
            Self::Squash(message) => {
                let head = checkout.id.as_deref().unwrap();
                if message.trim().is_empty() || head == id {
                    return Err("Choose at least two commits and enter a squash message.".into());
                }
                output(repo, &["merge-base", "--is-ancestor", &id, head])
                    .map_err(|_| "Choose a commit on the current branch's history.")?;
                let range = format!("{id}..{head}");
                let parents = output(
                    repo,
                    &[
                        "show",
                        "-s",
                        "--format=%P",
                        "--no-notes",
                        "--no-show-signature",
                        &id,
                    ],
                )?;
                if parents.contains(' ')
                    || !output(repo, &["rev-list", "--max-count=1", "--merges", &range])?.is_empty()
                {
                    return Err("Choose a linear range without merge commits.".into());
                }
                // Reuse HEAD's tree. The user's working tree and index never enter the new commit.
                let tree = format!("{head}^{{tree}}");
                let mut args = vec!["commit-tree", &tree];
                if !parents.is_empty() {
                    args.extend(["-p", &parents]);
                }
                args.extend(["-m", message]);
                let replacement = output(repo, &args)?;
                checkout.verify(repo)?;
                output(
                    repo,
                    &[
                        "update-ref",
                        "-m",
                        "squash: combine commits",
                        "HEAD",
                        &replacement,
                        head,
                    ],
                )
                .map(drop)
            }
        }
    }
}
