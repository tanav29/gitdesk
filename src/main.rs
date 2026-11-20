mod git;
mod github;
mod terminal;
mod terminal_view;
#[cfg(test)]
mod tests;
mod view;

use git::{Action, ConfigEdit, Entries, Scope, Snapshot};
use rune::{AppSpec, Latest, TextEdit};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use terminal::Terminal;

const MIN_WINDOW_SIZE: (f32, f32) = (740.0, 460.0);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum DiffMode {
    WorkingTree,
    Staged,
    Compare,
    Commit,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Tab {
    Changes,
    Diff(DiffMode),
    History,
    Branches,
    Settings,
    GitHub,
    Terminal,
}

#[derive(Default)]
struct Workspace {
    selected: Option<PathBuf>,
    snapshot: Option<Snapshot>,
    notice: String,
}

#[derive(Default)]
struct ConfigForm {
    scope: Scope,
    entries: Entries,
    key: TextEdit,
    value: TextEdit,
}

#[derive(Default)]
struct BranchComparison {
    branches: Vec<String>,
    selected: [Option<String>; 2],
    picker: Option<usize>,
    compared: bool,
}

struct GitHubThread {
    number: u64,
    title: String,
    url: String,
    conversation: Option<github::Conversation>,
    draft: TextEdit,
    show_diff: bool,
    patch: Option<String>,
    diff_pages: usize,
}

enum Update {
    Opened(git::Result<(PathBuf, Snapshot)>),
    Settings(git::Result<Entries>, git::Result<()>),
    GitHub(git::Result<Vec<github::Entry>>),
    GitHubConversation(git::Result<github::Conversation>),
    GitHubComment(git::Result<git::Result<github::Conversation>>),
    GitHubDiff(git::Result<String>),
    Terminal(git::Result<Terminal>),
    Repository {
        path: PathBuf,
        snapshot: git::Result<Snapshot>,
        outcome: git::Result<String>,
        committed: bool,
    },
    Diff(git::Result<String>),
    ComparisonBranches(git::Result<Vec<String>>),
}

struct Desk {
    workspace: Workspace,
    tab: Tab,
    draft: TextEdit,
    drafts: HashMap<PathBuf, String>,
    selected_changes: [HashSet<OsString>; 2],
    patch: String,
    diff_pages: usize,
    selected_commit: Option<git::Commit>,
    comparison: BranchComparison,
    opening: bool,
    path: TextEdit,
    config: ConfigForm,
    github_section: github::Section,
    github_entries: Vec<github::Entry>,
    github_thread: Option<GitHubThread>,
    shells: HashMap<PathBuf, Terminal>,
    waker: Option<rune::Waker>,
    task: Latest<Update>,
    busy: bool,
}

impl Desk {
    fn new(initial: Option<PathBuf>) -> Self {
        let mut desk = Self {
            workspace: Workspace::default(),
            opening: true,
            path: TextEdit::new(""),
            config: ConfigForm::default(),
            github_section: github::Section::default(),
            github_entries: Vec::new(),
            github_thread: None,
            shells: HashMap::new(),
            waker: None,
            tab: Tab::Changes,
            draft: TextEdit::new(""),
            drafts: HashMap::new(),
            selected_changes: Default::default(),
            patch: String::new(),
            diff_pages: 1,
            selected_commit: None,
            comparison: BranchComparison::default(),
            task: Latest::default(),
            busy: false,
        };
        if let Some(root) = initial {
            desk.open(root);
        }
        desk
    }

    fn start(&mut self, message: &str, job: impl FnOnce() -> Update + Send + 'static) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.workspace.notice = message.to_owned();
        self.task.request((), move |()| job());
    }

    fn open(&mut self, path: PathBuf) {
        self.path = TextEdit::new(path.to_string_lossy());
        self.start("Opening repository…", move || {
            Update::Opened(
                git::repository_path(&path)
                    .and_then(|root| git::snapshot(&root).map(|snapshot| (root, snapshot))),
            )
        });
    }

    fn open_input(&mut self) {
        let path = self.path.text.trim().trim_matches('"');
        if !path.is_empty() {
            self.open(PathBuf::from(path));
        }
    }

    fn shell(&self) -> Option<&Terminal> {
        self.shells.get(self.workspace.selected.as_ref()?)
    }

    fn load_github(&mut self, section: github::Section) {
        if self.busy {
            return;
        }
        let Some(path) = self.workspace.selected.clone() else {
            return;
        };
        self.github_section = section;
        self.github_thread = None;
        self.github_entries.clear();
        self.start("Loading GitHub…", move || {
            Update::GitHub(github::list(&path, section))
        });
    }

    fn open_github_thread(&mut self, number: u64) {
        if self.busy || self.github_section == github::Section::Runs {
            return;
        }
        let Some(entry) = self
            .github_entries
            .iter()
            .find(|entry| entry.number == Some(number))
        else {
            return;
        };
        self.github_thread = Some(GitHubThread {
            number,
            title: entry.title.clone(),
            url: entry.url.clone(),
            conversation: None,
            draft: TextEdit::new(""),
            show_diff: false,
            patch: None,
            diff_pages: 1,
        });
        self.refresh_github_thread();
    }

    fn refresh_github_thread(&mut self) {
        let (Some(repo), Some(thread)) = (&self.workspace.selected, &self.github_thread) else {
            return;
        };
        let (repo, url) = (repo.clone(), thread.url.clone());
        if thread.show_diff {
            self.start("Loading changed files…", move || {
                Update::GitHubDiff(github::diff(&repo, &url))
            });
        } else {
            let section = self.github_section;
            self.start("Loading conversation…", move || {
                Update::GitHubConversation(github::conversation(&repo, section, &url))
            });
        }
    }

    fn show_github_diff(&mut self, show_diff: bool) {
        if self.busy {
            return;
        }
        let Some(thread) = &mut self.github_thread else {
            return;
        };
        if thread.show_diff == show_diff {
            return;
        }
        thread.show_diff = show_diff;
        self.workspace.notice.clear();
        if (show_diff && thread.patch.is_none()) || (!show_diff && thread.conversation.is_none()) {
            self.refresh_github_thread();
        }
    }

    fn post_github_comment(&mut self) {
        let (Some(repo), Some(thread)) = (&self.workspace.selected, &self.github_thread) else {
            return;
        };
        if thread.draft.text.trim().is_empty() {
            return;
        }
        let (repo, url, body) = (repo.clone(), thread.url.clone(), thread.draft.text.clone());
        let section = self.github_section;
        self.start("Posting comment…", move || {
            Update::GitHubComment(
                github::comment(&repo, section, &url, &body)
                    .map(|()| github::conversation(&repo, section, &url)),
            )
        });
    }

    fn start_shell(&mut self) {
        let Some(path) = self.workspace.selected.clone() else {
            return;
        };
        if self.shell().is_some_and(|t| t.ended.is_none()) {
            return;
        }
        let wake = self
            .waker
            .clone()
            .unwrap_or_else(|| std::sync::Arc::new(|| {}));
        self.start("Opening terminal…", move || {
            Update::Terminal(Terminal::open(&path, wake))
        });
    }

    fn settings(&mut self, scope: Scope, edit: Option<ConfigEdit>) {
        let Some(repo) = self.workspace.selected.clone() else {
            return;
        };
        if self.config.scope != scope {
            self.config = ConfigForm {
                scope,
                ..Default::default()
            };
        }
        let key = self.config.key.text.trim().to_owned();
        let value = self.config.value.text.clone();
        self.start("Reading Git configuration…", move || {
            let result = edit.map_or(Ok(()), |mode| {
                git::edit_setting(&repo, scope, mode, &key, &value)
            });
            Update::Settings(git::settings(&repo, scope), result)
        });
    }

    fn load_repo(&mut self, path: PathBuf, action: Option<Action>) {
        let progress = action.as_ref().map_or("Reading repository…".into(), |a| {
            format!("Running {}…", a.label().to_lowercase())
        });
        self.start(&progress, move || {
            let outcome = action.as_ref().map_or_else(
                || Ok(String::new()),
                |a| a.apply(&path).map(|()| format!("{} complete.", a.label())),
            );
            let committed = matches!(action, Some(Action::Commit(_))) && outcome.is_ok();
            let snapshot = git::snapshot(&path);
            Update::Repository {
                path,
                snapshot,
                outcome,
                committed,
            }
        });
    }

    fn act(&mut self, action: Action) {
        if let Some(repo) = self.workspace.selected.clone() {
            self.load_repo(repo, Some(action));
        }
    }

    fn select_tab(&mut self, tab: Tab) {
        if self.busy {
            return;
        }
        self.tab = tab;
        self.comparison.picker = None;
        if tab == Tab::Terminal {
            self.start_shell();
        } else if tab == Tab::GitHub {
            if self.github_thread.is_some() {
                self.refresh_github_thread();
            } else {
                self.load_github(self.github_section);
            }
        } else if tab == Tab::Settings {
            self.settings(self.config.scope, None);
        } else if !matches!(tab, Tab::Diff(_))
            && let Some(repo) = self.workspace.selected.clone()
        {
            self.load_repo(repo, None);
        }
        if let Tab::Diff(mode) = tab
            && let Some(repo) = self.workspace.selected.clone()
        {
            self.patch.clear();
            match mode {
                DiffMode::Compare => {
                    self.comparison.compared = false;
                    self.start("Reading branches…", move || {
                        Update::ComparisonBranches(git::comparison_branches(&repo))
                    });
                }
                DiffMode::Commit => {
                    if let Some(commit) = &self.selected_commit {
                        let id = commit.id.clone();
                        self.start("Reading commit diff…", move || {
                            Update::Diff(git::commit_diff(&repo, &id))
                        });
                    }
                }
                DiffMode::WorkingTree | DiffMode::Staged => {
                    self.start("Reading diff…", move || {
                        Update::Diff(git::diff(&repo, mode == DiffMode::Staged))
                    });
                }
            }
        }
    }

    fn select_commit(&mut self, commit: git::Commit) {
        if !self.busy {
            self.selected_commit = Some(commit);
            self.select_tab(Tab::Diff(DiffMode::Commit));
        }
    }

    fn compare_branches(&mut self) {
        let [Some(base), Some(head)] = self.comparison.selected.clone() else {
            return;
        };
        let Some(repo) = self.workspace.selected.clone() else {
            return;
        };
        self.patch.clear();
        self.comparison.compared = true;
        self.start("Comparing branches…", move || {
            Update::Diff(git::compare_branches(&repo, &base, &head))
        });
    }

    fn swap_draft(&mut self, next: Option<&Path>) {
        if self.workspace.selected.as_deref() == next {
            return;
        }
        if let Some(previous) = &self.workspace.selected {
            self.drafts
                .insert(previous.clone(), self.draft.text.clone());
        }
        self.draft = TextEdit::new(next.and_then(|p| self.drafts.remove(p)).unwrap_or_default());
        self.selected_changes = Default::default();
        self.tab = Tab::Changes;
    }

    fn poll(&mut self) -> bool {
        let Some(update) = self.task.take() else {
            if self.opening || self.tab != Tab::Terminal {
                return false;
            }
            return self
                .workspace
                .selected
                .as_ref()
                .and_then(|path| self.shells.get_mut(path))
                .is_some_and(Terminal::poll);
        };
        self.busy = false;
        match update {
            Update::Opened(result) => match result {
                Ok((path, snapshot)) => {
                    self.swap_draft(Some(&path));
                    self.workspace = Workspace {
                        selected: Some(path),
                        snapshot: Some(snapshot),
                        notice: String::new(),
                    };
                    self.opening = false;
                    self.selected_changes = Default::default();
                    self.config = ConfigForm::default();
                    self.github_section = github::Section::default();
                    self.github_entries.clear();
                    self.github_thread = None;
                    self.comparison = BranchComparison::default();
                    self.selected_commit = None;
                    self.tab = Tab::Changes;
                }
                Err(e) => {
                    self.opening = true;
                    self.workspace.notice = e;
                }
            },
            Update::Settings(entries, result) => {
                self.workspace.notice = result.err().unwrap_or_default();
                match entries {
                    Ok(entries) => self.config.entries = entries,
                    Err(err) => {
                        self.config.entries.clear();
                        if self.workspace.notice.is_empty() {
                            self.workspace.notice = err;
                        }
                    }
                }
            }
            Update::GitHub(result) => {
                self.workspace.notice.clear();
                self.github_entries = result.unwrap_or_else(|error| {
                    self.workspace.notice = error;
                    Vec::new()
                });
            }
            Update::GitHubConversation(result) => {
                if let Some(thread) = &mut self.github_thread {
                    match result {
                        Ok(conversation) => {
                            thread.conversation = Some(conversation);
                            self.workspace.notice.clear();
                        }
                        Err(error) => self.workspace.notice = error,
                    }
                }
            }
            Update::GitHubComment(result) => {
                if let Some(thread) = &mut self.github_thread {
                    match result {
                        Ok(conversation) => {
                            thread.draft = TextEdit::new("");
                            self.workspace.notice = match conversation {
                                Ok(conversation) => {
                                    thread.conversation = Some(conversation);
                                    "Comment posted.".into()
                                }
                                Err(error) => format!(
                                    "Comment posted. Could not refresh the conversation. {error}"
                                ),
                            };
                        }
                        Err(error) => self.workspace.notice = error,
                    }
                }
            }
            Update::GitHubDiff(result) => {
                if let Some(thread) = &mut self.github_thread {
                    match result {
                        Ok(patch) => {
                            thread.patch = Some(patch);
                            thread.diff_pages = 1;
                            self.workspace.notice.clear();
                        }
                        Err(error) => self.workspace.notice = error,
                    }
                }
            }
            Update::Terminal(result) => match result {
                Ok(shell) => {
                    if let Some(path) = &self.workspace.selected {
                        self.shells.insert(path.clone(), shell);
                    }
                    self.workspace.notice.clear();
                }
                Err(e) => self.workspace.notice = e,
            },
            Update::Repository {
                path,
                snapshot,
                outcome,
                committed,
            } => {
                self.swap_draft(Some(&path));
                self.workspace.selected = Some(path);
                if committed {
                    self.draft = TextEdit::new("");
                }
                self.workspace.notice = outcome.unwrap_or_else(|e| e);
                self.workspace.snapshot = snapshot
                    .map_err(|e| self.workspace.notice.push_str(&format!(" {e}")))
                    .ok();
                if let Some(snapshot) = &self.workspace.snapshot {
                    let [working, staged] = &self.selected_changes;
                    let mut remaining = [HashSet::new(), HashSet::new()];
                    for change in &snapshot.changes {
                        if change.unstaged() && working.contains(&change.path) {
                            remaining[0].insert(change.path.clone());
                        }
                        if change.staged() && staged.contains(&change.path) {
                            remaining[1].insert(change.path.clone());
                        }
                    }
                    self.selected_changes = remaining;
                }
                if matches!(self.tab, Tab::Diff(_)) {
                    self.tab = Tab::Changes;
                }
            }
            Update::Diff(result) => match result {
                Ok(patch) => {
                    self.patch = patch;
                    self.diff_pages = 1;
                    self.workspace.notice.clear();
                }
                Err(error) => self.workspace.notice = error,
            },
            Update::ComparisonBranches(result) => {
                self.workspace.notice.clear();
                let branches = result.unwrap_or_else(|error| {
                    self.workspace.notice = error;
                    Vec::new()
                });
                for choice in &mut self.comparison.selected {
                    if choice.as_ref().is_some_and(|name| !branches.contains(name)) {
                        *choice = None;
                    }
                }
                self.comparison.branches = branches;
            }
        }
        true
    }
}

fn main() {
    rune::run(AppSpec {
        min_size: Some(MIN_WINDOW_SIZE),
        setup: Some(|s: &mut Desk, waker| {
            s.task.set_waker(waker.clone());
            s.waker = Some(waker);
        }),
        poll: Some(|s: &mut Desk, _, _| s.poll()),
        ..AppSpec::new(
            "Gitdesk",
            (1000.0, 640.0),
            || Desk::new(rune::extra_args().first().map(PathBuf::from)),
            view::view,
        )
    });
}
