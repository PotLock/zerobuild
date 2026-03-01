# Sandbox Security Model

This document explains how `LocalProcessSandboxClient` isolates code execution,
compares it against the E2B and Docker providers it replaces, and gives an honest
assessment of what it does and does not protect against.

---

## 1. What the local sandbox actually does

When the agent calls `sandbox_create`, the runtime:

1. Creates a fresh temporary directory: `$TMPDIR/zerobuild-sandbox-{uuid}/`
2. Pre-populates sub-directories so npm never writes outside:
   - `.npm-cache/` — npm package cache
   - `.npm-global/` — npm global prefix
   - `tmp/` — scratch space
3. Runs every subsequent command via `tokio::process::Command` with:
   - `env_clear()` — strips the entire parent environment
   - A minimal reconstructed environment (see §2)
   - `kill_on_drop(true)` — child is killed if the Rust future is dropped
   - `tokio::time::timeout` — hard wall-clock limit per command
4. Validates every file path through `safe_join()` before any read/write:
   - Strips leading `/`
   - Iterates path components, **rejects any `Component::ParentDir` (`..`)**
   - Only `Component::Normal` segments are accepted

When `sandbox_kill` is called, the entire directory tree is removed with
`std::fs::remove_dir_all`.

---

## 2. Environment isolation

The child process inherits **only** the following variables:

| Variable | Value | Purpose |
|---|---|---|
| `PATH` | copied from host | find `node`, `npm`, `npx`, etc. |
| `HOME` | sandbox dir | prevents reads from `~/.ssh`, `~/.config`, `~/.aws`, etc. |
| `TMPDIR` | `sandbox_dir/tmp` | scratch writes stay inside sandbox |
| `NPM_CONFIG_CACHE` | `sandbox_dir/.npm-cache` | npm cache stays inside sandbox |
| `NPM_CONFIG_PREFIX` | `sandbox_dir/.npm-global` | global npm installs stay inside sandbox |
| `NPM_CONFIG_UPDATE_NOTIFIER` | `false` | suppress network calls |
| `NEXT_TELEMETRY_DISABLED` | `1` | suppress Next.js telemetry |
| `CI` | `1` | non-interactive mode for tools |
| `LANG` | copied from host | locale / encoding |
| `TERM` | `xterm-256color` | terminal type |

Everything else — `AWS_*`, `GCP_*`, `GITHUB_TOKEN`, `SSH_AUTH_SOCK`,
`ANTHROPIC_API_KEY`, `DATABASE_URL`, proxy settings, and any other secrets in
the parent environment — is **not visible** to the child process.

---

## 3. Filesystem isolation

Path validation is enforced at the Rust level before any syscall reaches the OS:

```rust
fn safe_join(sandbox_dir: &Path, relative: &str) -> anyhow::Result<PathBuf> {
    for component in Path::new(stripped).components() {
        match component {
            Component::ParentDir => bail!("Path traversal rejected: '..'"),
            Component::Normal(part) => result.push(part),
            _ => {}
        }
    }
}
```

This means:

- `write_file("../etc/passwd", ...)` → **rejected with an error**
- `read_file("../../root/.ssh/id_rsa")` → **rejected with an error**
- `list_files("/home/user")` → **rejected** (leading `/` is stripped; `home/user`
  resolves inside sandbox)

File operations that succeed are physically scoped to the sandbox directory.
After `sandbox_kill`, the directory is deleted — no leftover artefacts on disk.

---

## 4. Comparison with E2B and Docker

| Property | LocalProcess | Docker | E2B |
|---|---|---|---|
| **External dependency** | None | Docker daemon | `E2B_API_KEY` + network |
| **Availability** | Always | Only if Docker running | Only if API reachable |
| **Kernel-level isolation** | ❌ No | ✅ Yes (namespaces + cgroups) | ✅ Yes (MicroVM / Firecracker) |
| **Syscall filtering** | ❌ No | ⚠️ Optional (seccomp) | ✅ Yes |
| **CPU / memory limits** | ❌ None | ✅ Configurable | ✅ Enforced by platform |
| **Network access** | Unrestricted | Configurable | Configurable |
| **FS escape via path** | ✅ Blocked (Rust) | ✅ Blocked (container rootfs) | ✅ Blocked (VM boundary) |
| **Env credential leak** | ✅ Blocked (env_clear) | ✅ Blocked (isolated env) | ✅ Blocked (isolated env) |
| **Process runs as** | Same OS user | `root` inside container | Isolated VM user |
| **Cost** | Free | Free | Paid API |
| **Cold-start latency** | ~0ms | 5–30 s (image pull) | 2–10 s |

### What E2B and Docker give you that local does not

