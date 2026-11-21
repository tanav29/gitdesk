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

#[test]
fn branch_management_renames_copies_and_safely_deletes() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fs::write(repo.join("base"), "base").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "base"]);
    git::Action::CreateBranchFrom("copy".into(), "main".into())
        .apply(repo)
        .unwrap();
    git::Action::RenameBranch("copy".into(), "renamed".into())
        .apply(repo)
        .unwrap();
    assert!(
        git::Action::RenameBranch("renamed".into(), "-invalid".into())
            .apply(repo)
            .is_err()
    );
    fixture_git(repo, &["switch", "renamed"]);
    fs::write(repo.join("new"), "new").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "unmerged"]);
    fixture_git(repo, &["switch", "main"]);
    assert!(
        git::Action::DeleteBranch("renamed".into())
            .apply(repo)
            .is_err()
    );
    assert!(fixture_git(repo, &["branch", "--list", "renamed"]).contains("renamed"));
    fixture_git(repo, &["merge", "--ff-only", "renamed"]);
    git::Action::DeleteBranch("renamed".into())
        .apply(repo)
        .unwrap();
    assert!(fixture_git(repo, &["branch", "--list", "renamed"]).is_empty());
}

#[test]
fn branch_actions_create_merge_and_rebase_current_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fs::write(repo.join("base"), "base").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "base"]);
    git::Action::CreateBranch("topic".into())
        .apply(repo)
        .unwrap();
    assert!(
        git::Action::CreateBranch("-invalid".into())
            .apply(repo)
            .is_err()
    );
    fixture_git(repo, &["switch", "topic"]);
    fs::write(repo.join("topic"), "topic").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "topic"]);
    fixture_git(repo, &["switch", "main"]);
    Action::Integrate(
        git::Integration::preview(repo, "refs/heads/topic".into(), git::IntegrationMode::Merge)
            .unwrap(),
    )
    .apply(repo)
    .unwrap();
    assert!(repo.join("topic").exists());
    git::Action::CreateBranch("feature".into())
        .apply(repo)
        .unwrap();
    fs::write(repo.join("main"), "main").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "main"]);
    fixture_git(repo, &["switch", "feature"]);
    fs::write(repo.join("feature"), "feature").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "feature"]);
    Action::Integrate(
        git::Integration::preview(repo, "refs/heads/main".into(), git::IntegrationMode::Rebase)
            .unwrap(),
    )
    .apply(repo)
    .unwrap();
    assert_eq!(
        fixture_git(repo, &["rev-parse", "HEAD^"]),
        fixture_git(repo, &["rev-parse", "main"])
    );
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
    let topic = fixture_git(repo, &["rev-parse", "HEAD"]);
    fixture_git(repo, &["switch", "main"]);
    fs::write(repo.join("main.txt"), "main contents\n").unwrap();
    fixture_git(repo, &["add", "--", "main.txt"]);
    fixture_git(repo, &["commit", "-m", "Main commit"]);
    let main = fixture_git(repo, &["rev-parse", "HEAD"]);
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
    assert_eq!(history.len(), 4);
    assert_eq!(history[0].id, merge);
    assert_eq!(history[0].parents, [main, topic]);
    assert_eq!(history[1].parents, [root.as_str()]);
    assert_eq!(history[2].parents, [root.as_str()]);
    assert_eq!(history[3].id, root);
    assert!(history[3].parents.is_empty());

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
fn settings_use_the_open_repository() {
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

#[test]
fn tags_create_at_an_explicit_commit_and_protect_replaced_tags() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fixture_git(repo, &["commit", "--allow-empty", "-m", "first"]);
    fixture_git(repo, &["commit", "--allow-empty", "-m", "second"]);
    for (name, target, message) in [("v1", "HEAD^", "First release"), ("latest", "", "")] {
        Action::CreateTag {
            name: name.into(),
            target: target.into(),
            message: message.into(),
        }
        .apply(repo)
        .unwrap();
    }
    let tags = git::snapshot(repo).unwrap().tags;
    let release = tags.iter().find(|t| t.name == "v1").unwrap();
    assert!(release.annotated);
    assert_eq!(release.subject, "First release");
    assert_eq!(
        release.short_target,
        fixture_git(repo, &["rev-parse", "--short", "HEAD^"])
    );
    let latest = tags.iter().find(|t| t.name == "latest").unwrap();
    assert!(!latest.annotated);
    fixture_git(repo, &["tag", "-f", "latest", "HEAD^"]);
    assert!(Action::DeleteTag(latest.clone()).apply(repo).is_err());
    Action::DeleteTag(release.clone()).apply(repo).unwrap();
    assert_eq!(fixture_git(repo, &["tag", "--list"]), "latest");
}

