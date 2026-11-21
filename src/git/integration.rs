use super::*;

#[derive(Clone, Copy, PartialEq)]
pub enum IntegrationMode {
    Merge,
    FastForward,
    MergeCommit,
    Rebase,
    RebaseMerges,
}

impl IntegrationMode {
    pub fn is_rebase(self) -> bool {
        matches!(self, Self::Rebase | Self::RebaseMerges)
    }
    pub fn label(self) -> &'static str {
        if self.is_rebase() { "Rebase" } else { "Merge" }
    }
}

#[derive(Clone)]
pub struct Integration {
    pub checkout: Checkout,
    pub source: String,
    pub source_id: String,
    pub ours: usize,
    pub theirs: usize,
    pub mode: IntegrationMode,
}

impl Integration {
    pub fn preview(repo: &Path, source: String, mode: IntegrationMode) -> Result<Self> {
        let checkout = Checkout::read(repo);
        checkout.verify(repo)?;
        let head = checkout.id.as_deref().unwrap();
        let source_id = output(repo, &["show-ref", "--hash", "--verify", &source])?;
        let counts = output(
            repo,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{head}...{source_id}"),
                "--",
            ],
        )?;
        let counts: Vec<usize> = counts
            .split_ascii_whitespace()
            .filter_map(|n| n.parse().ok())
            .collect();
        let [ours, theirs] = counts.as_slice() else {
            return Err("Git returned invalid commit counts.".into());
        };
        Ok(Self {
            checkout,
            source,
            source_id,
            ours: *ours,
            theirs: *theirs,
            mode,
        })
    }

    pub(super) fn apply(&self, repo: &Path) -> Result<()> {
        self.checkout.verify(repo)?;
        if output(repo, &["show-ref", "--hash", "--verify", &self.source])? != self.source_id {
            return Err("The source branch moved. Open a new preview.".into());
        }
        let mut args = match self.mode {
            IntegrationMode::Merge => vec!["merge", "--no-edit"],
            IntegrationMode::FastForward => vec!["merge", "--ff-only"],
            IntegrationMode::MergeCommit => vec!["merge", "--no-ff", "--no-edit"],
            IntegrationMode::Rebase => vec!["rebase", "--no-rebase-merges"],
            IntegrationMode::RebaseMerges => vec!["rebase", "--rebase-merges"],
        };
        args.extend(["--", &self.source]);
        output(repo, &args).map(drop)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Recovery {
    Continue,
    Skip,
    Abort,
}

impl Recovery {
    pub fn label(self) -> &'static str {
        match self {
            Self::Continue => "Continue",
            Self::Skip => "Skip commit",
            Self::Abort => "Abort",
        }
    }
}

impl Operation {
    pub fn command(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::Revert => "revert",
            Self::CherryPick => "cherry-pick",
            Self::Am => "am",
        }
    }

    pub(super) fn recover(self, repo: &Path, recovery: Recovery) -> Result<()> {
        if history_actions::operation(repo)? != Some(self) {
            return Err("The operation changed. Refresh to see the current state.".into());
        }
        let flag = match recovery {
            Recovery::Continue => "--continue",
            Recovery::Abort => "--abort",
            Recovery::Skip => "--skip",
        };
        output(repo, &[self.command(), flag]).map(drop)
    }
}
