mod git;
mod github;
#[cfg(test)]
mod tests;
mod view;

use git::{Action, CommitOptions, ConfigEdit, Entries, Scope, Snapshot};
use rune::{AppSpec, Chord, KeyEvent, Latest, TextEdit, Tx};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const PRIMARY_KEY: &str = if cfg!(target_os = "macos") {
    "⌘"
} else {
    "Ctrl"
};

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
    Stashes,
    Settings,
    GitHub,
    Tags,
    Worktrees,
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
struct TagForm {
    name: TextEdit,
    target: TextEdit,
    message: TextEdit,
    deleting: Option<git::Tag>,
    publishing: Option<git::Tag>,
    menu: Option<String>,
    remote: Option<String>,
    remote_picker: bool,
}

#[derive(Default)]
struct WorktreeForm {
    path: TextEdit,
    branch: TextEdit,
    existing: Option<String>,
    picker: bool,
    start: TextEdit,
    mode: git::WorktreeMode,
    removing: Option<PathBuf>,
}

struct StashForm {
    message: TextEdit,
    include_untracked: bool,
    restore_index: bool,
}

impl Default for StashForm {
    fn default() -> Self {
        Self {
            message: TextEdit::new(""),
            include_untracked: false,
            restore_index: true,
        }
    }
}

#[derive(Default)]
struct BranchComparison {
    branches: Vec<String>,
    selected: [Option<String>; 2],
    picker: Option<usize>,
    compared: bool,
}

#[derive(Clone)]
enum BranchPrompt {
    Rename(String),
    CreateFrom(String),
    Delete(String),
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
    Repository {
        path: PathBuf,
        snapshot: git::Result<Snapshot>,
        outcome: git::Result<String>,
        completed: Option<Action>,
    },
    IntegrationPreview(git::Result<git::Integration>),
    TaggedCommit(git::Result<git::Commit>),
    Diff(git::Result<String>),
    ComparisonBranches(git::Result<Vec<String>>),
}

#[derive(Clone, Copy)]
enum ExternalTarget {
    FileManager,
    Terminal,
    Editor,
}

impl ExternalTarget {
    fn label(self) -> &'static str {
        match self {
            Self::FileManager => "file manager",
            Self::Terminal => "terminal",
            Self::Editor => "VS Code",
        }
    }
}

