# Progress Notes

Read this first in a new session. It's the current, accurate state of the
project — what's built, why it's built that way, what's deliberately not
done yet, and what to do next.

## What this project is

Linux-native hardware monitoring/control/diagnostics platform for an ASUS
ROG Zephyrus G16, split Rust (daemon, control logic) + C++ (hardware layer,
NVML). Full scope is in `config/project-description.txt`. **System
Control** is the interface being built first: battery charge limit is done
end-to-end; platform profile is code-complete on both daemon and GUI sides
(builds clean, **not yet installed/tested live**); a new **Updates**
section (OS/drivers/packages, "if not updated causes issues" framing) is
underway -- the **OS** category (apt) is code-complete end-to-end (daemon +
GUI), same not-yet-installed-live caveat. Drivers (firmware, via
`fwupdmgr`) and Packages (`snap`/`flatpak`) haven't been started.

Ali (the user) is a 3rd-year student building this for resume/skills in
systems/embedded/DevOps. New to Rust, D-Bus, polkit, and packaging;
comfortable with hardware. Working style: **small increments, explain
everything, don't jump ahead** -- confirm before adding scope beyond what
was explicitly asked. Prefers short answers to "quick question"-labeled
messages, fuller explanations otherwise. He implemented platform profile's
GUI-side D-Bus proxy himself, following the battery pattern built for him
first. For the Updates feature he asked Claude to build both daemon and
GUI sides, since the GUI introduced two genuinely new patterns (two-step
preview/confirm, background-thread apply) rather than being a same-shape
copy of an existing panel.

## Repo layout (current, real)

```
Cargo.toml                              workspace root (2 members)
rust-daemon/                            the daemon crate (package name: controlcenterd)
  Cargo.toml
  src/error.rs                            ControlError enum -> zbus::fdo::Error mapping
  src/polkit.rs                           polkit CheckAuthorization call
  src/main.rs                             entry point: discover hw, serve on system bus
  src/system_control/                     one dir per project-description.txt section
    mod.rs                                  re-exports the section's public API
    battery.rs                              sysfs I/O: discover/read/validate/write+verify
    battery_iface.rs                        the org.controlcenter.Daemon1.Battery interface
    platform_profile.rs                     sysfs I/O for platform_profile: same shape as battery.rs
    platform_profile_iface.rs               the org.controlcenter.Daemon1.PlatformProfile interface
    updates.rs                              preview/apply logic for updates; apply_os triggers a
                                             separate systemd unit rather than running apt itself
    updates_iface.rs                        the org.controlcenter.Daemon1.Updates interface (os only so far)
controlcenter-gui/                      the GUI crate (package name: controlcenter-gui)
  Cargo.toml
  src/main.rs                             eframe App shell; composes section panels
  src/system_control/                     mirrors the daemon's section split
    mod.rs                                  State struct + show() composing the panels below
    battery.rs                              D-Bus proxy + Battery panel, wired to the daemon, works
    platform_profile.rs                     Platform Profile panel; D-Bus proxy + Apply button
                                             wired, same shape as battery.rs; compiles
    updates.rs                              Updates panel (OS category); preview + confirm dialog +
                                             background-thread apply; compiles
dbus/
  org.controlcenter.Daemon1.conf          bus-level access policy
  org.controlcenter.Daemon1.service       D-Bus activation file
polkit/
  org.controlcenter.daemon.policy         set-charge-limit + set-platform_profile + apply-os-update
                                           action definitions
systemd/
  controlcenterd.service                  the always-on daemon's unit (sandboxed, D-Bus-activated)
  controlcenterd-updates@.service         NEW: on-demand, unsandboxed, templated oneshot unit that
                                           actually runs an update script (see Key conventions below
                                           for why this is a separate unit from controlcenterd.service)
scripts/
  updates/update-os.sh                    NEW: apt-get update && apt-get upgrade -y, set -euo pipefail
config/project-description.txt         full original project scope (not written by Claude)
Design/                                 Ali's own Xournal++ sketches (e.g. the architecture
                                         diagram discussed this session), not code
cpp-hardware/, tests/                   empty scaffold folders, untouched so far
```

