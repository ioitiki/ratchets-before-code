# Report 05: do leases need a distributed lock?

**Brief:** [`../briefs/2026-10-14-05-do-leases-need-a-distributed-lock.md`](../briefs/2026-10-14-05-do-leases-need-a-distributed-lock.md)
**Spec:** [`../specs/2026-10-14-ratchets-before-code-design.md`](../specs/2026-10-14-ratchets-before-code-design.md): §4.2, §6 (required evidence 1), §7 step 5, §10.
**Executed by:** a worker (Claude Code, model Claude Opus 5.5) in its own Orca worktree, on branch `ioitiki/brief-05-lease-lock`.
**Date:** 2026-10-14
**Type:** investigate only. Nothing under `crates/`, `scripts/` or `.github/` changed, and neither did `Cargo.toml`, `Cargo.lock`, `flake.nix` or `docs/decisions/`. This PR adds this report and [`2026-10-14-05-evidence/`](2026-10-14-05-evidence/): the harness `measure.sh` and its raw output, `run-*.txt`.

## Answer

Do leases need a distributed lock? **No**, because nothing inside the design's scope ever produced two live leases on one job. One `ledgerd` took 2,000 submissions of 1,000 keys from 32 concurrent clients, and each of the 1,000 jobs ended with exactly one lease, one intent, one outcome and one `Succeeded` event. A lease left running 5 s past its expiry was never taken over, and the crash test still ends with one `Succeeded` event after `kill -9`. The one double execution I measured needs a second `ledgerd` on the same journal. The design assumes that never happens (spec §4.2), and nothing prevents it. In 3 runs out of 3, B treated A's live lease as dead, ran the effect again, and left two `Succeeded` events for one job. Both processes shared one host and one file, so §10's exclusion of multiple nodes does not cover this case, and a lock service would be the wrong fix for it. What is missing is one lock that is neither distributed nor on leases: an exclusive lock on the journal, taken when `ledgerd` opens it, so that a second process refuses to start. That is finding 1, and the operator decides whether to build it.

## Measured

Every block below is verbatim output. The harness ran inside the dev shell as `nix develop -c bash -o pipefail -c '…'`, and so did shellcheck, the ratchets and cargo. The only edits:
- The worktree's absolute path is shortened to `…`. Each command does this itself with `sed "s|$PWD|…|g"`, so the `run-*.txt` files are already shortened.
- Nix's "Git tree is dirty" warning goes to `nix develop`'s own stderr, which never reached the files.
- `[…]` marks lines left out of a quote. Every line shown is verbatim, and every file is complete in `2026-10-14-05-evidence/`.

In the harness's timelines:
- Each time is the first 200 ms sample that showed a change, counted from A's start. It is not the exact moment of the event.
- Events read `Kind(job)`, and leases read `Leased(job,lease,attempt)`.
- Effects read `(job,attempt)=outcome`. `none` means the intent is recorded and the outcome is not, and `abandoned` is recovery's marker.

### The tools

The dev shell has everything except curl, which comes from the host's system profile. The harness calls no `git`: it finds the repository from its own path.

```
$ nix develop -c bash -o pipefail -c 'for t in bash cargo sqlite3 jq shellcheck curl xargs awk sed grep date mktemp; do printf "%-10s %s\n" "$t" "$(command -v "$t")"; done; sqlite3 --version | cut -d" " -f1; curl --version | head -1 | cut -d" " -f1-2; jq --version; shellcheck --version | sed -n 2p' 2>/dev/null | tee target/brief05-tools.txt
bash       /nix/store/10dxp0qxqxxsyiljrh2kp0xqhz6arhcx-bash-5.3p15/bin/bash
cargo      /nix/store/gmgfpzc0w2pgby0lmq9r88m39albiyfw-cargo-1.98.1/bin/cargo
sqlite3    /nix/store/iy2zkfn4l0w24xabv6ryi0vh7bpjvqm6-sqlite-3.53.3-bin/bin/sqlite3
jq         /nix/store/m3y7hqmy20nd1ps6qy8410a326255bf0-jq-1.8.2-bin/bin/jq
shellcheck /nix/store/x3ssi7xb2qz6p8h4xqjg1xy0j61l998m-shellcheck-0.11.0-bin/bin/shellcheck
curl       /run/current-system/sw/bin/curl
xargs      /nix/store/ycrbcy9sg4knpnik6a01dk70l0m673x7-findutils-4.11.0/bin/xargs
awk        /nix/store/63g104xwbzfpcq0h7c99w19m6j9ly3p1-gawk-5.4.1/bin/awk
sed        /nix/store/1ggxndc1gd5061j5hqprhnhx9p30yilv-gnused-4.10/bin/sed
grep       /nix/store/3668q0d9zhc5n9l2pnyvpd44kz9bjsjm-gnugrep-3.12/bin/grep
date       /nix/store/2gfxiwls9hbgwdwcy43mprchwsq36mg6-coreutils-9.11/bin/date
mktemp     /nix/store/2gfxiwls9hbgwdwcy43mprchwsq36mg6-coreutils-9.11/bin/mktemp
3.53.3
curl 8.20.0
jq-1.8.2
version: 0.11.0
```

### The code measured

`main` is `778ccd5`. This branch adds only the brief (`6bb6b02`) and, in this PR, the report and its evidence:

```
$ git diff --stat main -- crates/ scripts/ .github/ Cargo.toml Cargo.lock flake.nix docs/decisions/
(no output)
```

### Case 1: inside one process

**Hypothesis:** `SharedLedger` (`Arc<tokio::sync::Mutex<Ledger>>`) serializes every `decide`, so two leases on one job can never both be granted.

The harness starts one `ledgerd` and makes 1,000 keys. It sends every key twice, back to back, so that the two requests for a key can race. `xargs -P 32` runs 32 clients at a time, each a separate `curl`. Every tenth key is a `Notify` job. Its effect prints `ledgerd: notify: <message>` on the server's stderr, which makes it the one effect in the demo that can be counted outside the journal. The other 900 are `sleep_then_record { ms: 1 }`. When every job's latest event is `Succeeded` or `Died`, the harness stops the server and counts from the journal.

