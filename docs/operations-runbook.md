# ZeroBuild Operations Runbook

This runbook is for operators who maintain availability, security posture, and incident response.

Last verified: **February 18, 2026**.

## Scope

Use this document for day-2 operations:

- starting and supervising runtime
- health checks and diagnostics
- safe rollout and rollback
- incident triage and recovery

For first-time installation, start from [one-click-bootstrap.md](one-click-bootstrap.md).

## Runtime Modes

| Mode | Command | When to use |
|---|---|---|
| Foreground runtime | `zerobuild daemon` | local debugging, short-lived sessions |
| Foreground gateway only | `zerobuild gateway` | webhook endpoint testing |
| User service | `zerobuild service install && zerobuild service start` | persistent operator-managed runtime |

## Baseline Operator Checklist

1. Validate configuration:

```bash
zerobuild status
```

2. Verify diagnostics:

```bash
zerobuild doctor
zerobuild channel doctor
```

3. Start runtime:

```bash
zerobuild daemon
```

4. For persistent user session service:

```bash
zerobuild service install
zerobuild service start
zerobuild service status
```

## Health and State Signals

| Signal | Command / File | Expected |
|---|---|---|
| Config validity | `zerobuild doctor` | no critical errors |
| Channel connectivity | `zerobuild channel doctor` | configured channels healthy |
| Runtime summary | `zerobuild status` | expected provider/model/channels |
| Daemon heartbeat/state | `~/.zerobuild/daemon_state.json` | file updates periodically |

## Logs and Diagnostics

### macOS / Windows (service wrapper logs)

- `~/.zerobuild/logs/daemon.stdout.log`
- `~/.zerobuild/logs/daemon.stderr.log`

### Linux (systemd user service)

```bash
journalctl --user -u zerobuild.service -f
```

## Incident Triage Flow (Fast Path)

1. Snapshot system state:

```bash
zerobuild status
zerobuild doctor
zerobuild channel doctor
```

2. Check service state:

```bash
zerobuild service status
```

3. If service is unhealthy, restart cleanly:

```bash
zerobuild service stop
zerobuild service start
```

4. If channels still fail, verify allowlists and credentials in `~/.zerobuild/config.toml`.

5. If gateway is involved, verify bind/auth settings (`[gateway]`) and local reachability.

## Safe Change Procedure

Before applying config changes:

1. backup `~/.zerobuild/config.toml`
2. apply one logical change at a time
3. run `zerobuild doctor`
4. restart daemon/service
5. verify with `status` + `channel doctor`

## Rollback Procedure

If a rollout regresses behavior:

1. restore previous `config.toml`
2. restart runtime (`daemon` or `service`)
3. confirm recovery via `doctor` and channel health checks
4. document incident root cause and mitigation

## Related Docs

- [one-click-bootstrap.md](one-click-bootstrap.md)
- [troubleshooting.md](troubleshooting.md)
- [config-reference.md](config-reference.md)
- [commands-reference.md](commands-reference.md)
