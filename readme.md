# Minit

> **What can you do right this Minit?**

Minit is a lightweight desktop to-do list built with **Rust** and **egui** that helps you decide what to work on based on how much free time you have.

<p align="center">
    ![Screenshot of the Minit App showing a list of chores to do.](/assets/TimeForLaundry.png)
</p>

## Features

- **Time-Based Suggestions**: Get suggestions for what to work on based on the amount of time you have available.
- **Priorities**: Organize tasks by **Low**, **Medium**, or **High** priority.
- **Minimal & Distraction-Free**: Native desktop performance with a simple, focused interface and zero bloat.
- **Automatic Saving**: Tasks and preferences are automatically saved between sessions.
- **Inline Editing**: Double-click any task title in the task-list to edit it directly.

## Quickstart

### Prerequisites

Make sure you have the [Rust](https://www.rust-lang.org/) toolchain installed, the recommended method is using [rustup.rs](https://rustup.rs/), the official Rust toolchain installer and version manager.

### Running Locally

Clone the repository and run the application:

```bash
git clone https://github.com/juniperorjuno/minit.git
cd minit
cargo run --release
```

## Tech Stack

| Technology | Purpose            |
|------------|--------------------|
| Rust       | Core application   |
| eframe	   | Native desktop GUI | 
| serde      | Serialization and persistence|
| chrono     | Time management    |

## License

Distributed under the MIT License. See [LICENSE](/LICENSE) for details.