```
$ E=docs/pipeline/reports/2026-10-14-05-evidence
$ nix develop -c bash -o pipefail -c './docs/pipeline/reports/2026-10-14-05-evidence/measure.sh one-process | sed "s|$PWD|…|g"' > $E/run-1-one-process.txt; echo "exit=$?"
exit=0
```

`run-1-one-process.txt`, complete:

```
journal: target/brief05/one-process.dl7ObQ/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.1s A started: pid 613494, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:41755
submitting 2000 requests for 1000 keys from 32 concurrent clients
every key is sent twice, back to back; every tenth key is a Notify job
all 2000 replies in after 3446 ms; by then 26 jobs had succeeded
queue drained 15699 ms after the first submission
== HTTP replies, by status code
   1000 200
   1000 201
== HTTP replies, by key
keys: 1000; keys whose replies all name the same job: 1000; keys created exactly once: 1000; distinct job ids: 1000
== events, by kind
+-----------+--------+
|   kind    | events |
+-----------+--------+
| Submitted |   1000 |
| Leased    |   1000 |
| Succeeded |   1000 |
+-----------+--------+
== Submitted events per idempotency key
+------------------+------+
| submitted_events | keys |
+------------------+------+
|                1 | 1000 |
+------------------+------+
== effect intents per (job, attempt)
+---------+--------------+
| intents | job_attempts |
+---------+--------------+
|       1 |         1000 |
+---------+--------------+
== per job: leases, intents, outcomes and Succeeded events
+--------+---------+----------+-----------+------+
| leases | intents | outcomes | succeeded | jobs |
+--------+---------+----------+-----------+------+
|      1 |       1 |        1 |         1 | 1000 |
+--------+---------+----------+-----------+------+
== each Succeeded event's result against its own job's token or message
+------------------+------------+
| succeeded_events | own_result |
+------------------+------------+
|             1000 |       1000 |
+------------------+------------+
== did the executor run while clients were still submitting?
+-------------------------------+-------------+
| leases_before_last_submission | submissions |
+-------------------------------+-------------+
|                            26 |        1000 |
+-------------------------------+-------------+
== Notify effects performed, from A's stderr: times each message was sent
sent 1 time(s): 100 messages
== A's other stderr lines
(none)
```

**Result:** exactly 1 of each for every job. That holds for `Submitted` events per key, leases per job, intents per `(job, attempt)`, intents and outcomes per job, and `Succeeded` events per job. Every result is the job's own token or message, and each `Notify` message was sent once. Every key's two replies named one job: one `201` and one `200`. The executor leased 26 jobs while clients were still submitting. The other 974 drained after the last reply.

#### An effect that outlives its lease (a check the brief did not ask for)

A lock earns its keep when a lease expires while its holder is still working. This run makes that happen inside one process: job 0 sleeps 35 s under a 30 s lease, and job 1 is queued behind it.

```
$ nix develop -c bash -o pipefail -c './docs/pipeline/reports/2026-10-14-05-evidence/measure.sh long-lease | sed "s|$PWD|…|g"' > $E/run-1-long-lease.txt; echo "exit=$?"
exit=0
```

`run-1-long-lease.txt`, complete:

```
journal: target/brief05/long-lease.vwtj4o/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.0s A started: pid 626868, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:37847
+0.0s A: submit job 0, a 35 s sleep_then_record (the lease is 30 s), then job 1, a 1 ms one
A: {"job":0,"created":true} 201
A: {"job":1,"created":true} 201
+0.1s
  A sees job 0: Pending, attempts 0, effect NotStarted
  A sees job 1: Pending, attempts 0, effect NotStarted
  events:  Submitted(0) Submitted(1)
  effects: 
+0.3s
  A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
  A sees job 1: Pending, attempts 0, effect NotStarted
  events:  Submitted(0) Submitted(1) Leased(0,0,1)
  effects: (0,1)=none
+30.2s
  A sees job 0: Leased(lease 0, past its expiry), attempts 1, effect InFlight(1)
  A sees job 1: Pending, attempts 0, effect NotStarted
  events:  Submitted(0) Submitted(1) Leased(0,0,1)
  effects: (0,1)=none
+35.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Succeeded, attempts 1, effect Finished(1)
  events:  Submitted(0) Submitted(1) Leased(0,0,1) Succeeded(0) Leased(1,1,1) Succeeded(1)
  effects: (0,1)=long (1,1)=short
+38.4s quiet: nothing has changed for 3 s
== A's stderr
(empty)
== events
+-----+-----------+-----+--------------------+
| seq |   kind    | job |       detail       |
+-----+-----------+-----+--------------------+
|   1 | Submitted |   0 | key long           |
|   2 | Submitted |   1 | key short          |
|   3 | Leased    |   0 | lease 0, attempt 1 |
|   4 | Succeeded |   0 | result long        |
|   5 | Leased    |   1 | lease 1, attempt 1 |
|   6 | Succeeded |   1 | result short       |
+-----+-----------+-----+--------------------+
== effects
+-----+---------+-----------------------+---------+
| job | attempt | intent_ms_after_first | outcome |
+-----+---------+-----------------------+---------+
|   0 |       1 |                     0 | long    |
|   1 |       1 |                 35020 | short   |
+-----+---------+-----------------------+---------+
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      1 |       1 |        1 |         1 |
|   1 |         1 |      1 |       1 |        1 |         1 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      6 |         1 |        6 | ok              |
+--------+-----------+----------+-----------------+
```

From +30.2 s to +35.3 s, A reports job 0's lease as past its expiry. Nothing expired it: the journal has no `Retrying` event, one lease, one intent and one `Succeeded` event. Job 1, a 1 ms job, waited 35 s for the executor.

### Case 2: across a restart

Plan 04's crash test, run once, exactly as CI's `crash-recovery` job runs it. The harness's `restart` case runs this single cargo command:

```
$ nix develop -c bash -o pipefail -c './docs/pipeline/reports/2026-10-14-05-evidence/measure.sh restart | sed "s|$PWD|…|g"' > $E/run-2-restart.txt; echo "exit=$?"
exit=0
```

`run-2-restart.txt`, complete:

```
$ cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture
   Compiling bitflags v2.13.2
   Compiling getrandom v0.4.3
   Compiling rustix v1.1.5
   Compiling linux-raw-sys v0.12.1
   Compiling cfg-if v1.0.5
   Compiling fastrand v2.5.0
   Compiling rusqlite v0.37.0
   Compiling ledger-store v0.1.0 (…/crates/ledger-store)
   Compiling ledgerd v0.1.0 (…/crates/ledgerd)
   Compiling tempfile v3.27.0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.53s
     Running tests/crash_recovery.rs (target/debug/deps/crash_recovery-642c09cb09ca0989)

running 1 test
killed mid-effect: signal: 9 (SIGKILL)
ledgerd: recovered 1 leased job(s) after a restart
succeeded 3.064763324s after the kill: {"effect":{"Finished":{"attempt":2}},"job":{"attempts":2,"id":0,"idempotency_key":"crash","kind":{"SleepThenRecord":{"ms":2000,"token":"crash-test"}},"max_attempts":3,"state":{"Succeeded":{"result":"crash-test"}}}}
test a_job_killed_mid_effect_completes_exactly_once_after_restart ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.20s

```

The test asserts one `Succeeded` event in the journal and attempt 2 as the one that succeeded. Kill to success took 3.06 s; [report 04](2026-10-14-04-ledgerd.md) measured 3.07 s and 3.05 s for the same test. Report 04's "Inferred" section still applies: "exactly once" means one `Succeeded` event, and the idempotent effect itself ran twice.

### Case 3: two processes on one journal

The brief's steps, each run with a fresh journal:
1. Start `ledgerd` A.
2. Submit a 5 s `sleep_then_record` to A, and poll A until the job's effect is `InFlight`.
3. Start `ledgerd` B on the same `LEDGER_DB`. A and B are both given `LEDGER_ADDR=127.0.0.1:0`, so each binds a free port of its own, and the two bound addresses differ.
4. Sample both processes' view of the job, and the journal, every 200 ms. The run is quiet when nothing has changed for 3 s and both processes report the job finished.

```
$ for r in 1 2 3; do nix develop -c bash -o pipefail -c "./docs/pipeline/reports/2026-10-14-05-evidence/measure.sh two-processes $r | sed \"s|\$PWD|…|g\"" > $E/run-3-two-processes-$r.txt 2>/dev/null; echo "run $r exit=$?"; done
run 1 exit=0
run 2 exit=0
run 3 exit=0
```

`run-3-two-processes-1.txt`, complete:

```
journal: target/brief05/two-processes-different-addr-1.DXACiR/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.0s A started: pid 636739, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:44437
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29961 ms left; starting B on the same LEDGER_DB
+0.2s B started: pid 636808, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:34933
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Pending, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0)
  effects: (0,1)=abandoned
+1.4s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2)
  effects: (0,1)=abandoned (0,2)=none
+5.1s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0)
  effects: (0,1)=run-1 (0,2)=none
+6.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0)
  effects: (0,1)=run-1 (0,2)=run-1
+9.3s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: yes
== A's stderr
(empty)
== B's stderr
ledgerd: recovered 1 leased job(s) after a restart
== events
+-----+-----------+-----+-----------------------+
| seq |   kind    | job |        detail         |
+-----+-----------+-----+-----------------------+
|   1 | Submitted |   0 | key long              |
|   2 | Leased    |   0 | lease 0, attempt 1    |
|   3 | Retrying  |   0 | restarted mid-attempt |
|   4 | Leased    |   0 | lease 1, attempt 2    |
|   5 | Succeeded |   0 | result run-1          |
|   6 | Succeeded |   0 | result run-1          |
+-----+-----------+-----+-----------------------+
== effects
+-----+---------+-----------------------+---------+
| job | attempt | intent_ms_after_first | outcome |
+-----+---------+-----------------------+---------+
|   0 |       1 |                     0 | run-1   |
|   0 |       2 |                  1072 | run-1   |
+-----+---------+-----------------------+---------+
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      2 |       2 |        2 |         2 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      6 |         1 |        6 | ok              |
+--------+-----------+----------+-----------------+
```

`run-3-two-processes-2.txt`, without its `events` and `effects` tables, which match run 1's apart from the token (`run-2`):

```
journal: target/brief05/two-processes-different-addr-2.ox3AMK/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.0s A started: pid 638654, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:38025
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29968 ms left; starting B on the same LEDGER_DB
+0.2s B started: pid 638705, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:45271
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Pending, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0)
  effects: (0,1)=abandoned
+1.1s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2)
  effects: (0,1)=abandoned (0,2)=none
+5.1s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0)
  effects: (0,1)=run-2 (0,2)=none
+6.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0)
  effects: (0,1)=run-2 (0,2)=run-2
+9.4s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: yes
== A's stderr
(empty)
== B's stderr
ledgerd: recovered 1 leased job(s) after a restart
[…]
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      2 |       2 |        2 |         2 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      6 |         1 |        6 | ok              |
+--------+-----------+----------+-----------------+
```

`run-3-two-processes-3.txt`, likewise. This run's +5.1 s sample caught A between recording its outcome and appending its `Succeeded` event:

```
journal: target/brief05/two-processes-different-addr-3.qYoe3D/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
+0.0s A started: pid 640516, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:37943
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29968 ms left; starting B on the same LEDGER_DB
+0.2s B started: pid 640567, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:38497
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Pending, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0)
  effects: (0,1)=abandoned
+1.4s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2)
  effects: (0,1)=abandoned (0,2)=none
+5.1s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2)
  effects: (0,1)=run-3 (0,2)=none
+5.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: Leased(lease 1, live), attempts 2, effect InFlight(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0)
  effects: (0,1)=run-3 (0,2)=none
+6.3s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0)
  effects: (0,1)=run-3 (0,2)=run-3
+9.3s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: yes
== A's stderr
(empty)
== B's stderr
ledgerd: recovered 1 leased job(s) after a restart
[…]
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      2 |       2 |        2 |         2 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      6 |         1 |        6 | ok              |
+--------+-----------+----------+-----------------+
```

