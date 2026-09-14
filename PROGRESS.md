# Progress Notes

Read this first in a new session. It's the current, accurate state of the
project — what's built, why it's built that way, what's deliberately not
done yet, and what to do next.

## What this project is

Linux-native hardware monitoring/control/diagnostics platform for an ASUS
ROG Zephyrus G16, split Rust (daemon, control logic) + C++ (hardware layer,
NVML). Full scope is in `config/project-description.txt`. **System
Control** is the interface being built first: battery charge limit is done
end-to-end; platform profile is underway (daemon side built, GUI side
currently broken mid-edit -- see Testing status).

Ali (the user) is a 3rd-year student building this for resume/skills in
systems/embedded/DevOps. New to Rust, D-Bus, polkit, and packaging;
comfortable with hardware. Working style: **small increments, explain
everything, don't jump ahead** -- confirm before adding scope beyond what
was explicitly asked. Prefers short answers to "quick question"-labeled
messages, fuller explanations otherwise. He's now implementing pieces
himself (platform profile's GUI-side D-Bus proxy, in progress) after
seeing the battery pattern built for him first.

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
controlcenter-gui/                      the GUI crate (package name: controlcenter-gui)
  Cargo.toml
  src/main.rs                             eframe App shell; composes section panels
  src/system_control/                     mirrors the daemon's section split
    mod.rs                                  State struct + show() composing the panels below
    battery.rs                              D-Bus proxy + Battery panel, wired to the daemon, works
    platform_profile.rs                     Platform Profile panel; Ali is mid-rewrite adding a
                                             D-Bus proxy here -- currently does not compile
dbus/
  org.controlcenter.Daemon1.conf          bus-level access policy
  org.controlcenter.Daemon1.service       D-Bus activation file
polkit/
  org.controlcenter.daemon.policy         set-charge-limit + set-platform_profile action definitions
systemd/
  controlcenterd.service                  systemd unit (sandboxed, D-Bus-activated)
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

**Daemon (`rust-daemon`)** -- two D-Bus interfaces at bus name
`org.controlcenter.Daemon1`, object path `/org/controlcenter/Daemon1`
(confirmed today: one object path can host multiple interfaces via
repeated `.serve_at()` calls on the same `Builder`):

- **`...Battery`**: `Supported` (bool), `ChargeLimit` (u8, live sysfs read),
  `SetChargeLimit(u8) -> u8` -- polkit-gated, validate/write/read-back,
  emits `PropertiesChanged`. Unchanged this session, still fully working.
- **`...PlatformProfile`** (new this session): `Supported` (bool),
  `Profile` (`String`, live read of `/sys/firmware/acpi/platform_profile`),
  `SetProfile(String) -> String` -- same polkit-gate/validate/write/
  read-back/`PropertiesChanged` shape as Battery. `validate` checks the
  requested value against the live `platform_profile_choices` file rather
  than a hardcoded list. Deliberately uses plain `String`, not a typed
  enum -- decided together: `validate` already re-checks against the real
  hardware file on every call, so a `Profile` enum wasn't buying extra
  safety, just ceremony (`FromStr`, `Copy`, etc.), and nothing outside this
  module consumes a typed value yet. Revisit only once something (a GUI
  dropdown, say) actually wants a fixed, typed set of choices.
  Authorization: action `org.controlcenter.daemon.set-platform_profile`
  (see polkit note below).
- `main.rs` currently wires both interfaces, but has rough edges from
  Ali's own most recent edit: an apparently-unused
  `use zbus::zvariant::Signature::ObjectPath;` import (1 build warning,
  not fatal), and typos (`platform_prfile_iface` variable name,
  "platoform profile" in a log message). Cosmetic, not blocking.

Authorization: `polkit.rs`'s `authorize()` is generic (connection, header,
action_id) and unchanged -- both interfaces just call it with their own
action id. `polkit/org.controlcenter.daemon.policy` now has both actions,
both `allow_active=yes` (any active local session, no password prompt).
Known inconsistency (not fixed, flagged only): the new action id uses an
underscore (`set-platform_profile`) where the existing one uses hyphens
throughout (`set-charge-limit`).

**Not in the daemon (deliberately, for now):** rate limiting, state
persistence/restore-on-restart, a CLI client (`controlctl`). These existed
in the stashed earlier pass and can be ported back in when asked.

**GUI (`controlcenter-gui`)** -- eframe/egui app, one window, two sections,
each its own module under `system_control/` with a `State` struct + a
`show(ui, &mut state)` function that `main.rs` composes:
- **Battery**: slider + Apply button, wired to the daemon over blocking
  zbus, works. (Minor leftover in `Default::default()`: calls
  `fetch_charge_limit()` twice -- once to log it, once for the actual
  field -- harmless but redundant; not fixed, not asked to be.)
