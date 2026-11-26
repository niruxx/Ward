# Ward

A native Rust / GTK4 / libadwaita firewall manager for Linux. Ward provides an unprivileged desktop interface, UFW integration through a narrowly scoped PolicyKit helper, and firewalld integration through its system D-Bus API.

**Development release, not yet ready for Flathub or production firewall administration.** The application has real backend operations, not simulated success. Use the explicit mock mode for interface development. Validate real firewall changes in a disposable VM with console access before deployment.

## Build and run

Requires current stable Rust, GTK **4.12+**, libadwaita **1.5+**, GLib/GIO development libraries, pkg-config, and a C compiler. The bindings are GTK Rust 0.11, libadwaita Rust 0.9, and GLib/GIO Rust 0.22; Cargo.lock pins the resolved versions.

On Ubuntu 24.04, install `build-essential pkg-config libgtk-4-dev libadwaita-1-dev libglib2.0-dev` and Rust. On Fedora, install `gcc pkgconf-pkg-config gtk4-devel libadwaita-devel glib2-devel` and Rust. Install UFW or firewalld separately; Ward never installs or switches firewall services.

```sh
cargo build --locked
./scripts/run-dev.sh
```

For a mock dashboard and rule lifecycle, without reading or modifying the host firewall:

```sh
WARD_MOCK=1 GSETTINGS_BACKEND=memory ./scripts/run-dev.sh
```

The GUI refuses to run as root. The schema is optional at startup so a missing installation produces a visible explanation rather than a crash. The development script compiles and locates it. Preferences and onboarding persist through GSettings in ordinary operation. `GSETTINGS_BACKEND=memory` keeps test preferences temporary.

UFW status requires administrator privileges on supported distributions. Install the helper and policy before refreshing UFW. firewalld uses its own existing PolicyKit policy; the desktop session must provide an authentication agent. Operations run on worker threads and leave GTK responsive while authorization is pending.

## Native installation

```sh
cargo build --release --locked
sudo ./scripts/install.sh
```

Or use Meson:

```sh
meson setup build --prefix=/usr
meson compile -C build
sudo meson install -C build
```

For a distribution package, `DESTDIR=/path/to/staging ./scripts/install.sh` stages the release binaries and integration files; packaging triggers must compile schemas/update desktop and icon caches. Both installation methods use `/usr/libexec/ward-helper`, which must match the PolicyKit executable annotation. Never mark the helper setuid. Installed binaries, policy, and parent directories must be root-owned and not writable by ordinary users. Neither installation method enables a firewall or changes its rules.

## Implemented

- Four-page introduction, backend discovery and explicit selection, GSettings completion, and a revisit action.
- A redesigned home page with a plain-language status card, prominent Add Rule and Allow Port actions, persistent compact-window creation buttons, and explanations of incoming/outgoing policy. Port exceptions use the same validated backend requests as other rules.
- Native primary menu and shortcuts, scrollable clamped dashboard, adaptive search/sort controls, textual status, confirmed refresh time, loading spinner, toasts, keyboard-accessible standard widgets, system/light/dark appearance, and native page transitions. Desktop animation settings remain authoritative.
- Rule search, sorting by description/protocol/port, add/review/delete dialogs, destructive confirmations, IPv4/IPv6 network validation, TCP/UDP and port ranges, and source/destination restrictions for UFW.
- UFW status and numbered rules; enable/disable; incoming/outgoing allow/deny/reject rules; TCP rate limits; deletion; default policies; logging levels; installed application profile listing and incoming allow rules. UFW manages dual-stack expansion according to its existing IPV6 setting, shown in the overview.
- firewalld status and all runtime zones, port/service/rich-rule inspection and removal; incoming port and service addition to the default zone; bounded custom allow/block/reject port rules with IPv4/IPv6 source and destination restrictions through the rich-rule API; source/interface membership changes in a specified zone; runtime-to-permanent save; and reload with a runtime-loss warning. The firewalld toggle is unavailable because stopping a daemon is not equivalent to editing firewall policy.
- Recent journal entries with refresh/copy and loading/empty/error states, a selectable diagnostic report with clipboard/file export, settings, application icon, desktop entry, schema, installation scripts, and an About dialog.
- Mock backend and backend/model tests that do not require root or a running firewall.