The brief's questions, answered from the three runs. The times are sample times from the timelines above:

| | run 1 | run 2 | run 3 |
|---|---|---|---|
| Does B start? | yes | yes | yes |
| Time left on A's lease when B started | 29,961 ms | 29,968 ms | 29,968 ms |
| B expires A's live lease (`Retrying`, "restarted mid-attempt", and A's attempt marked `abandoned`) | by +0.2 s | by +0.2 s | by +0.2 s |
| B re-drives the effect (`Leased(0,1,2)` and intent `(0,2)`) | +1.4 s | +1.1 s | +1.4 s |
| A appends `Succeeded` for lease 0, which the journal had already retried | +5.1 s | +5.1 s | +5.3 s |
| B appends `Succeeded` for lease 1 | +6.3 s | +6.3 s | +6.3 s |
| SQLite errors on either stderr | none | none | none |
| `integrity_check`; event `seq` | ok; 1 to 6 | ok; 1 to 6 | ok; 1 to 6 |
| Job 0's intents, outcomes, `Succeeded` events | 2, 2, 2 | 2, 2, 2 | 2, 2, 2 |

**Do the two processes' appends collide in the journal?** Not physically. SQLite took every write, the file passes `integrity_check`, and `seq` runs 1 to 6 with no gaps. They do collide logically:
- A completed a lease that the journal had already retried.
- A's outcome for attempt 1 overwrote the `abandoned` that B's recovery had recorded there.
- The journal ends with two `Succeeded` events for one job, which spec §4.3 lists as the first invariant.

The next extra run shows the collision becoming a clash of ids.

#### B is given A's own address (a check the brief did not ask for)

Double-starting a service with the same configuration is a common mistake. Here B gets A's bound address, so it cannot bind:

```
$ for v in same-addr both-submit; do nix develop -c bash -o pipefail -c "./docs/pipeline/reports/2026-10-14-05-evidence/measure.sh two-processes 1 $v | sed \"s|\$PWD|…|g\"" > $E/run-3-two-processes-$v.txt 2>/dev/null; echo "$v exit=$?"; done
same-addr exit=0
both-submit exit=0
```

`run-3-two-processes-same-addr.txt`, complete:

```
journal: target/brief05/two-processes-same-addr-1.QZgnXp/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.0s A started: pid 642949, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:37641
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
+0.1s A sees job 0: Leased(lease 0, live), attempts 1, effect InFlight(1)
+0.1s A's lease on job 0 has 29965 ms left; starting B on the same LEDGER_DB
+0.2s B did not start: it exited with status 1 before printing an address
+0.2s
  A sees job 0: Leased(lease 0, live), attempts 1, effect Finished(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1) Retrying(0)
  effects: (0,1)=abandoned
+5.1s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  B sees job 0: down
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Succeeded(0)
  effects: (0,1)=run-1
+8.2s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: no
== A's stderr
(empty)
== B's stderr
ledgerd: recovered 1 leased job(s) after a restart
Error: Os { code: 98, kind: AddrInUse, message: "Address already in use" }
== events
+-----+-----------+-----+-----------------------+
| seq |   kind    | job |        detail         |
+-----+-----------+-----+-----------------------+
|   1 | Submitted |   0 | key long              |
|   2 | Leased    |   0 | lease 0, attempt 1    |
|   3 | Retrying  |   0 | restarted mid-attempt |
|   4 | Succeeded |   0 | result run-1          |
+-----+-----------+-----+-----------------------+
== effects
+-----+---------+-----------------------+---------+
| job | attempt | intent_ms_after_first | outcome |
+-----+---------+-----------------------+---------+
|   0 |       1 |                     0 | run-1   |
+-----+---------+-----------------------+---------+
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      1 |       1 |        1 |         1 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|      4 |         1 |        4 | ok              |
+--------+-----------+----------+-----------------+
```

B never served a request, yet the journal holds its `Retrying` event, for a retry that never ran. A's running attempt was also marked `abandoned` until A overwrote that. The job still ended with one of each, because B exited before its executor started.

#### Both processes take a new job (a check the brief did not ask for)

The same steps as the brief's runs. When they go quiet, one new job is submitted to A and one to B, each with its own key. The part before that matches runs 1 to 3 and is left out:

```
journal: target/brief05/two-processes-both-submit-1.BVghmP/ledger.db
$ cargo build -p ledgerd
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.04s
+0.0s A started: pid 644377, LEDGER_ADDR=127.0.0.1:0, stdout: ledgerd listening on 127.0.0.1:46053
+0.0s A: submit job 0, a 5 s sleep_then_record
A: {"job":0,"created":true} 201
[…]
+9.3s quiet: nothing has changed for 3 s
+9.3s submit one new job to each process
A: {"job":1,"created":true} 201
B: {"job":1,"created":true} 201
+9.4s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Pending, attempts 0, effect NotStarted
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  B sees job 1: Leased(lease 2, live), attempts 1, effect InFlight(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0) Submitted(1) Submitted(1) Leased(1,2,1)
  effects: (0,1)=run-1 (0,2)=run-1 (1,1)=none
+9.7s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Leased(lease 1, live), attempts 1, effect InFlight(1)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  B sees job 1: Leased(lease 2, live), attempts 1, effect InFlight(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0) Submitted(1) Submitted(1) Leased(1,2,1) Leased(1,1,1)
  effects: (0,1)=run-1 (0,2)=run-1 (1,1)=none
+10.0s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Leased(lease 1, live), attempts 1, effect Finished(1)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  B sees job 1: Succeeded, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0) Submitted(1) Submitted(1) Leased(1,2,1) Leased(1,1,1) Succeeded(1)
  effects: (0,1)=run-1 (0,2)=run-1 (1,1)=from-b
+39.6s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Pending, attempts 1, effect Finished(1)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  B sees job 1: Succeeded, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0) Submitted(1) Submitted(1) Leased(1,2,1) Leased(1,1,1) Succeeded(1) Retrying(1)
  effects: (0,1)=run-1 (0,2)=run-1 (1,1)=from-b
+40.7s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Leased(lease 2, live), attempts 2, effect InFlight(2)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  B sees job 1: Succeeded, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0) Submitted(1) Submitted(1) Leased(1,2,1) Leased(1,1,1) Succeeded(1) Retrying(1) Leased(1,2,2)
  effects: (0,1)=run-1 (0,2)=run-1 (1,1)=from-b (1,2)=none
+41.2s
  A sees job 0: Succeeded, attempts 1, effect Finished(1)
  A sees job 1: Succeeded, attempts 2, effect Finished(2)
  B sees job 0: Succeeded, attempts 2, effect Finished(2)
  B sees job 1: Succeeded, attempts 1, effect Finished(1)
  events:  Submitted(0) Leased(0,0,1) Retrying(0) Leased(0,1,2) Succeeded(0) Succeeded(0) Submitted(1) Submitted(1) Leased(1,2,1) Leased(1,1,1) Succeeded(1) Retrying(1) Leased(1,2,2) Succeeded(1)
  effects: (0,1)=run-1 (0,2)=run-1 (1,1)=from-b (1,2)=from-a
+44.3s quiet: nothing has changed for 3 s
== still running at the end? A: yes, B: yes
== A's stderr
ledgerd: executor: sqlite: UNIQUE constraint failed: effects.job, effects.attempt
== B's stderr
ledgerd: recovered 1 leased job(s) after a restart
== events
+-----+-----------+-----+-----------------------+
| seq |   kind    | job |        detail         |
+-----+-----------+-----+-----------------------+
|   1 | Submitted |   0 | key long              |
|   2 | Leased    |   0 | lease 0, attempt 1    |
|   3 | Retrying  |   0 | restarted mid-attempt |
|   4 | Leased    |   0 | lease 1, attempt 2    |
|   5 | Succeeded |   0 | result run-1          |
|   6 | Succeeded |   0 | result run-1          |
|   7 | Submitted |   1 | key to-a              |
|   8 | Submitted |   1 | key to-b              |
|   9 | Leased    |   1 | lease 2, attempt 1    |
|  10 | Leased    |   1 | lease 1, attempt 1    |
|  11 | Succeeded |   1 | result from-b         |
|  12 | Retrying  |   1 | lease expired         |
|  13 | Leased    |   1 | lease 2, attempt 2    |
|  14 | Succeeded |   1 | result from-a         |
+-----+-----------+-----+-----------------------+
== effects
+-----+---------+-----------------------+---------+
| job | attempt | intent_ms_after_first | outcome |
+-----+---------+-----------------------+---------+
|   0 |       1 |                     0 | run-1   |
|   0 |       2 |                  1069 | run-1   |
|   1 |       1 |                  9329 | from-b  |
|   1 |       2 |                 40476 | from-a  |
+-----+---------+-----------------------+---------+
== per job
+-----+-----------+--------+---------+----------+-----------+
| job | submitted | leases | intents | outcomes | succeeded |
+-----+-----------+--------+---------+----------+-----------+
|   0 |         1 |      2 |       2 |        2 |         2 |
|   1 |         2 |      3 |       2 |        2 |         2 |
+-----+-----------+--------+---------+----------+-----------+
== the file
+--------+-----------+----------+-----------------+
| events | first_seq | last_seq | integrity_check |
+--------+-----------+----------+-----------------+
|     14 |         1 |       14 | ok              |
+--------+-----------+----------+-----------------+
```

Each process gave its new job the id it had counted to itself:
- Two different jobs were journaled as job 1 (`seq` 7 and 8).
- Lease id 2 was issued twice: by B at `seq` 9, and by A at `seq` 13.
- B recorded intent `(1,1)` first. A's own attempt 1 then failed in the store with `UNIQUE constraint failed: effects.job, effects.attempt`, and A's job sat leased until its lease expired 30 s later.
- The journal ends with two `Succeeded` events for job 1, carrying different results (`from-b`, `from-a`).

### Case 4: what supplies `now` to a lease decision

```
$ nix develop -c bash -o pipefail -c './docs/pipeline/reports/2026-10-14-05-evidence/measure.sh clock | sed "s|$PWD|…|g"' > $E/run-4-clock.txt 2>/dev/null; echo "exit=$?"
exit=0
```

`run-4-clock.txt`, complete:

```
$ grep -rnE 'SystemTime|Instant' crates/*/src
crates/ledgerd/src/lib.rs:9:use std::time::{Duration, SystemTime, UNIX_EPOCH};
crates/ledgerd/src/lib.rs:32:    let since_epoch = SystemTime::now()
$ grep -rnE 'now\(\)' crates/*/src
crates/ledgerd/src/lib.rs:31:pub fn now() -> Millis {
crates/ledgerd/src/lib.rs:32:    let since_epoch = SystemTime::now()
crates/ledgerd/src/lib.rs:257:        let next = ledger.lock().await.next_work(now());
crates/ledgerd/src/lib.rs:260:                if let Err(e) = ledger.lock().await.start_effect(&work, now()) {
crates/ledgerd/src/main.rs:17:    let ledger = Arc::new(Mutex::new(ledgerd::Ledger::open(&db, ledgerd::now())?));
$ grep -rnE 'Command::(Lease|ExpireLeases|Fail) \{' crates/*/src
crates/ledger-core/src/lib.rs:41:        Command::Lease { now, lease_for_ms } => Ok(state
crates/ledger-core/src/lib.rs:55:        Command::Fail {
crates/ledger-core/src/lib.rs:64:        Command::ExpireLeases { now } => Ok(state
crates/ledgerd/src/lib.rs:193:        self.execute(Command::ExpireLeases { now })?;
crates/ledgerd/src/lib.rs:194:        let events = self.execute(Command::Lease {
$ grep -rnE 'recover\(|Ledger::open\(|next_work\(' crates/*/src crates/*/tests
crates/ledger-core/src/lib.rs:88:pub fn recover(
crates/ledgerd/src/lib.rs:115:        let events = recover(&self.state, now, |job, attempt| {
crates/ledgerd/src/lib.rs:192:    pub fn next_work(&mut self, now: Millis) -> Result<Option<Work>, LedgerError> {
crates/ledgerd/src/lib.rs:257:        let next = ledger.lock().await.next_work(now());
crates/ledgerd/src/main.rs:17:    let ledger = Arc::new(Mutex::new(ledgerd::Ledger::open(&db, ledgerd::now())?));
crates/ledger-core/tests/recovery.rs:49:    let events = recover(&state, Millis(100), |_, _| {
crates/ledger-core/tests/recovery.rs:64:    let events = recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished));
crates/ledger-core/tests/recovery.rs:78:    let events = recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished));
crates/ledger-core/tests/recovery.rs:88:    let events = recover(&state, Millis(100), |_, _| None);
crates/ledger-core/tests/recovery.rs:98:    let events = recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished));
crates/ledger-core/tests/recovery.rs:117:        recover(&state, Millis(100), |_, _| Some(EffectRecord::Unfinished)),
$ grep -rnE 'expires|not_before' crates/ledger-core/src
crates/ledger-core/src/state.rs:41:                JobState::Pending { not_before } if not_before <= now => Some((not_before, job)),
crates/ledger-core/src/state.rs:44:            .min_by_key(|(not_before, job)| (*not_before, job.id))
crates/ledger-core/src/state.rs:50:            move |job| matches!(job.state, JobState::Leased { expires, .. } if expires <= now),
crates/ledger-core/src/state.rs:71:                            not_before: Millis(0),
crates/ledger-core/src/state.rs:82:                expires,
crates/ledger-core/src/state.rs:88:                        expires: *expires,
crates/ledger-core/src/state.rs:102:                job, not_before, ..
crates/ledger-core/src/state.rs:107:                        not_before: *not_before,
crates/ledger-core/src/lib.rs:47:                expires: now.plus(lease_for_ms),
crates/ledger-core/src/lib.rs:140:            not_before: now.plus(backoff_ms(job.attempts)),
$ grep -nE '\.route\(' crates/ledgerd/src/lib.rs
280:        .route("/jobs", post(submit))
281:        .route("/jobs/{id}", get(status))
$ grep -n -A5 '^pub struct SubmitJob' crates/ledger-api/src/lib.rs
163:pub struct SubmitJob {
164-    pub idempotency_key: String,
165-    pub kind: JobKind,
166-    pub max_attempts: u32,
167-}
168-
$ ./scripts/ratchets/core-purity.sh .
core-purity: ok
```

The reasoning drawn from it is under "Inferred", as the brief asks.

### The gates, in CI's order, and shellcheck on the harness

The CI order, captured to `target/brief05-gates.txt`. The first command adds `measure.sh` to CI's shellcheck list; CI itself checks only `scripts/ratchets/`:

```
$ nix develop -c bash -o pipefail -c '
for c in "shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh docs/pipeline/reports/2026-10-14-05-evidence/measure.sh" \
         "./scripts/ratchets/all.sh" \
         "./scripts/ratchets/test-ratchets.sh" \
         "cargo fmt --all -- --check" \
         "cargo clippy --workspace --all-targets -- -D warnings" \
         "cargo test --workspace"; do
  echo "\$ $c"
  eval "$c" 2>&1; echo "exit $?"; echo
done | sed "s|$PWD|…|g"' > target/brief05-gates.txt 2>/dev/null; echo "exit=$?"
exit=0
```

From `target/brief05-gates.txt`:

```
$ shellcheck scripts/ratchets/*.sh scripts/ratchets/fixtures/*.sh scripts/ratchets/fixtures/*/*.sh docs/pipeline/reports/2026-10-14-05-evidence/measure.sh
exit 0

$ ./scripts/ratchets/all.sh
core-line-budget: ok (234 of 250 lines)
core-purity: ok
no-dev-paths: ok
sql-containment: ok
wire-types-in-api: ok
all 5 ratchets hold
exit 0

$ ./scripts/ratchets/test-ratchets.sh
[…]
all 5 ratchets can fail, and no comment trips one
exit 0

$ cargo fmt --all -- --check
exit 0

$ cargo clippy --workspace --all-targets -- -D warnings
[…]
    Checking ledgerd v0.1.0 (…/crates/ledgerd)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.23s
exit 0

$ cargo test --workspace
[…]
    Finished `test` profile [unoptimized + debuginfo] target(s) in 5.52s
[…]
     Running tests/crash_recovery.rs (target/debug/deps/crash_recovery-a38ce57da4a8b6c1)

running 1 test
test a_job_killed_mid_effect_completes_exactly_once_after_restart ... ignored, spawns and kills the real server; run by CI's crash-recovery job

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
[…]
exit 0
```

All 17 test binaries passed, and the capture contains no failure, warning or error line:

```
$ grep -c '^test result: ok' target/brief05-gates.txt; grep -c '^test result' target/brief05-gates.txt; grep -n 'FAILED\|^warning\|^error' target/brief05-gates.txt; echo "grep exit=$?"
17
17
grep exit=1
```

No ratchet flagged the harness. The ratchets scan only `crates/`, and the harness is under `docs/`.

### Commit identity

```
$ git config user.email
26385844+ioitiki@users.noreply.github.com
```

## Inferred (not proven by anything above)