#[test]
fn linked_worktrees_can_be_created_opened_locked_and_safely_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("main");
    init(&repo);
    fixture_git(&repo, &["commit", "--allow-empty", "-m", "base"]);
    let path = tmp.path().join("new tree");
    Action::AddWorktree {
        path: path.clone(),
        mode: git::WorktreeMode::NewBranch,
        branch: "topic".into(),
        start: "HEAD".into(),
    }
    .apply(&repo)
    .unwrap();
    let trees = git::snapshot(&path).unwrap().worktrees;
    let tree = trees.iter().find(|t| t.current).unwrap();
    assert!(!tree.main);
    assert_eq!(tree.branch.as_deref(), Some("topic"));
    assert!(
        Action::RemoveWorktree(tree.path.clone())
            .apply(&path)
            .is_err()
    );
    Action::LockWorktree(tree.path.clone(), true)
        .apply(&repo)
        .unwrap();
    assert!(
        Action::RemoveWorktree(tree.path.clone())
            .apply(&repo)
            .is_err()
    );
    Action::LockWorktree(tree.path.clone(), false)
        .apply(&repo)
        .unwrap();
    fs::write(path.join("draft"), "keep").unwrap();
    assert!(
        Action::RemoveWorktree(tree.path.clone())
            .apply(&repo)
            .is_err()
    );
    fs::remove_file(path.join("draft")).unwrap();
    Action::RemoveWorktree(tree.path.clone())
        .apply(&repo)
        .unwrap();
    assert!(!path.exists());
    for mode in [git::WorktreeMode::Existing, git::WorktreeMode::Detached] {
        Action::AddWorktree {
            path: path.clone(),
            mode,
            branch: "topic".into(),
            start: "".into(),
        }
        .apply(&repo)
        .unwrap();
        let trees = git::snapshot(&path).unwrap().worktrees;
        let tree = trees.iter().find(|t| t.current).unwrap();
        assert_eq!(tree.branch.is_none(), mode == git::WorktreeMode::Detached);
        Action::RemoveWorktree(tree.path.clone())
            .apply(&repo)
            .unwrap();
    }
}