- **Platform Profile**: still shows Quiet/Balanced/Performance buttons as
  a local-only mock, but Ali is mid-rewrite adding a real D-Bus proxy
  (following the Battery pattern). **Currently broken** -- see Testing
  status; this blocks `cargo build --workspace` for the whole repo.

## Key conventions established (keep using these)

- **Module layout mirrors `config/project-description.txt`'s four sections**
  (System Control, System Monitoring, Fan Control, System Diagnostics).
  Each section that has real code gets its own `src/<section>/` dir in both
  crates, with `mod.rs` exposing only what the rest of the crate needs.
  Platform Profile followed this exactly, one file per concern
  (`platform_profile.rs` sysfs I/O, `platform_profile_iface.rs` D-Bus).
- **Prefer the simplest type that satisfies today's actual callers.** Don't
  reach for a typed enum (or other ceremony) preemptively just because a
  sibling module has one -- add it once something outside the module
  actually needs it. (This is why platform profile uses `String`, not an
  enum, unlike the GUI's separate local-only mock `Profile` enum.)
- D-Bus bus name `org.controlcenter.Daemon1`, object path
  `/org/controlcenter/Daemon1`; each hardware area gets its own interface
  under that same path.
- One `ControlError` enum (thiserror) per crate side that needs it, mapped
  through a single `From<ControlError> for zbus::fdo::Error` impl -- never
  leak raw `io::Error`/paths across the bus. New this session:
  `InvalidProfile { value, choices }`, mapped to `InvalidArgs` like
  `OutOfRange`.
- Every hardware write follows validate -> write -> read-back-to-verify,
  because some ASUS sysfs controls silently clamp/ignore invalid values
  instead of rejecting them.
- D-Bus-level `.conf` policy is deliberately wide open (any local user can
  *call*); polkit is the layer that actually gates *writing*. Two separate
  concerns, two separate files.
- GUI talks to the daemon with **blocking** zbus (`zbus::blocking::Connection`
  + `...ProxyBlocking`), not async -- keeps the GUI crate free of a tokio
  dependency since eframe already drives its own event loop.

## Gotchas hit and resolved this session (don't re-discover these)

- **A directory-scan `discover_*` function only makes sense when the sysfs
  path actually varies per device** (like battery's `BAT0`/`BAT1`). Copied
  verbatim onto `platform_profile` (a single fixed path,
  `/sys/firmware/acpi/platform_profile`), the same scan pattern never
  actually finds the file -- it joins the filename onto every directory
  entry, including onto the file itself, producing a path that doesn't
  exist. Fixed: `discover_platform_path` is now a plain existence check on
  the fixed path, not a scan.
- **Two test modules using the identical temp-dir name
  (`controlcenterd-test-{pid}`) race under `cargo test`'s parallel
  execution** -- one test's `remove_dir_all` cleanup can delete the other
  test's directory while it's still mid-use, causing an intermittent
  `NotFound` failure that has nothing to do with the code under test.
  Caught when adding `platform_profile`'s test right after `battery`'s.
  Fixed by giving each test module its own distinct directory name.
- **`pub use`-ing an item nothing calls yet still trips `dead_code`/
  `unused_imports` in a *binary* crate, even though it's `pub`.** `pub`
  only controls visibility, not whether rustc considers it reachable from
  `main`. Needed explicit `#[allow(dead_code)]` (on the defining module)
  + `#[allow(unused_imports)]` (on the re-export) while
  `platform_profile`/`platform_profile_iface` weren't wired into `main.rs`
  yet -- removed again once they were wired in.
- (Carried over from before, still true) **eframe 0.36 needs rustc 1.95**;
  pinned to `eframe = "=0.32.3"`. **XML comments can't contain `--`**
  anywhere in the body. **`#[zbus(property)]` auto-generates a
  `<name>_changed` method** -- don't hand-roll a signal with that name.
  **A `.conf` rule scoped to `send_interface` doesn't cover property
  reads/writes** (those go over `org.freedesktop.DBus.Properties`).

## Testing status

- ⚠️ **`cargo build --workspace` currently FAILS.** `controlcenter-gui`
  doesn't compile: `system_control/platform_profile.rs` is mid-edit (Ali's
  own WIP, not reverted -- see Repo layout). Concretely: `apply_profile`'s
  body is incomplete (`let battery = ` with nothing after it), the
  `#[zbus::proxy(...)]` `interface` is set to the *polkit action id*
  (`"org.controlcenter.Daemon1.set-platform-profile"`) instead of the
  *D-Bus interface name* (`"org.controlcenter.Daemon1.PlatformProfile"`,
  which is also an invalid D-Bus interface name as-is), and
  `zbus::blocking::Connection::new(...)` doesn't exist (should be
  `::system()`, per the pattern already working in `battery.rs`).