**Case 1: why one process is safe.** From the code:
- Every HTTP handler and every executor step locks the one `Mutex<Ledger>` for one `Ledger` call (`crates/ledgerd/src/lib.rs:28`, `:255-276`, `:285-304`). A submission checks its key and appends inside one hold, so two requests with one key cannot both create a job.
- The executor is one loop: lease, intent, perform, outcome, complete.
- `ExpireLeases` is issued only by `next_work` (`lib.rs:192-215`), and only that same loop calls `next_work`, between jobs. Inside one process, the task holding a lease is the only task that can expire it. That is why the long-lease run's lease sat past its expiry, untouched.
- An in-process lease expiry needs the executor to have given up on a job first, after a store error in `start_effect` or `finish_effect` (`lib.rs:260-267`). The both-submit run shows the `start_effect` case.

**Case 1: what the run did and did not exercise.**
- Submissions raced each other heavily: 2,000 requests in 3.4 s, 32 at a time, with every duplicate pair sent back to back. Submissions and the executor raced far less: 26 of the 1,000 leases fell between submissions. That ordering does not matter under the mutex, but the run proves it for only those 26.
- "Intents per `(job, attempt)`" can never exceed 1. The schema makes `(job, attempt)` the primary key (`crates/ledger-store/src/schema.sql:18`), and the both-submit run shows SQLite refusing a second intent. Intents per job is the count that could have shown a re-run.

**Case 3: why it goes wrong.**
- B's `Ledger::open` replays the journal and calls `recover`, which treats every `Leased` job as abandoned (`crates/ledger-core/src/lib.rs:85-115`). That is the design: nothing in the journal says which process holds a lease, or whether that process is alive. So B cannot tell A's live lease from a dead one.
- Each process reads the events table once, at open (`ledgerd/src/lib.rs:84-93`). After that it decides against its own memory. A's `Complete` passed `holding` (`ledger-core/src/lib.rs:118-124`) because A's memory still said lease 0 was current.
- The `effects` table, by contrast, is read live by `GET /jobs/{id}` (`ledgerd/src/lib.rs:167-189`). That is why A reported `effect Finished(1)` while its own effect was still running.
- The timings (5 s effect, 1 s backoff) made A finish first in every run. If B finished first, the count would be the same. Each process completes against its own memory, so both append `Succeeded` either way.
- No `NotIdempotent` job was in flight in any run: `Notify` has no duration to be in flight during. From the code, B's recovery would append `Died` and A would still append `Succeeded`, and replay keeps the last event, so the job would read as succeeded.

**Would a distributed lock have stopped case 3?** Only if B's recovery asked the lock before expiring a lease. That needs the same fact a journal lock gives for free: whether another live process holds the journal. A lock with a TTL also reads a clock, and it still needs the holder's writes checked against a fencing token where they land. The ledger has fencing tokens already (lease ids). Case 3 shows them being checked against stale memory instead of the journal. Multiple nodes are out of scope (§10), so none of this needs to be built.

**Case 4: nothing but `ledgerd::now` supplies `now` to a lease decision.** The reasoning, from `run-4-clock.txt` and ADR 0002:
- **One clock read.** The only clock read in any crate's source is `SystemTime::now()` inside `ledgerd::now` (`ledgerd/src/lib.rs:31-36`). The core reads none: `core-purity` holds, and ADR 0002 makes `now` an argument to every time-dependent command.
- **Who passes `now` in.** The lease decisions are `Command::Lease` and `Command::ExpireLeases`, and `ledgerd` builds them only in `next_work` (`lib.rs:193-194`). Its only caller is the executor, with `now()` (`lib.rs:257`). `recover` gets `now` from `Ledger::open`, whose only caller is `main.rs:17`, with `ledgerd::now()`. `ledgerd` never builds `Command::Fail`.
- **The HTTP API carries no time.** It has two routes, and `SubmitJob` holds a key, a kind and `max_attempts`.
- **Clock disagreement across a restart cannot change a lease decision.** `recover` never compares a lease's `expires` with `now`; it ends every lease it finds (`ledger-core/src/lib.rs:95`). There, `now` sets only the retry's `not_before`.
- **Times read back from the journal.** These are `expires` and `not_before`, written earlier by the same node's clock. They are compared with `now` only in `next_due` and `expired_leases` (`ledger-core/src/state.rs:41`, `:50`).
- **A wall clock, not a monotonic one.** `SystemTime` can step:
  - Forward: `expired_leases` comes true sooner. Inside one process that is checked only between jobs, so a step cannot hand a running job to a second holder.
  - Backward: leases and retries wait longer. That costs liveness, not safety.

  Neither is an argument for a lock, and a TTL lock would read a clock itself.
- **A door left open.** `Ledger::open` and `next_work` are `pub` and accept any `now`. Nothing in the repository passes anything but `ledgerd::now()`; a second caller added later could.