#[test]
fn history_actions_squash_through_root_preserve_edits_and_reject_stale_resets() {
    use git::{HistoryAction, ResetMode};
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    for name in ["one", "two", "three"] {
        fs::write(repo.join(name), name).unwrap();
        fixture_git(repo, &["add", name]);
        fixture_git(repo, &["commit", "-m", name]);
    }
    let snapshot = git::snapshot(repo).unwrap();
    let root = snapshot.history.last().unwrap().id.clone();
    let last = snapshot.checkout.id.clone().unwrap();
    let action = |checkout: git::Checkout, target: &str, action| Action::History {
        checkout,
        target: target.to_owned(),
        action,
    };
    fs::write(repo.join("two"), "staged").unwrap();
    fixture_git(repo, &["add", "two"]);
    fs::write(repo.join("two"), "unstaged").unwrap();
    action(
        snapshot.checkout.clone(),
        &root,
        HistoryAction::Squash("combined".into()),
    )
    .apply(repo)
    .unwrap();
    assert_eq!(fixture_git(repo, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(fixture_git(repo, &["log", "-1", "--format=%s"]), "combined");
    assert_eq!(fixture_git(repo, &["diff", &last, "HEAD"]), "");
    assert_eq!(fixture_git(repo, &["show", ":two"]), "staged");
    assert_eq!(fs::read_to_string(repo.join("two")).unwrap(), "unstaged");
    assert!(
        action(
            snapshot.checkout,
            &root,
            HistoryAction::Reset(ResetMode::Hard)
        )
        .apply(repo)
        .is_err()
    );
    let reset = |mode, target: &str| {
        action(
            git::snapshot(repo).unwrap().checkout,
            target,
            HistoryAction::Reset(mode),
        )
        .apply(repo)
        .unwrap()
    };
    reset(ResetMode::Soft, &root);
    assert_eq!(fixture_git(repo, &["show", ":two"]), "staged");
    reset(ResetMode::Mixed, &last);
    assert_eq!(fixture_git(repo, &["show", ":two"]), "two");
    assert_eq!(fs::read_to_string(repo.join("two")).unwrap(), "unstaged");
    reset(ResetMode::Hard, &last);
    assert_eq!(fs::read_to_string(repo.join("two")).unwrap(), "two");
    action(
        git::snapshot(repo).unwrap().checkout,
        &last,
        HistoryAction::Revert { mainline: None },
    )
    .apply(repo)
    .unwrap();
    assert!(!repo.join("three").exists());
    assert_eq!(fixture_git(repo, &["rev-parse", "HEAD^"]), last);
}

#[test]
fn branch_preview_and_conflict_recovery_through_the_ui() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init(repo);
    fs::write(repo.join("file"), "base").unwrap();
    fixture_git(repo, &["add", "."]);
    fixture_git(repo, &["commit", "-m", "base"]);
    fixture_git(repo, &["switch", "-c", "topic"]);
    fs::write(repo.join("file"), "topic").unwrap();
    fixture_git(repo, &["commit", "-am", "topic"]);
    fixture_git(repo, &["switch", "main"]);
    fs::write(repo.join("file"), "main").unwrap();
    fixture_git(repo, &["commit", "-am", "main"]);
    let main = fixture_git(repo, &["rev-parse", "HEAD"]);
    let mut rt = Runtime::new(
        Desk::new(Some(repo.into())),
        view::view,
        MIN_WINDOW_SIZE,
        Env {
            system_dark: false,
            scale: 1.0,
        },
    );
    rt.poll = Some(|s, _, _| s.poll());
    settle(&mut rt);
    click(&mut rt, "tab-Branches");
    click(&mut rt, "branch-actions-refs/heads/topic");
    click(&mut rt, "branch-menu-refs/heads/topic-item1");
    let plan = rt.state.integration.as_ref().unwrap().clone();
    assert_eq!((plan.ours, plan.theirs), (1, 1));
    click(&mut rt, "confirm-integration");
    assert!(rt.state.tab == Tab::Changes);
    assert_eq!(
        rt.state.workspace.snapshot.as_ref().unwrap().operation,
        Some(git::Operation::Merge)
    );
    click(&mut rt, "Abort");
    click(&mut rt, "confirm-recovery");
    assert_eq!(fixture_git(repo, &["rev-parse", "HEAD"]), main);
    assert_eq!(fs::read_to_string(repo.join("file")).unwrap(), "main");
    for recovery in [git::Recovery::Abort, git::Recovery::Skip] {
        let rebase = git::Integration::preview(
            repo,
            "refs/heads/topic".into(),
            git::IntegrationMode::RebaseMerges,
        )
        .unwrap();
        assert!(Action::Integrate(rebase).apply(repo).is_err());
        assert_eq!(
            git::snapshot(repo).unwrap().operation,
            Some(git::Operation::Rebase)
        );
        Action::Recover(git::Operation::Rebase, recovery)
            .apply(repo)
            .unwrap();
        assert_eq!(git::snapshot(repo).unwrap().operation, None);
    }
    fixture_git(repo, &["reset", "--hard", &main]);
    assert!(Action::Integrate(plan).apply(repo).is_err());
    fs::write(repo.join("file"), "resolved").unwrap();
    click(&mut rt, "refresh");
    click(&mut rt, "Stage-file");
    click(&mut rt, "continue-operation");
    assert_eq!(
        rt.state.workspace.snapshot.as_ref().unwrap().operation,
        None
    );
    assert_eq!(
        fixture_git(repo, &["rev-parse", "HEAD^2"]),
        fixture_git(repo, &["rev-parse", "topic"])
    );
}

#[test]
fn publishes_only_the_reviewed_tag_and_opens_its_history_actions() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("local");
    let remote = tmp.path().join("remote.git");
    init(&repo);
    fixture_git(tmp.path(), &["init", "--bare", remote.to_str().unwrap()]);
    fixture_git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    fixture_git(&repo, &["commit", "--allow-empty", "-m", "release"]);
    fixture_git(&repo, &["tag", "-a", "other", "-m", "another release"]);
    fixture_git(&repo, &["config", "push.followTags", "true"]);
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
    click(&mut rt, "tab-Tags");
    click(&mut rt, "tag-name");
    rt.paste("release");
    click(&mut rt, "tag-message");
    rt.paste("Ready");
    click(&mut rt, "create-tag");
    click(&mut rt, "tag-actions-release");
    click(&mut rt, "tag-menu-release-item1");
    click(&mut rt, "confirm-push-tag");
    assert_eq!(fixture_git(&remote, &["tag", "--list"]), "release");
    assert_eq!(fixture_git(&remote, &["cat-file", "-t", "release"]), "tag");
    click(&mut rt, "tag-actions-release");
    click(&mut rt, "tag-menu-release-item0");
    assert!(rt.state.tab == Tab::Diff(DiffMode::Commit));
    click(&mut rt, "commit-actions");
    click(&mut rt, "history-actions-item2");
    assert!(rt.state.history_form.is_some());
    click(&mut rt, "confirm-history-action");
    assert!(rt.state.history_form.is_none());
}

