# Git workspace

## Run

With Git and the stable Rust toolchain installed, run this from the source directory.

```powershell
cargo run --locked
```

Pass a repository path to open it at launch, for example `cargo run --locked -- "C:\Projects\my repo"`. **Open…** switches to another repository or linked worktree.

## Release

Push a version tag from a commit containing the release workflow.

```powershell
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

GitHub Actions tests and builds Windows x64, macOS Apple Silicon, and Linux x64 binaries. Once all three builds pass, it publishes the archives and `SHA256SUMS` to GitHub Releases. Tags like `v0.2.0-rc.1` publish as prereleases.

## Keyboard shortcuts

Use Ctrl on Windows and Linux, or Command on macOS.

| Shortcut | Action |
| --- | --- |
| Ctrl/Command + O | Open a repository |
| Esc | Cancel the repository picker when a repository is open |
| Ctrl/Command + 1 through 7 | Changes, Diff, History, Branches, Stashes, Settings, GitHub |
| Ctrl/Command + 8 or 9 | Tags or Worktrees |
| F5 | Refresh the current view |

## Working with a repository

Use **Changes** to stage or unstage individual files. To work on multiple files, use the checkboxes beside their names or the checkbox in a section header, then choose **Stage selected** or **Unstage selected**. Enter a commit message and choose **Commit staged** to commit the index. The app keeps a separate message draft for each repository until you close the app.

The **More** menu beside the commit button lets you add a signed-off-by trailer, sign with your configured Git signing key, amend the last commit, or allow a commit with no file changes. When amending, leave the message blank to reuse the previous message. Options apply to the next successful commit.

In **Stashes**, save staged and unstaged changes with an optional message. Check **Include untracked files** to save new files too. **Apply** restores the changes and keeps the stash. **Pop** removes the stash after a successful restore. **Restore staged state** is on by default.

**Diff** shows tracked changes in the working tree or index. **History** graphs up to 40 commits reachable from local branches, remote-tracking branches, and the current HEAD. Select a commit to open its patch in **Diff**. Merge commits are compared with their first parent.

### Commit actions

Choose **Actions** beside the selected commit’s title to tag it or change the current checkout.

- **Revert commit** creates a new commit that reverses the selected change. For a merge, choose the mainline parent whose side of the merge you want to keep.
- **Reset to commit** moves HEAD to the selected commit. **Soft** preserves the index and working files. **Mixed** resets the index and preserves working files. **Hard** also replaces tracked files and can remove untracked files that obstruct checkout.
- **Squash commits** combines the selected commit and every commit through HEAD into one commit with your message. Choose the oldest commit in a linear range of at least two commits. GitDesk reuses the tree at HEAD, preserving staged and unstaged edits. The original tip is available in the reflog.

### Branches and conflicts

In **Branches**, use a local branch’s **Actions** menu to switch, branch from it, rename it, or delete it. Git checks for unmerged changes before deletion. Choose a local or remote-tracking branch to preview a **Merge** or **Rebase**. The preview shows the direction and how many commits are unique to each side. For a merge, choose Git’s configured behavior, fast-forward only, or a merge commit. For a rebase, choose linear replay or preserve merges.

When an operation stops, **Changes** shows the conflicted paths. Edit the files in your editor, stage each resolution, and choose **Continue**. **Skip commit** is available during rebase and revert. **Abort** asks for confirmation before abandoning the operation. Reopening a worktree restores its in-progress controls.

### Tags and worktrees

In **Tags**, enter a name and an optional target branch, tag, or commit. The target defaults to HEAD. Add an annotation to create an annotated tag, or leave it blank for a lightweight tag. A tag’s **Actions** menu opens its target commit, pushes that single tag to a configured remote, or deletes it locally.

In **Worktrees**, enter a destination folder and choose a new branch, an available local branch, or a detached checkout. New branches and detached worktrees accept a starting revision. Relative paths start at the open repository. The list shows each checkout’s branch or detached HEAD, folder, and lock status. **Open** switches GitDesk to that worktree. **Lock** protects a linked worktree from removal and pruning. **Remove** requires confirmation and Git checks the worktree for changes.

### Repository tools

In **Diff → Compare branches**, choose a base and a comparison branch to view the changes from the base tip to the comparison tip. The pickers include local and remote-tracking branches. Use **Fetch** to update remote-tracking branches.

**Fetch**, **Pull**, and **Push** use the repository’s Git configuration and credentials. Pull uses fast-forward only. Git reports errors in the window. Bare repositories support history, branch inspection, fetch, and push.

**Settings** reads and edits Git configuration at local, global, worktree, and system scope, subject to Git’s permissions and repository configuration.

**GitHub** shows open pull requests, open issues, and recent Actions runs for the repository. Choose **View details** on a pull request or issue to read its description and comments or post a comment. On a pull request, **Files changed** shows the diff with line numbers. **Open on GitHub** opens the original item in your browser. Install GitHub CLI on PATH and sign in with `gh auth login` to use these views.

Use **Open in** next to Refresh to open the repository in your file manager, a system terminal, or Visual Studio Code.