Git: HEAD is still `67f4c28` ("ChargingLimit: ..."); everything described
below is **uncommitted working-tree state**, not yet committed. There is
also an older `git stash` entry from before the `rust-daemon`/
`controlcenter-gui` rename (used `controlcenterd/`/`controlctl/` dirs) with
a `state.rs` (persistence) and rate-limiting the current code doesn't have.
Won't merge cleanly; treat as reference (`git stash show -p stash@{0}`),
not something to `pop` directly.

## What's actually implemented

**Daemon (`rust-daemon`)** -- three D-Bus interfaces at bus name
`org.controlcenter.Daemon1`, object path `/org/controlcenter/Daemon1`
(one object path hosting multiple interfaces via repeated `.serve_at()`
calls on the same `Builder`):

- **`...Battery`**: `Supported` (bool), `ChargeLimit` (u8, live sysfs read),
  `SetChargeLimit(u8) -> u8` -- polkit-gated, validate/write/read-back,
  emits `PropertiesChanged`. Fully working, confirmed live.
- **`...PlatformProfile`**: `Supported` (bool), `Profile` (`String`, live
  read of `/sys/firmware/acpi/platform_profile`), `SetProfile(String) ->
  String` -- same polkit-gate/validate/write/read-back/`PropertiesChanged`
  shape as Battery. Deliberately `String`, not a typed enum -- `validate`
  already re-checks against the real hardware file on every call, so an
  enum wasn't buying extra safety, just ceremony, and nothing outside the
  module consumes a typed value. Authorization: action
  `org.controlcenter.daemon.set-platform_profile` (passwordless for any
  active session -- low blast radius).
- **`...Updates`** (new): `PreviewOs() -> String`, `ApplyOs() -> ()`.
  - `PreviewOs` runs `apt list --upgradable` directly (no root, no polkit
    gate -- read-only, same tier as reading `ChargeLimit`).
  - `ApplyOs` is polkit-gated (`org.controlcenter.daemon.apply-os-update`)
    and, unlike every other write in this daemon, does **not** touch
    hardware/state itself. It starts
    `controlcenterd-updates@os.service` via `systemctl start --wait` and
    blocks until that unit finishes, then reads
    `/run/controlcenterd/update-os.log` back for failure detail (last 20
    lines) if the unit's exit status wasn't success. See "Key conventions"
    below for why applying an update needed a second systemd unit instead
    of just running `apt` from inside `controlcenterd` itself.
  - Only the `os` category exists so far. `drivers` (firmware, via
    `fwupdmgr`) and `packages` (`snap`/`flatpak`) are not started --
    `updates.rs`/`updates_iface.rs` are set up so each is a
    `preview_<category>`/`apply_<category>` pair following the exact same
    shape as `os`, plus one more templated-unit instance
    (`controlcenterd-updates@drivers.service` etc. -- the unit file is
    already generic, only a new script + daemon functions + polkit action
    are needed per category).

Authorization: `polkit.rs`'s `authorize()` is generic (connection, header,
action_id) and unchanged -- every interface just calls it with its own
action id. `polkit/org.controlcenter.daemon.policy` now has three actions.
Known inconsistency (not fixed, flagged only): `set-platform_profile` uses
an underscore where the other two action ids use hyphens throughout.

**Not in the daemon (deliberately, for now):** rate limiting, state
persistence/restore-on-restart, a CLI client (`controlctl`). These existed
in the stashed earlier pass and can be ported back in when asked.

**GUI (`controlcenter-gui`)** -- eframe/egui app, one window, three
sections so far, each its own module under `system_control/` with a
`State` struct + a `show(ui, &mut state)` function that `main.rs` composes:
- **Battery**: slider + Apply button, wired to the daemon over blocking
  zbus, works.
- **Platform Profile**: Quiet/Balanced/Performance buttons select locally,
  Apply sends the choice to the daemon (same `apply_...`/`fetch_...` +
  `ApplyStatus` shape as Battery). Compiles; not yet tested live.