fn launch_external(path: &Path, target: ExternalTarget) -> std::io::Result<()> {
    use std::process::Command;
    #[cfg(target_os = "windows")]
    let mut command = match target {
        ExternalTarget::FileManager => {
            let mut c = Command::new("explorer.exe");
            c.arg(path);
            c
        }
        ExternalTarget::Terminal => {
            let mut c = Command::new("cmd.exe");
            c.args(["/c", "start", "", "cmd.exe"]);
            c.current_dir(path);
            c
        }
        ExternalTarget::Editor => {
            let mut c = Command::new("code.cmd");
            c.arg(path);
            c
        }
    };
    #[cfg(target_os = "macos")]
    let mut command = match target {
        ExternalTarget::FileManager => {
            let mut c = Command::new("open");
            c.arg(path);
            c
        }
        ExternalTarget::Terminal => {
            let mut c = Command::new("open");
            c.args(["-a", "Terminal"]).arg(path);
            c
        }
        ExternalTarget::Editor => {
            let mut c = Command::new("open");
            c.args(["-a", "Visual Studio Code"]).arg(path);
            c
        }
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = match target {
        ExternalTarget::FileManager => {
            let mut c = Command::new("xdg-open");
            c.arg(path);
            c
        }
        ExternalTarget::Terminal => {
            let mut c = Command::new("x-terminal-emulator");
            c.current_dir(path);
            c
        }
        ExternalTarget::Editor => {
            let mut c = Command::new("code");
            c.arg(path);
            c
        }
    };
    command.spawn().map(|_| ())
}

struct Desk {
    workspace: Workspace,
    tab: Tab,
    draft: TextEdit,
    drafts: HashMap<PathBuf, String>,
    commit_options: CommitOptions,
    commit_menu_open: bool,
    selected_changes: [HashSet<OsString>; 2],
    patch: String,
    diff_pages: usize,
    selected_commit: Option<git::Commit>,
    history_form: Option<view::HistoryForm>,
    history_menu: bool,
    integration: Option<git::Integration>,
    recovery: Option<(git::Operation, git::Recovery)>,
    comparison: BranchComparison,
    opening: bool,
    path: TextEdit,
    branch_name: TextEdit,
    branch_menu: Option<String>,
    branch_prompt: Option<BranchPrompt>,
    branch_prompt_name: TextEdit,
    config: ConfigForm,
    tags: TagForm,
    worktrees: WorktreeForm,
    stash: StashForm,
    github_section: github::Section,
    github_entries: Vec<github::Entry>,
    github_thread: Option<GitHubThread>,
    open_menu: bool,
    task: Latest<Update>,
    busy: bool,
}

impl Desk {
    fn new(initial: Option<PathBuf>) -> Self {
        let mut desk = Self {
            workspace: Workspace::default(),
            opening: true,
            path: TextEdit::new(""),
            branch_name: TextEdit::new(""),
            branch_menu: None,
            branch_prompt: None,
            branch_prompt_name: TextEdit::new(""),
            config: ConfigForm::default(),
            tags: TagForm::default(),
            worktrees: WorktreeForm::default(),
            stash: StashForm::default(),
            github_section: github::Section::default(),
            github_entries: Vec::new(),
            github_thread: None,
            open_menu: false,
            tab: Tab::Changes,
            draft: TextEdit::new(""),
            drafts: HashMap::new(),
            commit_options: CommitOptions::default(),
            commit_menu_open: false,
            selected_changes: Default::default(),
            patch: String::new(),
            diff_pages: 1,
            selected_commit: None,
            history_form: None,
            history_menu: false,
            integration: None,
            recovery: None,
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
        self.commit_menu_open = false;
        self.branch_menu = None;
        self.history_menu = false;
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

    fn open_external(&mut self, target: ExternalTarget) {
        self.open_menu = false;
        let Some(path) = self.workspace.selected.as_deref() else {
            return;
        };
        self.workspace.notice = launch_external(path, target)
            .err()
            .map(|error| format!("Could not open {}: {error}", target.label()))
            .unwrap_or_default();
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
            let completed = action.filter(|_| outcome.is_ok());
            let snapshot = git::snapshot(&path);
            Update::Repository {
                path,
                snapshot,
                outcome,
                completed,
            }
        });
    }

    fn act(&mut self, action: Action) {
        if let Some(repo) = self.workspace.selected.clone() {
            self.load_repo(repo, Some(action));
        }
    }

    fn create_branch(&mut self) {
        let name = self.branch_name.text.trim().to_owned();
        if !name.is_empty() {
            self.act(Action::CreateBranch(name));
        }
    }

    fn prompt_branch(&mut self, prompt: BranchPrompt) {
        self.branch_menu = None;
        self.branch_prompt_name = match &prompt {
            BranchPrompt::Rename(name) => TextEdit::new(name),
            _ => TextEdit::new(""),
        };
        self.branch_prompt = Some(prompt);
        self.workspace.notice.clear();
    }

    fn submit_branch_prompt(&mut self) {
        let name = self.branch_prompt_name.text.trim().to_owned();
        let action = match &self.branch_prompt {
            Some(BranchPrompt::Rename(old)) if !name.is_empty() => {
                Action::RenameBranch(old.clone(), name)
            }
            Some(BranchPrompt::CreateFrom(source)) if !name.is_empty() => {
                Action::CreateBranchFrom(name, source.clone())
            }
            Some(BranchPrompt::Delete(branch)) => Action::DeleteBranch(branch.clone()),
            _ => return,
        };
        self.act(action);
    }

    fn show_opener(&mut self) {
        self.opening = true;
        self.workspace.notice.clear();
        self.path = TextEdit::new("");
    }

    fn refresh(&mut self) {
        if self.tab == Tab::Diff(DiffMode::Compare) && self.comparison.compared {
            self.compare_branches();
        } else {
            self.select_tab(self.tab);
        }
    }

    fn tabs(&self) -> [(&'static str, Tab); 9] {
        let diff = if self.selected_commit.is_some() {
            DiffMode::Commit
        } else if self
            .workspace
            .snapshot
            .as_ref()
            .is_some_and(|data| data.bare)
        {
            DiffMode::Compare
        } else {
            DiffMode::WorkingTree
        };
        [
            ("Changes", Tab::Changes),
            ("Diff", Tab::Diff(diff)),
            ("History", Tab::History),
            ("Branches", Tab::Branches),
            ("Stashes", Tab::Stashes),
            ("Settings", Tab::Settings),
            ("GitHub", Tab::GitHub),
            ("Tags", Tab::Tags),
            ("Worktrees", Tab::Worktrees),
        ]
    }

    fn key(&mut self, event: &KeyEvent, tx: &mut Tx) -> bool {
        if self.busy {
            return false;
        }
        let pressed = |shortcut: &str| {
            Chord::parse(shortcut)
                .expect("valid application shortcut")
                .matches_tx(event, tx)
        };
        if self.opening {
            if self.workspace.selected.is_some() && pressed("escape") {
                self.opening = false;
                self.workspace.notice.clear();
                return true;
            }
            return false;
        }
        if pressed("escape")
            && (self.integration.take().is_some() || self.recovery.take().is_some())
        {
            return true;
        }
        if pressed("escape") && self.history_form.take().is_some() {
            return true;
        }
        if pressed("escape") && self.branch_prompt.is_some() {
            self.branch_prompt = None;
            return true;
        }
        if pressed("escape")
            && (self.tags.deleting.take().is_some()
                || self.tags.publishing.take().is_some()
                || self.worktrees.removing.take().is_some())
        {
            self.tags.remote_picker = false;
            return true;
        }
        if pressed("mod+o") {
            self.show_opener();
            return true;
        }
        if pressed("f5") {
            self.refresh();
            return true;
        }
        let bare = self
            .workspace
            .snapshot
            .as_ref()
            .is_some_and(|data| data.bare);
        for ((_, tab), shortcut) in self.tabs().into_iter().zip([
            "mod+1", "mod+2", "mod+3", "mod+4", "mod+5", "mod+6", "mod+7", "mod+8", "mod+9",
        ]) {
            if !(bare && matches!(tab, Tab::Changes | Tab::Stashes)) && pressed(shortcut) {
                self.select_tab(tab);
                return true;
            }
        }
        false
    }

    fn select_tab(&mut self, tab: Tab) {
        if self.busy {
            return;
        }
        self.history_form = None;
        self.integration = None;
        self.recovery = None;
        self.tags.deleting = None;
        self.tags.publishing = None;
        self.tags.remote_picker = false;
        self.tags.menu = None;
        self.worktrees.removing = None;
        self.worktrees.picker = false;
        self.tab = tab;
        self.comparison.picker = None;
        if tab == Tab::GitHub {
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
            return false;
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
                    self.commit_options = CommitOptions::default();
                    self.config = ConfigForm::default();
                    self.tags = TagForm::default();
                    self.worktrees = WorktreeForm::default();
                    self.stash = StashForm::default();
                    self.branch_name = TextEdit::new("");
                    self.branch_menu = None;
                    self.branch_prompt = None;
                    self.github_section = github::Section::default();
                    self.github_entries.clear();
                    self.github_thread = None;
                    self.comparison = BranchComparison::default();
                    self.history_form = None;
                    self.integration = None;
                    self.recovery = None;
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
            Update::Repository {
                path,
                snapshot,
                outcome,
                completed,
            } => {
                self.swap_draft(Some(&path));
                self.workspace.selected = Some(path);
                if matches!(completed, Some(Action::Commit(..))) {
                    self.draft = TextEdit::new("");
                    self.commit_options = CommitOptions::default();
                }
                if matches!(completed, Some(Action::Stash(..))) {
                    self.stash.message = TextEdit::new("");
                }
                if matches!(completed, Some(Action::AddWorktree { .. })) {
                    self.worktrees = WorktreeForm::default();
                }
                if matches!(completed, Some(Action::RemoveWorktree(_))) {
                    self.worktrees.removing = None;
                }
                if matches!(completed, Some(Action::CreateBranch(..))) {
                    self.branch_name = TextEdit::new("");
                }
                match &completed {
                    Some(Action::CreateTag { .. }) => self.tags = TagForm::default(),
                    Some(Action::DeleteTag(_)) => self.tags.deleting = None,
                    Some(Action::PushTag(..)) => self.tags.publishing = None,
                    _ => {}
                }
                if matches!(
                    completed,
                    Some(
                        Action::RenameBranch(..)
                            | Action::CreateBranchFrom(..)
                            | Action::DeleteBranch(..)
                    )
                ) {
                    self.branch_prompt = None;
                }
                if matches!(completed, Some(Action::Integrate(_))) {
                    self.integration = None;
                }
                self.recovery = None;
                self.workspace.notice = outcome.unwrap_or_else(|e| e);
                self.workspace.snapshot = snapshot
                    .map_err(|e| self.workspace.notice.push_str(&format!(" {e}")))
                    .ok();
                if let Some(snapshot) = &self.workspace.snapshot {
                    if snapshot.operation.is_some()
                        && (self.integration.take().is_some() || self.history_form.is_some())
                    {
                        self.tab = Tab::Changes;
                    }
                    if !snapshot.has_head {
                        self.commit_options.amend = false;
                    }
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
                if matches!(completed, Some(Action::History { .. }))
                    || self
                        .workspace
                        .snapshot
                        .as_ref()
                        .is_some_and(|d| d.operation.is_some())
                {
                    self.history_form = None;
                }
                if matches!(self.tab, Tab::Diff(_)) && self.history_form.is_none() {
                    self.tab = Tab::Changes;
                }
            }
            Update::TaggedCommit(result) => match result {
                Ok(commit) => self.select_commit(commit),
                Err(error) => self.workspace.notice = error,
            },
            Update::IntegrationPreview(result) => match result {
                Ok(plan) => {
                    self.integration = Some(plan);
                    self.workspace.notice.clear();
                }
                Err(error) => self.workspace.notice = error,
            },
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
    if let Err(error) = rune::set_app_icon(include_bytes!("../assets/icon.png").as_slice()) {
        eprintln!("Could not load application icon: {error}");
    }
    rune::run(AppSpec {
        min_size: Some(MIN_WINDOW_SIZE),
        setup: Some(|s: &mut Desk, waker| {
            s.task.set_waker(waker);
        }),
        poll: Some(|s: &mut Desk, _, _| s.poll()),
        on_key: Some(Desk::key),
        ..AppSpec::new(
            "GitDesk",
            (1000.0, 640.0),
            || Desk::new(rune::extra_args().first().map(PathBuf::from)),
            view::view,
        )
    });
}