The UI refinement review, design references, screenshots, and detailed verification are in [docs/ui-review.md](docs/ui-review.md).

## Distribution packages

Native x86-64 package recipes are in `packaging/`. Release builds target Debian 13 (`.deb`), Fedora 45 (`.rpm`), and current Arch (`.pkg.tar.zst`). The Debian and Fedora binaries are built in their own distribution environments. Packages are unsigned local development builds.

`python3 packaging/source-dist.py` creates a vendored source archive in `dist/`, its SHA-256 file, and an Arch `PKGBUILD` with the actual source checksum. Rust 1.92+ is required by the pinned dependencies; vendored source builds run Cargo offline. Build from the extracted source archive so vendored crates and generated third-party notices are present. Use `packaging/debian/build.sh` on Debian, `rpmbuild -ba packaging/fedora/ward.spec` with the source archive in RPM's SOURCES directory on Fedora, or `makepkg` beside the rendered PKGBUILD and source archive on Arch. Distribution GTK development packages and standard package build tools are also required.

Every package installs the same scoped helper and PolicyKit policy. Backends are optional (`Suggests`/`optdepends`), and no package script starts services or edits firewall configuration. Debian install/remove scripts update desktop/GSettings/icon caches; Fedora and Arch use their standard package-manager file triggers/hooks. No package contains a precompiled system-wide GSettings cache.

## Architecture and security

`src/model.rs` defines typed, bounded rule semantics and the serialized helper request enum. `src/backend/mod.rs` provides common discovery and request dispatch, while `ufw.rs` and `firewalld.rs` retain backend-specific operations. `src/bin/ward-helper.rs` handles only typed UFW operations. `src/main.rs` initializes the unprivileged application; `src/ui.rs` builds the dashboard and coordinates asynchronous operations, notifications, and error states. `src/ui/onboarding.rs`, `src/ui/rules.rs`, `src/ui/settings.rs`, and `src/ui/logs.rs` contain the corresponding native interfaces. `src/ui/home.rs` contains the home-page action layout and policy explanations. `build.rs` embeds the application icon as a GResource for reliable development and installed builds.

The UFW helper accepts a single JSON request of at most 64 KiB, rejects unknown operations and fields, validates rule and policy arguments, and invokes a fixed UFW executable with structured arguments and a clean environment. It never invokes a shell. Profile names must match the installed profile list. PolicyKit requires administrator authentication for active sessions and denies inactive/remote sessions. Authorization is deliberately not retained by Ward’s policy. Read and write operations share this boundary because UFW itself restricts status access.

A root-owned directory lock serializes Ward helper requests. If a helper is killed before cleanup, an administrator must verify no helper is running before removing `/run/ward-helper.lock`. The directory is temporary across reboot. UFW also manages its own internal locking. A configuration revision is compared before each write to reject stale operations. **External clients can still race between that comparison and a backend mutation**; neither API provides an atomic compare-and-swap transaction. Never assume a stale revision check is a transaction.

Ward blocks UFW mutations when firewalld owns its D-Bus name. firewalld mutations are blocked when an installed UFW has `ENABLED=yes`, or its enabled configuration cannot be read. This is a conservative conflict check, not a universal detector of every netfilter manager. nftables scripts, container networking, and other tools can also affect effective policy.

Every UI mutation requires review/confirmation. Backend confirmation precedes success feedback, and a failed operation invalidates the snapshot until refresh. A mutation may have succeeded even if its subsequent read failed. Remote-access warnings require a recovery path; **automatic SSH detection and timed rollback are not implemented**. Ward does not guarantee protection against lockouts. Existing rule text is preserved, never reconstructed into a supposedly equivalent editable rule.

