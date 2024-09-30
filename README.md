# checklist

Yet another todo TUI app, meant to be used regardless of terminal size.

## Installation

At the moment cloning from this GitHub and installing with cargo is the best way to install this.

```sh
git clone git@github.com:dylanfair/checklist.git
cargo install --path ./checklist
```

In the future I can learn how to get this installed elsewhere. :)

## Why checklist?

What I wanted for myself was a todo TUI that I could use in a constrained terminal space, like if I just had a slim bar horizontally or vertically. Something I could have off to the side without needing to completely full-screen in order to log some tasks. Now `checklist` can still be used as a full-screen app, but it wasn't my primary goal. Below are some pictures of how `checklist` will conform depending on the terminal size it's been given. 

![horizontal checklist](./images/horizontal-example.png)

![vertical checklist](./images/vertical-example.png)

![checklist in the corner](./images/top-right-example.png)

There's a long way to go and likely some subjectivity on how to make this more effective, but I hope this gives you a general idea of what I'm aiming for here. `checklist` will automatically shift between a Horizontal or Vertical view based on terminal area conditions, however you can also explicitly choose to be in Horizontal or Vertical view if you want.

## So what else?

Besides that, `checklist` will have your typical todo app features:

* Adding a task
* Updating a task
* Deleting a task

A task can have attributes such as:

* Urgency (Low, Medium, High, Critical)
* Status (Open, Working, Paused, Completed)
* Tags (which can be filtered for)
* And space to write out a description or maybe the latest update

Speaking of filters, as of now (September 2024) the only other filter is by Status (Completed, NotCompleted, and All), and you can sort Urgency in an ascending or descending manner (Critical > High > Medium > Low).

The keybindings take inspiration from vim motions, such as `j` and `k` for moving up and down the task list. A full listing can be found when hitting `h` in the app.

![Keybindings as of writing the README (subject to have changed)](./images/key-bindings.png)

## Getting started

Once you have `checklist` installed, you can get started with:

```sh
checklist
```

Easy enough! On first time use this will create a sqlite database and configuration file in your local configuration directory - likely one of the following places:

Linux: /home/<USER>/.config/checklist/
Windows: C:\Users\<USER>\AppData\Local\checklist\
Mac*: ~/Library/Application Support/checklist/

*I don't have a Mac so haven't tested this, but I believe that's where it will go 

You can also always check where the database that `checklist` is using at with:

```sh
checklist where
```

There are only a couple other commands:

```sh
checklist wipe
```

This will wipe out all tasks in your database should you accept the confirmation prompt -- use with caution.


`checklist display` will open up the TUI just like `checklist` by itself would, but it does also allow you to preemptively set the layout view you want to use with the `-v` flag, like so:

```sh
checklist display -v horizontal
```