**What no run here covers:**
- more than one host, or a journal on a network filesystem
- power loss (report 04's caveat stands)
- a `NotIdempotent` effect in flight when B starts
- what a later replay makes of the both-submit journal. From `State::apply`: the second `Submitted` event replaces job 1's kind with B's, both keys map to job 1, and the job reads as succeeded with A's result (`from-a`).
- the same timings on CI's `ubuntu-latest`. This harness is not part of CI.

## Recommendation

- **Build no distributed lock.** No Redis, etcd or ZooKeeper, and no Postgres advisory lock behind leases. Every in-scope case measured exactly once, and the one double needs two processes on one journal. A per-lease lock would not stop that unless recovery consulted it, which comes back to knowing whether another process holds the journal.
- **Add no per-lease locking inside `ledgerd` either.** The mutex and the single executor loop already give one holder per lease (case 1, and the long-lease run).
- **Decide whether to make "one `ledgerd` per journal" enforced rather than assumed (finding 1).** If yes, it should be one small build brief with this acceptance test: start A, start B on the same `LEDGER_DB`, and require B to exit non-zero having written nothing. That means no new event and no outcome rewritten. This harness's case 3 then serves as the regression check: the job should end with one intent, one outcome and one `Succeeded` event.
  - The mechanism is the plan's to choose. Two candidates: an OS advisory lock (`flock`) on the journal or on a lock file beside it, held for the process's life; or SQLite's `PRAGMA locking_mode = EXCLUSIVE` with a write at open.
  - Either one is local to the host. Neither is reliable on a network filesystem (inferred).
- **For spec §6, required evidence 1:** for the question asked, this investigation recommends not building. It does not claim the design needs nothing. Finding 1 is a measured gap, and closing it is a separate build decision for the operator.

## Findings for the operator

1. **Nothing enforces one `ledgerd` per journal.**
   - *Measured:* case 3, all 3 runs. B started on A's live journal, expired A's lease, re-ran the effect, and left two `Succeeded` events.
   - *Proposed next step:* the build brief under "Recommendation": an exclusive lock on the journal, taken before replay, so that a second process exits having written nothing.
2. **A second process that fails to start has already written to the journal.**
   - *Measured:* the same-addr run. `main.rs` opens the ledger (replay, then recovery) at line 17, before it binds at line 18. B exited with `Address already in use`, after appending `Retrying` for A's running job and marking A's attempt `abandoned`.
   - *Proposed next step:* take finding 1's lock first, which closes this too. Whether to bind before replaying as well is the plan's call.
3. **`record_outcome` overwrites an outcome that is already recorded.**
   - *Measured:* every case-3 run. A's outcome replaced recovery's `abandoned` on `(0,1)`. The `UPDATE` has no `outcome IS NULL` condition (`ledger-store/src/lib.rs:127-136`).
   - Inside one process it never fires: each attempt records one outcome, and recovery marks only unfinished intents.
   - *Proposed next step:* a store test, and a refusal when the attempt already has an outcome. A stale writer then gets an error instead of rewriting history.
4. **Each process checks leases against its own memory, not the journal.**
   - *Measured:* case 3, where A's stale lease 0 completed. Also the both-submit run, where both processes allocated job id 1 and lease id 2.
   - *Proposed next step:* none while finding 1's lock holds. If more than one writer ever enters scope, that check has to move into the append transaction, a compare-and-append against the journal. Record it as a known limit of the design, not a lock to add.
5. **A lease that expires inside one process is retried without consulting its effect record.**
   - *Inferred* for the dangerous case. Suppose `finish_effect` fails in the store after the effect ran (`ledgerd/src/lib.rs:265-267`). The job stays leased, and 30 s later `ExpireLeases` retries it through `after_failed_attempt` (`ledger-core/src/lib.rs:64-67`, `:128-144`). Nothing looks at the intent or at `EffectSemantics`. A `NotIdempotent` effect would run again, which ADR 0003 forbids on restart.
   - *Measured* for the harmless case: in the both-submit run, a failed `start_effect` was retried on expiry. No effect had started, so the retry was correct.
   - *Proposed next step:* a core example: an expired lease whose attempt has an unfinished intent, on a `NotIdempotent` kind, should die, not retry. Then decide whether expiry goes through the same lookup as `recover`.
6. **One slow effect stops the whole queue, and lease expiry cannot help.**
   - *Measured:* the long-lease run. A 1 ms job waited 35 s behind a 35 s job, whose lease sat 5 s past expiry. The executor running the effect is the only thing that expires leases. A hung effect would stall every job indefinitely.
   - This is the other face of case 1's safety.
   - *Proposed next step:* decide whether effects get a deadline. If they do, test what happens to an effect that outlives it, because dropping a future does not stop work it already started outside the process.

## Deviations from the brief

1. **How `ledgerd` is started.** The harness builds with `cargo build -p ledgerd` and then runs the binary cargo built, `target/debug/ledgerd`, rather than `cargo run`. The PID it waits on and signals is then `ledgerd`'s own.
2. **B's `LEDGER_ADDR`.** In the brief's three runs, A and B were both given the string `127.0.0.1:0`. Each bound a different free port, as their first stdout lines show. A literally different string was used only in the same-addr run, where B is given A's bound address.
3. **Checks the brief did not ask for**, each run once and labeled where it appears:
   - the long-lease run
   - the same-addr run
   - the both-submit run
   - the `Notify` count from stderr in case 1
4. **Tools outside the dev shell:**
   - curl, inside the harness, from the host, as the brief allows.
   - `git` and `gh`, on the host, outside the harness: the diff, status, commit, push and PR.
   - `grep`, on the host, to count the test results in `target/brief05-gates.txt`.
   - `awk`, inside the dev shell, to splice the run files into this report from its draft under `target/`.
   - One `python3 -I` call, on the host, which made one text replacement in `measure.sh` while I was writing it, before any run.
5. **The harness never deletes anything.** Its first draft cleared each run's directory with `rm -rf`. Claude Code's safety check refused to run it. Each run now gets a new directory from `mktemp -d` under `target/brief05/`, and the old ones stay there.
6. **Smoke runs.** Before the recorded runs, I ran smaller versions under `target/brief05-smoke/`, in this order: `clock`, `one-process 20 4`, one two-process run, and one same-addr run. They are not evidence. The script changed twice while they ran:
   - deviation 5's change, before `one-process 20 4`
   - before the same-addr run, changes to what it prints, and nothing else: times count from A's start, process names print as `A` and `B`, and A's replies are prefixed `A:`

   It did not change after the last smoke run, so every `run-*.txt` file came from the final script. The `clock` smoke's output is identical to `run-4-clock.txt`.
7. **This PR's CI.** The report is committed before the push, so CI's run on this branch is not quoted here. The coordinator verifies it before merging.

**Clean-room:** every file this worker read or wrote is inside this worktree:
- the evidence directory and this report
- `target/brief05/` (journals and server logs)
- `target/brief05-smoke/`
- `target/brief05-gates.txt` and `target/brief05-tools.txt`
- this report's draft and the `awk` script that spliced it, both under `target/`

The harness saved one long command output to its own directory, outside the worktree; I did not read it back from there. The tools went outside the worktree in their usual way:
- Nix used its store, and cargo its registry cache.
- `test-ratchets.sh` made scratch copies with `mktemp -d`, and the crash test made its journal with `tempfile::tempdir()`. Both are under the system temp directory.
- `gh` reads and writes this repository's PR.
