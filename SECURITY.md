# Security

The Xbox Series Dashboard is an early alpha made by one person. Only the newest release is looked after.

## Reporting a problem

Please report a security problem privately, not in a public issue: on the repository's **Security** tab, choose
**Report a vulnerability** (GitHub's private vulnerability reporting). Say what is affected, how to reproduce it, and
which version (Settings > System > About, or `dashboard.exe --version`).

You will get an answer as soon as it has been read. A fix goes into the next release, and the report is published
with it once that release is out.

## What is in scope

- `dashboard.exe` and what it does on the machine: shell mode (`Winlogon\Shell`, the sentinel), the installer and
  uninstaller, the updater, and how it starts games, apps and the stores' own programs.
- How it handles what it reads: the stores' files, the Discord web session it keeps in its data folder, and anything
  it downloads.

Problems in the stores, Discord, Windows or a dependency are for their owners; tell them, and tell us if the
Dashboard makes one worse.
