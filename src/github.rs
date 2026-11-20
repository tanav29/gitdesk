use crate::git::{REPO_ENV, Result};
use serde::Deserialize;
use std::{io::ErrorKind, path::Path, process::Command};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    PullRequests,
    Issues,
    Runs,
}

impl Section {
    pub const ALL: [Self; 3] = [Self::PullRequests, Self::Issues, Self::Runs];

    pub fn label(self) -> &'static str {
        match self {
            Self::PullRequests => "Pull requests",
            Self::Issues => "Issues",
            Self::Runs => "Actions",
        }
    }

    pub fn empty(self) -> &'static str {
        match self {
            Self::PullRequests => "No open pull requests.",
            Self::Issues => "No open issues.",
            Self::Runs => "No recent workflow runs.",
        }
    }

    fn query(self) -> (&'static str, &'static str) {
        match self {
            Self::PullRequests => ("pr", "number,title,url,isDraft"),
            Self::Issues => ("issue", "number,title,url"),
            Self::Runs => (
                "run",
                "displayTitle,url,workflowName,headBranch,status,conclusion",
            ),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(alias = "displayTitle")]
    pub title: String,
    pub url: String,
    pub number: Option<u64>,
    #[serde(default)]
    pub is_draft: bool,
    pub workflow_name: Option<String>,
    pub head_branch: Option<String>,
    pub status: Option<String>,
    pub conclusion: Option<String>,
}

impl Entry {
    pub fn status(&self) -> Option<String> {
        if self.is_draft {
            return Some("Draft".into());
        }
        self.conclusion
            .as_deref()
            .filter(|value| !value.is_empty())
            .or(self.status.as_deref())
            .map(|value| value.replace('_', " "))
    }
}

pub fn list(repo: &Path, section: Section) -> Result<Vec<Entry>> {
    let (group, fields) = section.query();
    let mut gh = Command::new("gh");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        gh.creation_flags(0x08000000);
    }
    gh.current_dir(repo)
        .env("GH_PROMPT_DISABLED", "1")
        .env_remove("GH_REPO");
    for key in REPO_ENV {
        gh.env_remove(key);
    }
    let output = gh
        .args([group, "list", "--limit", "25", "--json", fields])
        .output()
        .map_err(|error| {
            if error.kind() == ErrorKind::NotFound {
                "Install GitHub CLI (gh) and add it to PATH to use this view.".into()
            } else {
                format!("Could not start GitHub CLI. {error}")
            }
        })?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message = message.trim();
        return Err(if message.is_empty() {
            format!("GitHub CLI exited with {}.", output.status)
        } else {
            message.to_owned()
        });
    }
    parse(&output.stdout)
}

pub fn parse(output: &[u8]) -> Result<Vec<Entry>> {
    serde_json::from_slice(output)
        .map_err(|error| format!("Could not read GitHub results. {error}"))
}
