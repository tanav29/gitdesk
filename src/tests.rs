use super::*;
use rune::{Env, Runtime};
use std::{
    fs,
    process::Command,
    thread,
    time::{Duration, Instant},
};

fn fixture_git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn init(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fixture_git(dir, &["init", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Gitdesk Test"),
        ("user.email", "test@example.invalid"),
        ("commit.gpgSign", "false"),
        ("core.hooksPath", ".git/empty-hooks"),
    ] {
        fixture_git(dir, &["config", key, value]);
    }
}

fn settle(rt: &mut Runtime<Desk>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    rt.frame(1.0 / 60.0);
    while rt.state.busy {
        assert!(Instant::now() < deadline, "{}", rt.state.workspace.notice);
        thread::sleep(Duration::from_millis(10));
        rt.frame(1.0 / 60.0);
    }
    for _ in 0..45 {
        rt.frame(1.0 / 60.0);
    }
}

fn click(rt: &mut Runtime<Desk>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("Missing control {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn enter_path(rt: &mut Runtime<Desk>, path: &Path) {
    click(rt, "repo-path");
    rt.paste(path.to_string_lossy());
    settle(rt);
    click(rt, "open-path");
}

#[test]
fn stashes_restore_working_tree_and_index_and_only_pop_removes_the_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fixture_git(repo, &["config", "core.autocrlf", "false"]);
    fs::write(repo.join("tracked.txt"), "base\n").unwrap();
    fixture_git(repo, &["add", "tracked.txt"]);
    fixture_git(repo, &["commit", "-m", "Initial"]);
    fs::write(repo.join("tracked.txt"), "staged\n").unwrap();
    fixture_git(repo, &["add", "tracked.txt"]);
    fs::write(repo.join("tracked.txt"), "working\n").unwrap();
    fs::write(repo.join("untracked.txt"), "new\n").unwrap();

    let mut rt = Runtime::new(
        Desk::new(Some(repo.to_owned())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    click(&mut rt, "tab-Stashes");
    click(&mut rt, "stash-message");
    rt.paste("Save all three states");
    settle(&mut rt);
    click(&mut rt, "stash-include-untracked");
    assert!(rt.state.stash.include_untracked);
    click(&mut rt, "create-stash");
    let snapshot = rt.state.workspace.snapshot.as_ref().unwrap();
    assert!(
        snapshot.changes.is_empty(),
        "{}: {:?}",
        rt.state.workspace.notice,
        snapshot.changes
    );
    assert_eq!(snapshot.stashes.len(), 1);
    assert!(
        snapshot.stashes[0]
            .description
            .contains("Save all three states")
    );
    assert!(rt.state.stash.message.text.is_empty());

    click(&mut rt, "apply-stash@{0}");
    assert_eq!(fixture_git(repo, &["show", ":tracked.txt"]), "staged");
    assert_eq!(
        fs::read_to_string(repo.join("tracked.txt")).unwrap(),
        "working\n"
    );
    assert!(repo.join("untracked.txt").exists());
    assert_eq!(
        rt.state.workspace.snapshot.as_ref().unwrap().stashes.len(),
        1
    );

    click(&mut rt, "pop-stash@{0}");
    assert_eq!(
        rt.state.workspace.snapshot.as_ref().unwrap().stashes.len(),
        1
    );
    fixture_git(
        repo,
        &[
            "restore",
            "--source=HEAD",
            "--staged",
            "--worktree",
            "tracked.txt",
        ],
    );
    fs::remove_file(repo.join("untracked.txt")).unwrap();
    click(&mut rt, "refresh");
    click(&mut rt, "pop-stash@{0}");
    assert_eq!(fixture_git(repo, &["show", ":tracked.txt"]), "staged");
    assert_eq!(
        fs::read_to_string(repo.join("tracked.txt")).unwrap(),
        "working\n"
    );
    assert!(repo.join("untracked.txt").exists());
    assert!(
        rt.state
            .workspace
            .snapshot
            .as_ref()
            .unwrap()
            .stashes
            .is_empty()
    );
}

#[test]
fn commit_menu_supports_sign_off_empty_commits_and_amending() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    let mut rt = Runtime::new(
        Desk::new(Some(repo.to_owned())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);

    assert!(rt.accessibility_action(
        "commit-options-button",
        rune::accesskit::Action::Expand,
        None,
    ));
    settle(&mut rt);
    click(&mut rt, "commit-options-item0");
    click(&mut rt, "commit-options-trigger");
    click(&mut rt, "commit-options-item4");
    click(&mut rt, "commit-message");
    rt.paste("Checkpoint");
    settle(&mut rt);
    click(&mut rt, "commit");
    let message = fixture_git(repo, &["log", "-1", "--format=%B"]);
    assert!(message.contains("Signed-off-by: Gitdesk Test <test@example.invalid>"));
    assert!(fixture_git(repo, &["ls-tree", "-r", "--name-only", "HEAD"]).is_empty());
    assert!(!rt.state.commit_options.sign_off && !rt.state.commit_options.allow_empty);

    fs::write(repo.join("added.txt"), "amended contents").unwrap();
    click(&mut rt, "refresh");
    click(&mut rt, "Stage-added.txt");
    click(&mut rt, "commit-options-trigger");
    click(&mut rt, "commit-options-item3");
    click(&mut rt, "commit");
    assert_eq!(fixture_git(repo, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(fixture_git(repo, &["log", "-1", "--format=%B"]), message);
    assert_eq!(
        fixture_git(repo, &["show", "HEAD:added.txt"]),
        "amended contents"
    );
    assert!(!rt.state.commit_options.amend);
}

#[test]
fn history_graph_opens_root_and_merge_patches_in_the_diff_tab() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fs::write(repo.join("base.txt"), "root contents\n").unwrap();
    fixture_git(repo, &["add", "--", "base.txt"]);
    fixture_git(repo, &["commit", "-m", "Initial commit"]);
    let root = fixture_git(repo, &["rev-parse", "HEAD"]);

    fixture_git(repo, &["switch", "-c", "topic"]);
    fs::write(repo.join("topic.txt"), "topic contents\n").unwrap();
    fixture_git(repo, &["add", "--", "topic.txt"]);
    fixture_git(repo, &["commit", "-m", "Topic commit"]);
    fixture_git(repo, &["switch", "main"]);
    fs::write(repo.join("main.txt"), "main contents\n").unwrap();
    fixture_git(repo, &["add", "--", "main.txt"]);
    fixture_git(repo, &["commit", "-m", "Main commit"]);
    fixture_git(repo, &["merge", "--no-ff", "--no-edit", "topic"]);
    let merge = fixture_git(repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join("base.txt"), "uncommitted contents\n").unwrap();

    let mut rt = Runtime::new(
        Desk::new(Some(repo.to_owned())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    click(&mut rt, "tab-History");
    let history = &rt.state.workspace.snapshot.as_ref().unwrap().history;
    assert!(
        history
            .iter()
            .any(|row| row.commit.is_none() && row.graph.contains('\\'))
    );
    assert_eq!(history.iter().filter(|row| row.commit.is_some()).count(), 4);

    click(&mut rt, &format!("commit-{merge}"));
    assert!(rt.state.tab == Tab::Diff(DiffMode::Commit));
    assert!(rt.state.patch.contains("+topic contents"));
    assert!(!rt.state.patch.contains("main contents"));
    assert!(!rt.state.patch.contains("uncommitted contents"));
    click(&mut rt, "commit-back-to-history");
    click(&mut rt, &format!("commit-{root}"));
    assert!(rt.state.patch.contains("+root contents"));
    assert!(!rt.state.patch.contains("topic contents"));
    assert_eq!(fixture_git(repo, &["rev-parse", "HEAD"]), merge);
}

#[test]
fn stages_and_unstages_only_selected_files_before_the_first_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    for file in ["- first.txt", "second [2].txt", "untouched.txt"] {
        fs::write(repo.join(file), file).unwrap();
    }
    let mut rt = Runtime::new(
        Desk::new(Some(repo.to_owned())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    click(&mut rt, "select-Stage-- first.txt");
    click(&mut rt, "select-Stage-second [2].txt");
    click(&mut rt, "Stage-selected");
    assert_eq!(
        fixture_git(repo, &["diff", "--cached", "--name-only"]),
        "- first.txt\nsecond [2].txt"
    );
    assert!(rt.state.selected_changes[0].is_empty());

    click(&mut rt, "select-all-Unstage");
    click(&mut rt, "Unstage-selected");
    assert!(fixture_git(repo, &["diff", "--cached", "--name-only"]).is_empty());
    assert!(rt.state.selected_changes[1].is_empty());
    assert_eq!(
        fs::read_to_string(repo.join("- first.txt")).unwrap(),
        "- first.txt"
    );
}

#[test]
fn staging_commit_and_drafts_follow_the_selected_repository() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("first");
    let b = tmp.path().join("second");
    for repo in [&a, &b] {
        init(repo);
        fs::write(repo.join("-note [1].txt"), "draft").unwrap();
    }
    let mut rt = Runtime::new(
        Desk::new(None),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    enter_path(&mut rt, tmp.path());
    assert!(rt.state.opening);
    assert!(rt.state.workspace.selected.is_none());
    rt.update(|s, _| s.path = TextEdit::new(""));
    enter_path(&mut rt, &a);
    click(&mut rt, "commit-message");
    rt.paste("First draft");
    settle(&mut rt);
    assert_eq!(rt.state.draft.text, "First draft");
    click(&mut rt, "change-repository");
    enter_path(&mut rt, &b);
    assert_eq!(rt.state.workspace.selected.as_ref(), Some(&b));
    assert!(rt.state.draft.text.is_empty());
    click(&mut rt, "Stage--note [1].txt");
    assert!(git::snapshot(&b).unwrap().changes[0].staged());
    click(&mut rt, "Unstage--note [1].txt");
    assert!(!git::snapshot(&b).unwrap().changes[0].staged());
    click(&mut rt, "Stage--note [1].txt");
    click(&mut rt, "commit-message");
    rt.paste("Commit from Gitdesk");
    settle(&mut rt);
    click(&mut rt, "commit");
    assert_eq!(
        fixture_git(&b, &["log", "-1", "--format=%s"]),
        "Commit from Gitdesk"
    );
    assert!(rt.state.draft.text.is_empty());
    click(&mut rt, "tab-History");
    assert!(rt.state.tab == Tab::History);
    click(&mut rt, "change-repository");
    enter_path(&mut rt, &a);
    assert_eq!(rt.state.draft.text, "First draft");
    assert!(fixture_git(&a, &["diff", "--cached", "--name-only"]).is_empty());
    assert!(!git::snapshot(&a).unwrap().has_head);
}

#[test]
fn worktrees_bare_repositories_and_remote_actions() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("project");
    init(&repo);
    fs::write(repo.join("file"), "one\n").unwrap();
    Action::Stage(vec!["file".into()]).apply(&repo).unwrap();
    Action::Commit("Initial".into(), CommitOptions::default())
        .apply(&repo)
        .unwrap();
    fs::write(repo.join("file"), "two\n").unwrap();
    assert!(git::diff(&repo, false).unwrap().contains("+two"));
    Action::Stage(vec!["file".into()]).apply(&repo).unwrap();
    assert!(git::diff(&repo, true).unwrap().contains("+two"));
    Action::Unstage(vec!["file".into()], true)
        .apply(&repo)
        .unwrap();
    assert!(git::diff(&repo, true).unwrap().is_empty());
    fixture_git(&repo, &["branch", "topic"]);
    Action::Switch("topic".into()).apply(&repo).unwrap();
    fixture_git(&repo, &["worktree", "add", "../linked", "main"]);
    fixture_git(tmp.path(), &["init", "--bare", "-b", "main", "origin.git"]);
    let remote = tmp.path().join("origin.git");
    fixture_git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    fixture_git(&repo, &["config", "push.autoSetupRemote", "true"]);
    for action in [Action::Push, Action::Fetch, Action::Pull] {
        action.apply(&repo).unwrap();
    }
    assert_eq!(
        fixture_git(&remote, &["rev-parse", "topic"]),
        fixture_git(&repo, &["rev-parse", "HEAD"])
    );
    assert!(git::snapshot(&remote).unwrap().bare);
    assert_eq!(
        git::snapshot(&tmp.path().join("linked")).unwrap().branch,
        "main"
    );
    assert_eq!(git::repository_path(&remote).unwrap(), remote);
    assert_eq!(
        git::repository_path(&tmp.path().join("linked")).unwrap(),
        tmp.path().join("linked")
    );
}

fn await_terminal(rt: &mut Runtime<Desk>, ready: impl Fn(&Terminal) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        rt.frame(1.0 / 60.0);
        if rt.state.shell().is_some_and(&ready) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Terminal timed out: {:?}",
            rt.state.shell().map(|t| t.screen(|s| s.contents()))
        );
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn compares_local_and_remote_branch_tips_without_switching_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fs::write(repo.join("sample.txt"), "base\n").unwrap();
    fixture_git(repo, &["add", "sample.txt"]);
    fixture_git(repo, &["commit", "-m", "base"]);
    fixture_git(repo, &["branch", "topic"]);
    fixture_git(repo, &["switch", "topic"]);
    fs::write(repo.join("sample.txt"), "topic\n").unwrap();
    fixture_git(repo, &["commit", "-am", "topic"]);
    fixture_git(repo, &["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    fixture_git(
        repo,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/topic",
        ],
    );
    fixture_git(repo, &["switch", "main"]);
    fs::write(repo.join("sample.txt"), "working tree change\n").unwrap();

    let mut rt = Runtime::new(
        Desk::new(Some(repo.to_owned())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    click(&mut rt, "tab-Diff");
    assert!(rt.state.patch.contains("+working tree change"));
    click(&mut rt, "Compare branches");
    assert_eq!(
        rt.state.comparison.branches,
        ["heads/main", "heads/topic", "remotes/origin/topic"]
    );
    click(&mut rt, "base-branch-trigger");
    click(&mut rt, "option0");
    click(&mut rt, "compare-branch-trigger");
    click(&mut rt, "option2");
    click(&mut rt, "run-branch-comparison");
    assert!(rt.state.patch.contains("-base"));
    assert!(rt.state.patch.contains("+topic"));
    assert!(!rt.state.patch.contains("working tree change"));
    assert_eq!(fixture_git(repo, &["branch", "--show-current"]), "main");
    assert_eq!(
        fs::read_to_string(repo.join("sample.txt")).unwrap(),
        "working tree change\n"
    );
}

#[test]
fn settings_and_interactive_git_use_the_open_repository() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repository with spaces");
    init(&repo);
    let mut rt = Runtime::new(
        Desk::new(Some(repo.clone())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    click(&mut rt, "tab-Settings");
    click(&mut rt, "config-key");
    rt.paste("alias.make-topic");
    settle(&mut rt);
    click(&mut rt, "config-value");
    rt.paste("branch");
    settle(&mut rt);
    click(&mut rt, "config-Save");
    assert_eq!(
        fixture_git(&repo, &["config", "--local", "alias.make-topic"]),
        "branch"
    );
    assert!(
        rt.state
            .config
            .entries
            .iter()
            .any(|(k, v)| k == "alias.make-topic" && v == "branch")
    );

    fs::write(repo.join("sample.txt"), "one\n").unwrap();
    Action::Stage(vec!["sample.txt".into()])
        .apply(&repo)
        .unwrap();
    Action::Commit("first".into(), CommitOptions::default())
        .apply(&repo)
        .unwrap();
    fs::write(repo.join("sample.txt"), "two\n").unwrap();
    click(&mut rt, "tab-Terminal");
    await_terminal(&mut rt, |s| s.screen(|sc| !sc.contents().trim().is_empty()));
    click(&mut rt, "terminal-screen");
    for ch in "git -c color.ui=false add -p".chars() {
        rt.key(rune::KeyEvent {
            key: rune::Key::Text(ch.to_string()),
            mods: Default::default(),
        });
    }
    rt.key(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: Default::default(),
    });
    await_terminal(&mut rt, |s| {
        s.screen(|sc| sc.contents().contains("Stage this hunk"))
    });
    rt.key(rune::KeyEvent {
        key: rune::Key::Text("y".into()),
        mods: Default::default(),
    });
    rt.key(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: Default::default(),
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while git::diff(&repo, true).unwrap().is_empty() {
        assert!(
            Instant::now() < deadline,
            "interactive staging did not finish"
        );
        rt.frame(1.0 / 60.0);
        thread::sleep(Duration::from_millis(25));
    }
    click(&mut rt, "tab-Changes");
    assert!(rt.state.workspace.snapshot.as_ref().unwrap().changes[0].staged());
}

#[test]
fn github_renders_pull_requests_and_workflow_status() {
    let mut desk = Desk::new(None);
    desk.opening = false;
    desk.tab = Tab::GitHub;
    let pull_requests = github::parse(
        br#"[{"number":42,"title":"Add a feature","url":"https://github.com/example/repo/pull/42","isDraft":true}]"#,
    )
    .unwrap();
    assert_eq!(pull_requests[0].number, Some(42));
    assert_eq!(pull_requests[0].status().as_deref(), Some("Draft"));
    let runs = github::parse(
        br#"[{"displayTitle":"Build desktop app","url":"https://github.com/example/repo/actions/runs/42","workflowName":"Build","headBranch":"main","status":"completed","conclusion":"success"}]"#,
    )
    .unwrap();
    assert_eq!(runs[0].title, "Build desktop app");
    assert_eq!(runs[0].status().as_deref(), Some("success"));
    desk.github_entries = pull_requests;

    let mut rt = Runtime::new(
        desk,
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    settle(&mut rt);
    for key in ["github-Pull requests", "github-Issues", "github-Actions"] {
        assert!(rt.find(key).is_some(), "Missing {key}");
    }
    assert!(
        rt.find("github-entry-https://github.com/example/repo/pull/42")
            .is_some()
    );
}

#[test]
fn github_keeps_the_comment_draft_when_inspecting_changed_files() {
    let mut desk = Desk::new(None);
    desk.opening = false;
    desk.tab = Tab::GitHub;
    desk.github_entries = github::parse(
        br#"[{"number":42,"title":"Add a feature","url":"https://github.com/example/repo/pull/42"}]"#,
    )
    .unwrap();
    let mut rt = Runtime::new(
        desk,
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    settle(&mut rt);
    click(&mut rt, "github-details-42");
    rt.update(|s, _| {
        let thread = s.github_thread.as_mut().unwrap();
        thread.conversation = Some(serde_json::from_str(
            r#"{"author":{"login":"octocat"},"body":"Description","comments":[{"author":null,"body":"A comment"}]}"#,
        ).unwrap());
        thread.patch = Some(
            "diff --git a/file.txt b/file.txt\n@@ -1 +1 @@\n-before\n+after\n".into(),
        );
    });
    settle(&mut rt);
    click(&mut rt, "github-comment-body");
    rt.paste("First line\nSecond line");
    settle(&mut rt);
    click(&mut rt, "Files changed");
    assert!(rt.find("github-changed-files").is_some());
    assert!(rt.find("github-comment-body").is_none());
    click(&mut rt, "Conversation");
    assert_eq!(
        rt.state.github_thread.as_ref().unwrap().draft.text,
        "First line\nSecond line"
    );
    rt.update(|s, _| s.github_section = github::Section::Issues);
    settle(&mut rt);
    assert!(rt.find("Files changed").is_none());
    assert!(rt.find("github-comment-body").is_some());
    click(&mut rt, "github-back");
    assert!(rt.find("github-details-42").is_some());
}