**Docker** wraps the child in Linux namespaces (`pid`, `net`, `mnt`, `uts`,
`ipc`) and cgroups. A rogue process cannot see other host processes, cannot bind
to host network interfaces (with `--network none`), and is resource-limited.
Even if it escapes the filesystem jail, it cannot reach the host kernel directly
without a container-escape CVE.

**E2B** goes further: it uses Firecracker MicroVMs, giving each sandbox a
separate guest kernel. A full kernel-level exploit is needed to escape — the
attack surface is orders of magnitude smaller than a container or a plain process.

**LocalProcessSandboxClient** does none of that. The child `sh` process runs
with the same UID as the ZeroBuild agent, in the same kernel namespace, with
unrestricted syscalls.

---

## 5. Honest threat assessment

### What is protected

| Threat | Protected? | Mechanism |
|---|---|---|
| Reading `~/.ssh`, `~/.aws`, etc. | ✅ Yes | `HOME` redirected; `env_clear` |
| Stealing `GITHUB_TOKEN` or API keys from env | ✅ Yes | `env_clear` |
| Writing files outside the sandbox dir | ✅ Yes | `safe_join` path validation |
| Reading files outside the sandbox dir via the tool API | ✅ Yes | `safe_join` path validation |
| Leftover artefacts after kill | ✅ Yes | `remove_dir_all` on kill |
| Runaway processes after timeout | ✅ Yes | `kill_on_drop` + `timeout` |

### What is NOT protected

| Threat | Protected? | Notes |
|---|---|---|
| Direct `open("/etc/passwd")` inside the command | ❌ No | The shell can open any path the OS user can read |
| Reading `~/.ssh` by passing the absolute path to `sh -c` | ❌ No | The command string itself is not sanitised |
| Writing to arbitrary host paths via the command string | ❌ No | e.g. `sh -c "echo x > /tmp/evil"` is allowed |
| Spawning background processes that outlive the timeout | ⚠️ Partial | `kill_on_drop` kills the direct child; orphaned grandchildren may survive if they `setsid()` |
| Network connections to internal services | ❌ No | No network namespace isolation |
| Resource exhaustion (fork bomb, disk fill) | ❌ No | No cgroup limits |
| Privilege escalation via SUID binaries | ❌ No | OS-level, not addressed |

The key attack vector is the **command string itself**. `run_command` accepts an
arbitrary shell command. If the LLM (or an adversarial prompt injection in the
user's codebase) emits a command like:

```bash
cat ~/.ssh/id_rsa
```

the sandbox will execute it and return the output. The `safe_join` path guard
only applies to the **tool-level file API** (`write_file`, `read_file`,
`list_files`), not to the shell command string.

---

## 6. Why this trade-off is acceptable for ZeroBuild

ZeroBuild's threat model is:

- The LLM writing the code is **the operator's own agent** — not untrusted user
  code from arbitrary third parties.
- The primary risks are **accidental**, not adversarial:
  - Accidentally writing to the wrong path (`../../`)
  - Accidentally leaking credentials via env vars
  - Leaving temp files behind
  - Getting stuck in an infinite loop

All three of those accidental risks are fully addressed by the local sandbox.

The local sandbox is **not suitable** for:

- Running untrusted code submitted by end-users
- Multi-tenant environments where different users share the same agent process
- Security-sensitive pipelines where the agent reads secrets and the generated
  code could exfiltrate them via the network

For those use cases, Docker (with `--network none` and resource limits) or E2B
would be the correct choice.

---

## 7. Potential hardening steps (future work)

If stronger isolation becomes necessary without reintroducing Docker or E2B:

| Technique | What it adds |
|---|---|
| `landlock` (Linux 5.13+) | Kernel-enforced FS path restrictions on the child PID |
| `seccomp-bpf` | Syscall allowlist — blocks `connect()`, `execve()` of unexpected binaries, etc. |
| `unshare --user --net --pid` | Unprivileged user namespace + network namespace, no Docker required |
| macOS `sandbox-exec` profile | macOS equivalent of `landlock`/seccomp |
| `rlimit` (CPU, file size, process count) | Prevents resource exhaustion |

These can be layered on top of the existing implementation without changing the
`SandboxClient` trait interface.

---

## 8. Summary

`LocalProcessSandboxClient` is a **lightweight, dependency-free isolation layer**
that prevents the most common accidental risks in an LLM-driven build workflow:
credential exposure via environment variables and accidental writes outside the
working directory.

It is **not** a hardened security boundary. It does not provide kernel-level
isolation, syscall filtering, or network restrictions. For the specific use case
of a trusted agent building its own Next.js projects, the protection it provides
is sufficient. For any scenario involving untrusted code or multi-tenant access,
a VM-level sandbox (E2B) or at minimum a properly configured Docker container
should be used instead.
