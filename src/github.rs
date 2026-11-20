use crate::git::{REPO_ENV, Result};
use serde::Deserialize;
use std::{
    io::{self, ErrorKind, Write},
    path::Path,
    process::{Command, Output, Stdio},
};

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

#[derive(Deserialize)]
pub struct Conversation {
    pub author: Option<Author>,
    pub body: String,
    pub comments: Vec<Comment>,
}

#[derive(Deserialize)]
pub struct Author {
    pub login: String,
}

#[derive(Deserialize)]
pub struct Comment {
    pub author: Option<Author>,
    pub body: String,
}

fn command(repo: &Path) -> Command {
    let mut gh = Command::new("gh");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        gh.creation_flags(0x08000000);
    }
    gh.current_dir(repo)
        .env("GH_PROMPT_DISABLED", "1")
        .env_remove("GH_FORCE_TTY")
        .env_remove("GH_REPO");
    for key in REPO_ENV {
        gh.env_remove(key);
    }
    gh
}

fn start_error(error: io::Error) -> String {
    if error.kind() == ErrorKind::NotFound {
        "Install GitHub CLI (gh) and add it to PATH to use this view.".into()
    } else {
        format!("Could not start GitHub CLI. {error}")
    }
}

fn stdout(output: Output) -> Result<Vec<u8>> {
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let message = message.trim();
        return Err(if message.is_empty() {
            format!("GitHub CLI exited with {}.", output.status)
        } else {
            message.to_owned()
        });
    }
    Ok(output.stdout)
}

fn read(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    stdout(command(repo).args(args).output().map_err(start_error)?)
}

pub fn list(repo: &Path, section: Section) -> Result<Vec<Entry>> {
    let (group, fields) = section.query();
    parse(&read(
        repo,
        &[group, "list", "--limit", "25", "--json", fields],
    )?)
}

pub fn conversation(repo: &Path, section: Section, url: &str) -> Result<Conversation> {
    let (group, _) = section.query();
    let output = read(
        repo,
        &[group, "view", "--json", "author,body,comments", "--", url],
    )?;
    serde_json::from_slice(&output)
        .map_err(|error| format!("Could not read the GitHub conversation. {error}"))
}

pub fn diff(repo: &Path, url: &str) -> Result<String> {
    read(repo, &["pr", "diff", "--color=never", "--", url])
        .map(|output| String::from_utf8_lossy(&output).into_owned())
}

pub fn comment(repo: &Path, section: Section, url: &str, body: &str) -> Result<()> {
    let (group, _) = section.query();
    let mut child = command(repo)
        .args([group, "comment", "--body-file", "-", "--", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(start_error)?;
    let written = child.stdin.take().unwrap().write_all(body.as_bytes());
    let output = child
        .wait_with_output()
        .map_err(|error| format!("Could not read the comment submission result. {error}"))?;
    stdout(output)?;
    written.map_err(|error| format!("Could not send the comment text. {error}"))
}

pub fn parse(output: &[u8]) -> Result<Vec<Entry>> {
    serde_json::from_slice(output)
        .map_err(|error| format!("Could not read GitHub results. {error}"))
}