- **Updates** (new): "Operating System" panel. "Check for updates" calls
  `PreviewOs` synchronously (fast, read-only, same as every other button
  in this app so far) and shows what `apt` found, or "Everything is up to
  date." if the output was empty. "Update now" (only enabled once updates
  are available) opens a confirm dialog -- calm, factual wording about
  what's about to happen and that a restart may be needed afterward, not
  an alarming one -- and only clicking "Yes, update" in that dialog
  actually calls `ApplyOs`. Two things here are new compared to the other
  two panels:
  - **The apply call runs on a background OS thread**
    (`std::thread::spawn`), not directly inside `show()`. `apt upgrade`
    can take minutes; battery/platform-profile's writes are instant, so
    calling the daemon straight from `show()` (which runs on egui's single
    UI thread) never mattered before, but doing that here would freeze the
    whole window for however long the update takes. `State::apply` is an
    `Arc<Mutex<ApplyStatus>>>` the background thread reports into;
    `show()` just reads it each frame like any other field, and the
    background thread calls `ctx.request_repaint()` when it's done so the
    final result is guaranteed to actually get drawn.
  - **Two-step preview/confirm**, matching what Ali asked for: nothing is
    applied on the first click anywhere in this panel.
  - Compiles, runs without panicking (confirmed via `cargo run` this
    session). **Not visually confirmed** -- no screenshot tool was
    available in this session to check the dialog/layout actually look
    right; Ali should open it and click through once before trusting it.

## Key conventions established (keep using these)

- **Module layout mirrors `config/project-description.txt`'s four sections**
  (System Control, System Monitoring, Fan Control, System Diagnostics).
  Each section that has real code gets its own `src/<section>/` dir in both
  crates, with `mod.rs` exposing only what the rest of the crate needs.
- **Prefer the simplest type that satisfies today's actual callers.** Don't
  reach for a typed enum (or other ceremony) preemptively just because a
  sibling module has one -- add it once something outside the module
  actually needs it.
- D-Bus bus name `org.controlcenter.Daemon1`, object path
  `/org/controlcenter/Daemon1`; each hardware/system area gets its own
  interface under that same path.
- One `ControlError` enum (thiserror) per crate side that needs it, mapped
  through a single `From<ControlError> for zbus::fdo::Error` impl -- never
  leak raw `io::Error`/paths across the bus.
- Every hardware write follows validate -> write -> read-back-to-verify,
  because some ASUS sysfs controls silently clamp/ignore invalid values
  instead of rejecting them.
- D-Bus-level `.conf` policy is deliberately wide open (any local user can
  *call*); polkit is the layer that actually gates *writing*. Two separate
  concerns, two separate files.
- GUI talks to the daemon with **blocking** zbus (`zbus::blocking::Connection`
  + `...ProxyBlocking`), not async -- keeps the GUI crate free of a tokio
  dependency since eframe already drives its own event loop. The one
  exception: an operation whose D-Bus call can legitimately take minutes
  (so far, just `ApplyOs`) still uses the blocking client, but from a
  spawned background thread rather than directly inside `show()`, so it
  doesn't freeze the window. See the Updates GUI entry above.
- **Polkit's `allow_active` tier should match the action's actual blast
  radius, not default to "passwordless."** Charge-limit/platform-profile
  are `allow_active=yes` because worst case is minor hardware wear; the
  new `apply-os-update` action is `allow_active=auth_admin` (prompts for
  the admin password every time) because `apt upgrade` can touch the
  kernel and anything installed. Judge each new action on its own risk,
  don't copy the previous one's tier by default.
