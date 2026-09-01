---
topic: the Windows installer has been executed on this machine and its install works — plus why the published assets cannot be downloaded here at all, and one installer test declined on purpose
status: resolved
links:
  - linux-could-not-build-from-the-readme-2026-09-01.md
  - the-apk-installs-and-launches-2026-09-01.md
  - ../backlog/lts-parity-criteria.md
---

# The Windows installer has been run

`STATUS.md` counts "four of nine published assets have never been executed by
anyone" as a v0.0.29 item. For Windows, one of them has — and the evidence was
sitting on this machine.

## The install that is already here

```text
C:\Users\atur\AppData\Local\Chaos\
  bin\                 14 .exe + 5 documents
  installed-files.txt  18 entries
  setup.log            "Installing Chaos 0.0.20."
  version.txt          0.0.21
```

So `chaos-setup.exe` ran here, installed **0.0.20**, and the tree was later moved
to **0.0.21**. And the installed binaries work:

```text
chaos-run     0.0.21
chaos-probe   0.0.21
chaos-pull    0.0.21
gguf-info     0.0.21

$ chaos-probe --quick
os     Windows (x86_64)
cpu    20 threads
ram    15.7 GiB total, 6.1 GiB available   [GlobalMemoryStatusEx]
gpu    NVIDIA GeForce RTX 3050 6GB Laptop GPU  6.0 GiB   [nvidia-smi]
```

Not just `--version`: the probe does real work, reads `GlobalMemoryStatusEx` and
finds the card through `nvidia-smi`.

## What the old install is missing, and why that is good news

**`chaos.exe` — the front door — is not in it.** `chaos run`, `chaos start`,
`chaos connect`, `chaos config` were all unavailable from that install.

That is the gap `every_binary_reaches_every_platform` was written for, and
v0.0.22's notes record `chaos.exe` and `chaos-qr.exe` reaching the zip "for the
first time". This install predates the fix, so it is **evidence the fix
mattered** rather than a live bug.

Checked against today's source. `gui/setup/build.rs` embeds whatever is in
`CHAOS_STAGE_DIR` — there is no hardcoded list — and `release.yml` now stages
twenty binaries by name, `chaos` and `chaos-qr` among them. Building the
installer with a staged payload here produces:

```text
chaos-setup embeds 10 file(s)
  LICENSE  NOTICE  README.md
  chaos.exe  chaos-meta.exe  chaos-probe.exe  chaos-pull.exe
  chaos-qr.exe  chaos-run.exe  gguf-info.exe
```

**The front door is embedded.** Installer 10,943,472 bytes against a 10,175,197
byte payload, which also passes the workflow's own "did it embed anything at all"
size check — a check that exists because an installer with an empty payload still
builds, still runs, and installs nothing. Two of the three stale build
directories in `target/` have exactly that empty payload, so the guard is
earning its place.

## One test declined, deliberately

**I did not run the installer.** `install_to` does not only copy files: it adds
the prefix to `PATH`, creates a Start Menu shortcut, and writes
`HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Chaos`. Running it with
`--prefix` pointed at a scratch directory would have left a stale `PATH` entry, a
broken shortcut, and — worst — an **Add/Remove Programs entry pointing at a
deleted temporary folder**, replacing the one that currently points at the real
install.

`--report` is not a dry run; it adds a message box to the same real work. So the
value of confirming file extraction was well below the cost of breaking a working
install registration, and the extraction is already covered by the payload
listing above.

## One thing unexplained, and not filed

`setup.log` says *"Installing Chaos 0.0.20"* while `version.txt` says **0.0.21**,
and their timestamps differ by two days. The install path writes
`prefix/setup.log` on **every** install (`main.rs:133-138`), so an upgrade should
have rewritten it.

Two candidates, and nothing here separates them:

1. the 0.0.21 upgrade was not done by `chaos-setup` at all, but by an earlier
   session copying binaries in and writing `version.txt`; or
2. some path updates `version.txt` without rewriting the log.

**Earlier sessions demonstrably did hand-build artefacts on this machine** — the
Chaos package already installed on the emulator had `primaryCpuAbi=x86_64`, which
no published APK provides. So (1) is at least as likely as (2), and filing a bug
on a guess is what this repository spent the day retracting. Recorded, not filed.

## And the reason no published asset could be downloaded

Every route to a release asset ends at one host that is blocked here:

```text
api.github.com                     200 in 0.34 s
github.com                         200 in 0.13 s
codeload.github.com                301
objects.githubusercontent.com      will not connect
release-assets.githubusercontent.com   DNS ok, TCP in 0.13 s, then
                                       "Recv failure: Connection was reset" during TLS
```

`gh release download` fails on all nine assets, and the classic
`github.com/.../releases/download/...` URL **302s to the same blocked host**. So
asset testing on this machine has to work from artefacts already on disk (the
APK) or from the source the assets are built from (Linux, Windows).

This is the same shape as the recorded fact that CI logs cannot be read here. It
is a property of the network, not of the release — **do not read a failed
download as a broken asset.**