- ✅ `rust-daemon` alone still builds (`cargo build -p controlcenterd`) --
  1 warning only (the unused import noted above).
- ✅ Battery sysfs logic unit-tested (bounds check + real-file round-trip).
- ✅ Platform profile sysfs logic unit-tested the same way
  (`round_trips_through_a_real_file`); no test for `validate` itself since
  it reads a hardcoded real system path, not an injectable one (same
  limitation the daemon's `validate` for charge limit doesn't have, since
  that one's pure/no I/O).
- ✅ Daemon previously confirmed live for Battery (see below); platform
  profile's daemon-side (`main.rs` wiring, live `SetProfile` call) has
  **not** been installed/restarted on the real system this session --
  untested live.
- ⚠️ **Still known-broken, not confirmed fixed this session**: reading the
  `ChargeLimit` property fails with `Access denied` live
  (`busctl get-property ...` confirmed this again this session). The fix
  is in `dbus/org.controlcenter.Daemon1.conf` (further comment-cleaned
  this session, logic unchanged) but a `sudo install`/reload attempt this
  session failed on authentication -- **still needs to be redeployed by
  Ali** (see Redeploying below).
- ❌ Not yet tested: the GUI's Apply button (Battery) against the real live
  daemon.

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

Full install commands (paths this project uses):
```bash
cargo build --release -p controlcenterd
sudo install -Dm755 target/release/controlcenterd /usr/local/bin/controlcenterd
sudo install -Dm644 dbus/org.controlcenter.Daemon1.conf /etc/dbus-1/system.d/org.controlcenter.Daemon1.conf
sudo install -Dm644 dbus/org.controlcenter.Daemon1.service /usr/share/dbus-1/system-services/org.controlcenter.Daemon1.service
sudo install -Dm644 polkit/org.controlcenter.daemon.policy /usr/share/polkit-1/actions/org.controlcenter.daemon.policy
sudo install -Dm644 systemd/controlcenterd.service /etc/systemd/system/controlcenterd.service
sudo systemctl daemon-reload
sudo systemctl restart controlcenterd
```

## Immediate next steps

1. **Fix `controlcenter-gui/src/system_control/platform_profile.rs`** --
   it's what's blocking the whole workspace build right now. Needs (Ali is
   doing this himself, following the already-working `battery.rs` in the
   same directory as the pattern): the `#[zbus::proxy(...)]` `interface`
   changed to `"org.controlcenter.Daemon1.PlatformProfile"`; a finished
   `apply_profile` body; `Connection::system()` instead of the
   nonexistent `Connection::new(...)`; drop the unused
   `zbus::blocking::connection` import.
2. **Redeploy the `.conf` fix live** -- still not done; needs Ali's sudo
   password (see Redeploying table). Confirm with
   `busctl get-property ... ChargeLimit` afterward.
3. Tidy `rust-daemon/src/main.rs`'s rough edges from the platform-profile
   wiring: drop the unused `Signature::ObjectPath` import, fix
   `platform_prfile_iface` → `platform_profile_iface` and "platoform
   profile" → "platform profile" in the warn message.
4. Once the GUI compiles again: wire the Platform Profile panel's Apply
   button the same way Battery's already is, then retest both live against
   the real daemon end-to-end (GUI Apply was never tested live even for
   Battery -- see Testing status).
5. **Decide on `log` vs `tracing`.** `log = "0.4.34"` was added to *both*
   crates' `Cargo.toml` this session (for `log::debug!` calls added while
   debugging the charge-limit-always-100 issue) -- the daemon already uses
   `tracing` throughout; having two logging crates in one binary is
   probably not intentional and worth resolving one way or the other.
6. Longer-term / explicitly deferred: rate limiting, state
   persistence/restore-on-boot, `controlctl` CLI client, C++ hardware
   layer, System Monitoring / Fan Control / Diagnostics interfaces (all
   in the original project description, not started).

## Real hardware facts discovered (useful, don't re-probe blindly)

- Battery: `BAT1`, threshold file at
  `/sys/class/power_supply/BAT1/charge_control_end_threshold`
- `asus_wmi` / `asus_nb_wmi` kernel modules loaded; ASUS ACPI devices
  present (`ASUS2018:00`, `ASUS9001:00`)
- `platform_profile` sysfs works; `platform_profile_choices` confirmed
  live as `"quiet balanced performance"`. Current value has read as both
  `"performance"` (earlier session) and `"quiet"` (this session) --
  reflects whatever was actually set at the time, not a discrepancy to
  chase.
- Real desktop session available: Wayland, `dbus-daemon`, `systemctl`,
  `pkaction`, `busctl` all present and usable for live testing