## Scope and remaining work

This first implementation does **not** claim the entire publication-quality specification is finished:

- Arbitrary existing-rule editing and per-rule enable/disable are unavailable. UFW does not expose a universal enable flag; complex rules cannot safely be round-tripped. Replace rules deliberately using add and delete.
- UFW reports no numbered rules while inactive; the overview therefore cannot count its stored inactive configuration. Ward does not edit `/etc/default/ufw` or globally toggle IPv6.
- firewalld configuration shown in the dashboard is **runtime**. The save action commits the whole runtime configuration, including other applications’ changes. There is no separate permanent editor/diff, arbitrary rich-language editor, policy-object editor, zone-target editor, or service definition editor. Rule count covers displayed ports/services/rich rules, not the complete effective kernel policy.
- Completed firewalld setups read status when opened. Subsequent refreshes are manual; UFW status reads remain explicit because they need administrator authorization. There is no periodic polling, launch-on-login control, or rule-default preference. Security confirmations cannot be disabled.
- Logs are limited to the most recent 200 visible journal records. UFW logging can be disabled and journal access restricted; firewalld’s service journal is not necessarily a packet log.
- Diagnostic reports can be copied or saved with the native file chooser. No invented developer identity, repository URL, or release metadata is included; a canonical upstream and release ownership must be established before publication.
- Native packaging is implemented. **Flatpak distribution needs additional host integration**: an ordinary sandbox cannot execute the installed UFW helper through pkexec. A separately installed, audited host D-Bus service with sender-based PolicyKit checks is required for UFW. firewalld also needs a narrowly scoped system-bus talk permission. Do not grant unrestricted host access or distribute the current native helper inside a Flatpak and claim UFW works. No misleading Flatpak manifest is provided.
- Native light/dark/high-contrast and 360-pixel layouts have graphical regression coverage. Screen-reader usability, translations, file-chooser/portal behavior across desktops, and real authorization still need manual release review. No custom CSS overrides GNOME styling.

## Testing

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
glib-compile-schemas --strict --dry-run data
# Backend/helper only, without GTK development packages:
cargo test --no-default-features --locked
```

Mock UI exercise: launch with `WARD_MOCK=1 GSETTINGS_BACKEND=memory`, complete onboarding, enable the simulated firewall, add a TCP 443 rule, search/sort, cancel then confirm deletion, switch theme, and revisit onboarding. No mock call invokes either host backend. Backend-only tests cover command-injection-like input, port boundaries, invalid CIDRs, invalid semantics, unknown privileged requests, numbered IPv4/IPv6 parsing, and the mock lifecycle.

The two graphical workflows are explicit and excluded from headless runs. They cover the mock rule lifecycle and backend-specific forms; real-backend confirmations are always canceled:

```sh
glib-compile-schemas --strict data
GSETTINGS_SCHEMA_DIR="$PWD/data" GSETTINGS_BACKEND=memory cargo test -- --ignored
```

Verified in this workspace: a native debug build, a Meson release build and staged installation, strict Clippy, backend/security and policy-explanation tests, two graphical workflows in a Wayland session (also run in high contrast), schema/XML/desktop validation, and shell syntax. GTK 4.20.3 and libadwaita 1.8.2 were built under `/tmp` for validation because system development packages were absent. No host firewall was changed. Real firewall/PolicyKit integration remains unverified here.

Real integration must run in **separate disposable UFW and firewalld VMs** with console access. Record the original configuration and test canceled authorization, enable/disable (UFW), IPv4/IPv6 rules, invalid input, deletion, external configuration changes, logging restrictions, runtime save/reload, service/interface/source changes, unavailable services, and simultaneous-backend conflict detection. Verify both backend state and network behavior from a second VM. No tests automatically mutate the developer’s firewall.

References: [gtk-rs](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/), [libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/), [firewalld D-Bus API](https://firewalld.org/documentation/man-pages/firewalld.dbus.html).

Licensed under GPL-3.0-or-later; see LICENSE.