#[test]
fn linked_checkout_rebase_continues_independently_and_rejects_a_moved_source() {
    use git::{Integration, IntegrationMode, Operation, Recovery, WorktreeMode};
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    init(&repo);
    fs::write(repo.join("f"), "base").unwrap();
    fixture_git(&repo, &["add", "f"]);
    fixture_git(&repo, &["commit", "-m", "base"]);
    Action::AddWorktree {
        path: linked.clone(),
        mode: WorktreeMode::NewBranch,
        branch: "topic".into(),
        start: "".into(),
    }
    .apply(&repo)
    .unwrap();
    fs::write(linked.join("f"), "topic").unwrap();
    fixture_git(&linked, &["commit", "-am", "topic"]);
    let preview = || {
        Integration::preview(&linked, "refs/heads/main".into(), IntegrationMode::Rebase).unwrap()
    };
    let old = preview();
    fs::write(repo.join("f"), "main").unwrap();
    fixture_git(&repo, &["commit", "-am", "main"]);
    assert!(
        Action::Integrate(old)
            .apply(&linked)
            .unwrap_err()
            .contains("source branch moved")
    );
    assert!(Action::Integrate(preview()).apply(&linked).is_err());
    assert_eq!(git::snapshot(&repo).unwrap().operation, None);
    assert_eq!(
        git::snapshot(&linked).unwrap().operation,
        Some(Operation::Rebase)
    );
    fs::write(linked.join("f"), "resolved").unwrap();
    Action::Stage(vec!["f".into()]).apply(&linked).unwrap();
    Action::Recover(Operation::Rebase, Recovery::Continue)
        .apply(&linked)
        .unwrap();
    assert_eq!(
        fixture_git(&linked, &["symbolic-ref", "--short", "HEAD"]),
        "topic"
    );
    assert_eq!(
        fixture_git(&linked, &["rev-parse", "HEAD^"]),
        fixture_git(&repo, &["rev-parse", "HEAD"])
    );
    assert_eq!(fs::read_to_string(repo.join("f")).unwrap(), "main");
}