- **An operation that needs broad filesystem write access doesn't belong
  in `controlcenterd` itself if the daemon's unit is sandboxed
  (`ProtectSystem=strict` etc., see `controlcenterd.service`).** Applying
  an update needs to write to `/var`, `/etc`, `/boot`, `/usr` -- exactly
  what that sandboxing exists to prevent. Rather than loosen the always-on,
  always-D-Bus-reachable daemon's sandbox, updates run in a **separate,
  unsandboxed, on-demand oneshot unit** (`controlcenterd-updates@.service`)
  that the daemon starts (it's already root) and waits on, and which only
  exists for the few minutes an update actually takes. Keeps the
  permanently-running attack surface small; the broad-permission code only
  runs transiently, on a fixed script, never reachable over D-Bus itself.
  Any future category that needs similar broad access (drivers, packages)
  reuses this same unit as a new instance (`@drivers`, `@packages`), not a
  new sandbox exception bolted onto `controlcenterd.service`.
- **A oneshot unit's output is captured to a fixed path by the script
  itself** (`mkdir -p` + `exec >log 2>&1` at the top of
  `update-os.sh`), not via the unit file. First attempt used
  `RuntimeDirectory=`/`StandardOutput=file:/path` instead -- looked
  right per the docs, but failed live every time
  (`status=209/STDOUT`, "Failed to set up standard output: No such
  file or directory") because systemd tried to open the log file
  before `RuntimeDirectory=` had created its parent directory. Never
  root-caused *why* the ordering didn't hold; the script owning its
  own output sidesteps the question entirely and is simpler besides.
- **`set -euo pipefail` at the top of an update script is what gives
  "stop at the first failure"** -- Ali's explicit requirement for
  sequential updates. No orchestration logic needed for that within one
  category; across categories (once Drivers/Packages exist) the GUI is
  expected to check each category's result before starting the next one,
  same idea one level up.

## Gotchas hit and resolved this/previous sessions (don't re-discover these)

- **A directory-scan `discover_*` function only makes sense when the sysfs
  path actually varies per device** (like battery's `BAT0`/`BAT1`). A
  single fixed path (like `platform_profile`) just needs an existence
  check, not a scan.
- **Two test modules using the identical temp-dir name race under `cargo
  test`'s parallel execution** -- give each test module its own distinct
  directory name.
- **`pub use`-ing an item nothing calls yet still trips `dead_code`/
  `unused_imports` in a *binary* crate, even though it's `pub`.**
- **eframe 0.36 needs rustc 1.95**; pinned to `eframe = "=0.32.3"`. **XML
  comments can't contain `--`** anywhere in the body. **`#[zbus(property)]`
  auto-generates a `<name>_changed` method** -- don't hand-roll a signal
  with that name. **A `.conf` rule scoped to `send_interface` doesn't
  cover property reads/writes** (those go over
  `org.freedesktop.DBus.Properties`).
- **Confirmed live: a polkit `.policy` file with an invalid XML comment
  (containing `--`) fails to register *any* of its actions**, not just
  the one near the bad comment -- `pkaction` silently shows nothing new
  and the daemon's error is a generic "Action ... is not registered",
  with no hint that the cause is a markup typo three actions away. Worth
  a sanity check (`pkaction | grep controlcenter`) after any policy file
  edit before assuming the actual authorization logic is what's wrong.
- **`apt list --upgradable` writes its "Listing... Done" progress line to
  stderr, not stdout** -- capturing only stdout (as `preview_os` does)
  means an up-to-date system's output is genuinely empty, which is what
  the GUI checks (`output.trim().is_empty()`) to show "Everything is up to
  date." instead of an empty list. Not yet confirmed against a real `apt`
  run this session (see Testing status).
- **`tokio`'s `process` feature isn't on by default** and wasn't in this
  workspace's `tokio` dependency until this session (`updates.rs` needs
  `tokio::process::Command`) -- added to `[workspace.dependencies]` in the
  root `Cargo.toml` so both crates stay on the same feature set.

## Testing status

- ✅ **`cargo build --workspace` passes**, both crates. Warnings only:
  unused `zbus::blocking::connection` import in `platform_profile.rs`,
  unused `log::debug` import in `battery.rs` (both GUI-side, pre-existing,
  not fixed).
- ✅ `cargo test -p controlcenterd` passes (3 tests: battery bounds-check +
  round-trip, platform-profile round-trip). No tests added for
  `updates.rs` -- it's process-spawning I/O with nothing to unit-test
  without mocking a subprocess, same reasoning as why `platform_profile`'s
  `validate` has no test.
- ✅ `cargo run -p controlcenter-gui` starts and runs without panicking
  (confirmed this session) -- but **not visually verified**, no
  screenshot tool was available. Ali should actually click through the
  Updates panel (Check for updates / the confirm dialog's wording and
  layout / Update now) before trusting it looks right.
- ✅ Daemon previously confirmed live for Battery. Platform profile and
  the entire Updates feature have **never** been installed/restarted on
  the real system -- untested live. See the deploy checklist below.
- ⚠️ **Still known-broken, not confirmed fixed**: reading the `ChargeLimit`
  property fails with `Access denied` live (`busctl get-property ...`
  confirmed in an earlier session). The fix is in
  `dbus/org.controlcenter.Daemon1.conf`, but it still needs to be
  reinstalled/reloaded by Ali.
- ❌ Not yet tested live: the GUI's Apply/Update-now buttons, for Battery,
  Platform Profile, or Updates, against the real daemon. For Updates
  specifically, also untested: the `controlcenterd-updates@os.service`
  unit actually running as root outside the main daemon's sandbox, and
  whether `ProtectSystem=strict` on the *main* daemon still permits it to
  read back `/run/controlcenterd/update-os.log` (expected to work --
  `ProtectSystem` only forces paths read-only, it doesn't hide them -- but
  this is reasoning from the systemd docs, not something confirmed against
  the real system yet).

## Redeploying after a change

Each packaging file is read by a different subsystem -- only reinstall
what the change actually affects, and use `restart` (not `start`) once the
daemon's already running, since `start` on an already-running unit is a
no-op.

| Changed | Redeploy with |
|---|---|
| `rust-daemon/src/*.rs` | `cargo build --release -p controlcenterd` → reinstall binary → `sudo systemctl restart controlcenterd` |
| `dbus/org.controlcenter.Daemon1.conf` | reinstall the `.conf` → `sudo systemctl reload dbus` (no binary rebuild, no daemon restart -- this is enforced entirely by dbus-daemon, not our code) |
| `dbus/org.controlcenter.Daemon1.service` (activation file) | reinstall it; dbus-daemon reads it fresh on next activation, no explicit reload needed |
| `polkit/org.controlcenter.daemon.policy` | reinstall it; polkit watches its actions directory and picks up changes automatically -- confirm with `pkaction \| grep controlcenter` |
| `systemd/controlcenterd.service` | reinstall it → `sudo systemctl daemon-reload` → `sudo systemctl restart controlcenterd` |
| `systemd/controlcenterd-updates@.service` | reinstall it → `sudo systemctl daemon-reload` (no restart needed -- it's not running continuously, each `systemctl start controlcenterd-updates@<x>.service` picks up the current unit file fresh) |
| `scripts/updates/*.sh` | reinstall to `/usr/local/libexec/controlcenterd/` (must stay executable: `chmod +x`) -- no reload needed, the next run just uses the new script |

Full install commands (paths this project uses):
```bash
cargo build --release -p controlcenterd
sudo install -Dm755 target/release/controlcenterd /usr/local/bin/controlcenterd
sudo install -Dm644 dbus/org.controlcenter.Daemon1.conf /etc/dbus-1/system.d/org.controlcenter.Daemon1.conf
sudo install -Dm644 dbus/org.controlcenter.Daemon1.service /usr/share/dbus-1/system-services/org.controlcenter.Daemon1.service
sudo install -Dm644 polkit/org.controlcenter.daemon.policy /usr/share/polkit-1/actions/org.controlcenter.daemon.policy
sudo install -Dm644 systemd/controlcenterd.service /etc/systemd/system/controlcenterd.service
sudo install -Dm644 systemd/controlcenterd-updates@.service /etc/systemd/system/controlcenterd-updates@.service
sudo install -Dm755 scripts/updates/update-os.sh /usr/local/libexec/controlcenterd/update-os.sh
sudo systemctl daemon-reload
sudo systemctl restart controlcenterd
```

## TODO: first live deploy (checklist)

Platform profile and the whole Updates feature have never been
installed/restarted on the real system -- everything below has only been
checked with `cargo build`/`cargo run`, not against the real D-Bus bus or
a real `apt`. Delete lines as you get through them.

- [ ] Run the full install block above (includes the new updates unit +
      script)
- [ ] `systemctl status controlcenterd --no-pager` -- confirm it's active,
      no crash-loop
- [ ] `busctl introspect org.controlcenter.Daemon1 /org/controlcenter/Daemon1`
      -- confirm `...Battery`, `...PlatformProfile`, and `...Updates` are
      all listed
- [ ] `cargo run -p controlcenter-gui` -- click Apply on Battery and
      Platform Profile against the real daemon (neither tested live
      end-to-end through the GUI yet)
- [ ] While in there: confirm the long-standing `ChargeLimit` "Access
      denied" bug (see Testing status) is actually gone now that the
      `.conf` is reinstalled -- `busctl get-property ... ChargeLimit`
- [ ] Updates panel: click "Check for updates" -- confirm it shows real
      `apt list --upgradable` output (or "Everything is up to date.")
- [ ] Click "Update now" -- confirm the confirm dialog's wording/layout
      actually look right (never visually checked, see Testing status)
- [ ] Click "Yes, update" -- confirm `apt-get update && apt-get upgrade`
      actually runs as root via `controlcenterd-updates@os.service`,
      polkit prompts for the admin password (`auth_admin`, not
      passwordless), and the GUI shows Success/Error correctly without
      freezing the window while it runs
- [ ] `systemctl status controlcenterd-updates@os.service` after a run --
      confirm it's `inactive`/exited cleanly (it's a oneshot, should not
      stay running)
- [ ] Deliberately break something (e.g. no network) and re-run Update to
      confirm the failure path shows something useful (the tail of
      `/run/controlcenterd/update-os.log`), not just a bare error

## Immediate next steps

1. **Work through the "TODO: first live deploy" checklist above.**
2. Tidy the two remaining unused-import warnings in the GUI crate
   (`zbus::blocking::connection` in `platform_profile.rs`, `log::debug`
   in `battery.rs`).
3. **Decide on `log` vs `tracing`.** `log = "0.4.34"` is in *both* crates'
   `Cargo.toml` (from debugging the charge-limit-always-100 issue) --
   the daemon already uses `tracing` throughout; worth resolving one way
   or the other.
4. Once OS updates are confirmed working live: add **Drivers**
   (`fwupdmgr`) and **Packages** (`snap`/`flatpak`) following the exact
   same shape (`preview_<category>`/`apply_<category>` in `updates.rs`,
   a new script + polkit action + `controlcenterd-updates@<category>`
   instance, a new section in the GUI's `updates.rs`), plus the
   right-most "Update All" button Ali described -- sequential, stopping
   at the first category that fails (GUI-side: call `ApplyOs`, check the
   result, only then call `ApplyDrivers`, etc.).
5. Longer-term / explicitly deferred: rate limiting, state
   persistence/restore-on-boot, `controlctl` CLI client, C++ hardware
   layer, System Monitoring / Fan Control / Diagnostics interfaces (all
   in the original project description, not started).

## Real hardware facts discovered (useful, don't re-probe blindly)

- Battery: `BAT1`, threshold file at
  `/sys/class/power_supply/BAT1/charge_control_end_threshold`
- `asus_wmi` / `asus_nb_wmi` kernel modules loaded; ASUS ACPI devices
  present (`ASUS2018:00`, `ASUS9001:00`)
- `platform_profile` sysfs works; `platform_profile_choices` confirmed
  live as `"quiet balanced performance"`.
- Real desktop session available: Wayland, `dbus-daemon`, `systemctl`,
  `pkaction`, `busctl` all present and usable for live testing. No
  screenshot tool installed (`grim`/`gnome-screenshot`/etc. all absent) --
  GUI visual checks need Ali's own eyes, not Claude's.
- OS is Ubuntu 25.10 (`apt`-based). Also has `snap`, `flatpak`, and
  `fwupdmgr` available -- the tools Drivers/Packages categories will use.
  NVIDIA driver (`nvidia`/`nvidia_drm`/`nvidia_modeset`/`nvidia_uvm`
  kernel modules loaded) is apt-managed on Ubuntu, not a separate channel
  -- this is why "OS" covers it rather than it being its own category.
