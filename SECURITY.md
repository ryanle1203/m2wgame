# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Use GitHub's private vulnerability
reporting ("Security" tab, "Report a vulnerability") on this repository. We aim to answer within
a week.

## What M2WGame handles that is sensitive

- **Steam passwords and Steam Guard codes** are typed in the app and written directly to the
  SteamCMD process. They must never be stored, logged or sent anywhere else. Any change that
  could leak them is a security bug.
- **Steam's debugging port.** To show Steam's hidden dialogs, M2WGame starts Steam with a
  Chromium debugging port on `127.0.0.1` and a random port number, only when the user asks, and only
  for that run. Any local program could talk to that port while it is open; the app keeps the
  window of exposure short but cannot authenticate it.
- **Deleting files.** Removing a game deletes only inside the game's own bottle folder and is
  covered by tests; a path-escape bug here is a security bug.

## Supported versions

Only the latest release / `main`.
