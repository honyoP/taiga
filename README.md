![taiga_banner](https://github.com/user-attachments/assets/5ff181bd-05c2-454a-a2d0-1c3e91eeb19c)


# Taiga

> **CodeName: YATTA (Yet Another Terminal Task App)**

A task organizer for the "mentally deficit monkey" in all of us. 🐒

**Taiga** is a CLI task manager built in Rust. It does one thing, and it does it fast: it manages your tasks without forcing you to leave the terminal or wait for a heavy web app to load.

If you like **Vim**, **Markdown**, and **not using a mouse**, you're home.

---

## 🤷 Why?

I got tired of opening a browser tab just to write down "Buy Milk."

I wanted a tool that:

1. **Starts instantly.** (No Electron bloat).
2. **Stores data in plain text.** (I want to own my data, not lock it in a database).
3. **Doesn't judge me** for having 50 overdue tasks.

## ✨ Features

* **⚡ Blazingly Fast:** It's Rust. It finishes executing before your finger leaves the Enter key.
* **📄 Plain Text Storage:** Tasks are saved in a `.md` file. You can `cat` it, `grep` it, or edit it manually if you're brave.
* **🧠 Human Scheduling:** Understands "tomorrow", "next friday", and "2024-01-01".
* **🛡️ ID-Based:** Every task gets a unique ID. No ambiguities.
* **🦾 Regex Powered:** Uses a custom regex parser to read your markdown file, because XML parsers are for cowards.
* **🔌 Plugin System:** Extend Taiga with plugins. Because why stop at task management when you can have world domination?
* **🍅 Pomodoro Timer:** Built-in focus timer with audio cues and break windows. Procrastination just got harder.
* **🖥️ Terminal UI:** A full TUI mode for those who find typing commands too mainstream.

---

## 📦 Installation

### Option 1: Pre-compiled Binaries (Easiest)

Go to the [Releases Page](https://github.com/honyoP/taiga/releases) and grab the binary for your OS.

**Linux / Mac:**

```bash
chmod +x taiga
mv taiga /usr/local/bin/

```

### Option 2: Build from Source (For the cool kids)

You need [Rust](https://www.rust-lang.org/) installed.

```bash
git clone https://github.com/YOUR_USERNAME/taiga.git
cd taiga
cargo install --path .

```

---

## 🎮 Usage

Taiga uses a natural subcommand structure.

### 1. Add a Task

Just type.

```bash
taiga add "Fix the production bug"

```

**With a Schedule:**
Use the `when` keyword to attach a date.

```bash
taiga add "Buy groceries" when "tomorrow"
taiga add "Submit report" when "next friday"

```

### 2. List Tasks

See what you've been putting off.

```bash
taiga list           # Show all tasks
taiga list open      # Show only incomplete tasks
taiga list done      # Show completed tasks

```

*Output:*

```text
[ID:1] - [ ] Fix the production bug
[ID:2] - [ ] Buy groceries (Scheduled: 2024-03-20)

```

### 3. Get Stuff Done

Mark a task as complete using its **ID**.

```bash
taiga check 2

```

### 4. Nuke It

Delete a task forever.

```bash
taiga remove 1

```

### 5. Edit Tasks

Made a typo? Change the name or date.

```bash
taiga edit 1 --name "Actually fix the bug this time"
taiga edit 1 --date "next monday"

```

### 6. Housekeeping

```bash
taiga clear --checked   # Remove all completed tasks
taiga reindex           # Renumber task IDs sequentially
taiga recover           # Restore from backup (we all make mistakes)

```

---

## 🔌 Plugins

Taiga has a plugin system. Yes, really. A CLI task manager with plugins. We've come full circle.

### 🍅 Pomodoro Timer

Focus like a caffeinated squirrel.

```bash
taiga pomo start 25 5 4          # 25 min focus, 5 min break, 4 cycles
taiga pomo status                # Check timer status
taiga pomo pause                 # Take an unscheduled break
taiga pomo resume                # Back to work
taiga pomo stop                  # Give up (we don't judge)

```

**Options:**
- `--no-gui` — Skip the break window popup (for the minimalists)
- `--no-sound` — Disable audio cues (for the library dwellers)

The pomodoro plugin runs as a daemon in the background, so you can close your terminal and it'll keep ticking. It even plays sounds when breaks start and end. You're welcome.

### 🖥️ Terminal UI

For when you want to feel like a hacker managing your grocery list.

```bash
taiga tui run

```

Navigate with arrow keys, filter tasks, add new ones—all without leaving the terminal. It's like Vim, but for tasks. And less painful to exit.

### Listing Plugins

See what plugins you've got loaded:

```bash
taiga plugins

```

---

## ⚙️ Under the Hood

Taiga stores your tasks in a simple Markdown file located in your system's default data directory (managed by `confy`).

* **Linux:** `~/.config/taiginator/taiga.md` (or similar)
* **Mac:** `~/Library/Application Support/rs.taiginator/taiga.md`
* **Windows:** `%APPDATA%\taiginator\taiga.md`

Because it's just a file, you can back it up with Git, sync it via Dropbox, or print it out and eat it.

### Plugin Architecture

Plugins live in `~/.config/taiga/plugins/` as dynamic libraries (`.so` on Linux, `.dylib` on Mac, `.dll` on Windows). The plugin API supports:

- **Commands**: Add new subcommands to the CLI
- **Daemon mode**: Long-running background processes with IPC
- **Lifecycle hooks**: Run code on plugin load/unload

Want to write your own plugin? Check out `taiga-plugin-api` crate and the existing plugins in `plugins/` for examples. Go wild.

## 🛠 Building & Contributing

Found a bug? Want to add a feature?
PRs are welcome. Just please run `cargo fmt` before you push or the CI will yell at you.

```bash
# Run locally
cargo run -- add "Test task"

# Run tests
cargo test

```

## 📜 License

MIT. Do whatever you want with it. Just don't blame me if you miss your dentist appointment.
