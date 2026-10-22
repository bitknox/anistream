# Security

## Reporting a vulnerability

Report a vulnerability privately, through GitHub's private vulnerability reporting: open the
repository's [Security tab](https://github.com/bitknox/anistream/security) and choose **Report a
vulnerability**, or go straight to the
[new advisory form](https://github.com/bitknox/anistream/security/advisories/new). Only you and
the maintainers see the report.

Please don't open a public issue, pull request or discussion about a vulnerability before its fix
is released.

A useful report says what is affected (the app and platform, the crate, or the store and how it is
deployed), the version or commit, the steps that reproduce it, and what an attacker gains. A
proof of concept helps, as long as it doesn't touch other people's machines, accounts or stores.

You'll get a reply in the advisory, and the discussion and the fix are worked out there.

## In scope

- The apps: the terminal app, the Mac, iPhone and iPad app, and the Android app.
- The engine: the Rust crates under `crates/` that the apps share, including the local database,
  account sign-in and token storage, tracker sync and playback.
- The plugin sandbox: anything that lets a plugin reach a host its manifest doesn't declare, read
  files, settings or other plugins' data, outlive its limits, or affect the host process. The
  guarantees are listed under [The sandbox](docs/plugins.md#the-sandbox).
- The plugin store: the server, the serverless templates under `deploy/`, the admin page, and the
  protocol in [docs/plugin-stores.md](docs/plugin-stores.md), including invites, device tokens,
  sign-in links and the signed plugin index. A way around what
  [What a store protects](docs/plugin-stores.md#what-a-store-protects) says a store protects is in
  scope. The limits that section states, such as a member being able to read the plugins they
  downloaded, are not vulnerabilities.
- Releases: the workflows that build and publish them, the Android release signing, and the
  SHA-256 checksums published beside every file.

## Out of scope

- Third-party sources: the sites and services that plugins, indexers or trackers reach. anistream
  doesn't run them and can't fix them.
- Plugins people install, whether from a file or from someone's store. A bug in a plugin is for
  its author. A plugin that gets out of the sandbox is in scope, as above.
- A store's operator and its machine. Someone who controls the machine a store runs on controls
  that store.
- Vulnerabilities in a dependency such as mpv, FFmpeg or a Rust crate belong with that project.
  Report them here too if the way anistream uses the dependency exposes its users.

## How fixes ship

Every merge to main that changes more than the website or the docs builds and publishes a
release: the terminal builds, the apps and the store's container image. So a fix ships as soon as it is merged, and only the newest release gets fixes.
Update to it to get the fix.

A store you run yourself gets the fix when you update it: pull the new `anistream-store` image, or
update your copy of the deploy template.

Once the release is out, the advisory is published, with a CVE where one applies. Reporters are
credited in it unless they ask not to be.
