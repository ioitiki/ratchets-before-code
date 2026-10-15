#!/usr/bin/env bash
# The harness for brief 05: do leases need a distributed lock?
#
# It measures ledgerd as it is and changes nothing under crates/. It builds
# ledgerd with cargo, runs the binary cargo built against a fresh journal
# under target/brief05/, drives it over HTTP with curl, and reads the journal
# with sqlite3, using the schema in crates/ledger-store/src/schema.sql. Every
# sqlite3 call opens the journal read-only, so the harness never writes to it.
#
# Run it inside the dev shell:
#   measure.sh one-process [JOBS] [CLIENTS]  case 1: concurrent clients, one process
#   measure.sh long-lease                    case 1, extra: an effect outlives its lease
#   measure.sh restart                       case 2: plan 04's crash test, once
#   measure.sh two-processes RUN [VARIANT]   case 3: two processes on one journal
#       different-addr  the brief's steps (the default)
#       same-addr       extra: B is given A's own address, so it cannot bind
#       both-submit     extra: afterwards, one new job is submitted to each
#   measure.sh clock                         case 4: what supplies `now` to a lease
#   measure.sh submit-one KEY                internal: one client, run by xargs
#
# Everything goes to stdout. In the timelines, events read Kind(job) and
# Leased(job,lease,attempt), and effects read (job,attempt)=outcome, where
# "none" means the intent is recorded and the outcome is not.

set -euo pipefail
exec 2>&1

ROOT=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$ROOT"
BIN=target/debug/ledgerd
WORK=target/brief05
QUIET_MS=3000

declare -A PID=() ADDR=()
STARTED=()
DIR=
DB=
T0=$(date +%s%3N)

say() { printf '%s\n' "$*"; }
ms_now() { date +%s%3N; }

# since: the time since T0, as +S.Ds.
since() {
  local ms=$(($(ms_now) - T0))
  printf '+%d.%ds' $((ms / 1000)) $((ms % 1000 / 100))
}

# show CMD: print a command, then run it.
show() {
  say "\$ $1"
  eval "$1" || say "(exit $?)"
}

# fresh NAME: a new, empty directory for one run's journal and server logs.
fresh() {
  mkdir -p "$WORK"
  DIR=$(mktemp -d "$WORK/$1.XXXXXX")
  DB=$DIR/ledger.db
  say "journal: $DB"
}

build() {
  say "\$ cargo build -p ledgerd"
  cargo build -p ledgerd
}

cleanup() {
  local name
  for name in "${!PID[@]}"; do
    kill -KILL "${PID[$name]}" 2>/dev/null || true
  done
}
trap cleanup EXIT

# start NAME LEDGER_ADDR: run ledgerd on $DB in the background, and wait for
# the address it prints. Returns 1, saying why, if it exits first.
start() {
  local name=$1 addr=$2 deadline status
  STARTED+=("$name")
  LEDGER_DB=$DB LEDGER_ADDR=$addr "$BIN" >"$DIR/$name.out" 2>"$DIR/$name.err" &
  PID[$name]=$!
  deadline=$(($(ms_now) + 10000))
  until [[ -s "$DIR/$name.out" ]]; do
    if ! kill -0 "${PID[$name]}" 2>/dev/null; then
      status=0
      wait "${PID[$name]}" || status=$?
      unset "PID[$name]"
      say "$(since) ${name^^} did not start: it exited with status $status before printing an address"
      return 1
    fi
    if (($(ms_now) > deadline)); then
      say "$(since) ${name^^} printed no address within 10 s"
      return 1
    fi
    sleep 0.05
  done
  ADDR[$name]=$(head -n 1 "$DIR/$name.out")
  say "$(since) ${name^^} started: pid ${PID[$name]}, LEDGER_ADDR=$addr, stdout: ${ADDR[$name]}"
  ADDR[$name]=${ADDR[$name]##* }
}

# stop NAME: SIGTERM, then reap it.
stop() {
  local name=$1
  [[ -n "${PID[$name]:-}" ]] || return 0
  kill -TERM "${PID[$name]}" 2>/dev/null || true
  { wait "${PID[$name]}"; } 2>/dev/null || true
  unset "PID[$name]"
}

alive() {
  [[ -n "${PID[$1]:-}" ]] && kill -0 "${PID[$1]}" 2>/dev/null
}

# post NAME BODY: submit a job to process NAME. Prints the reply and its code.
post() {
  curl -sS --max-time 5 -w ' %{http_code}\n' -H 'Content-Type: application/json' \
    --data "$2" "http://${ADDR[$1]}/jobs"
}

# view NAME JOB: one job's status as process NAME reports it, in one line.
view() {
  local name=$1 job=$2 reply code
  if ! alive "$name"; then
    say "down"
    return
  fi
  if ! reply=$(curl -sS --max-time 2 -w '\n%{http_code}' "http://${ADDR[$name]}/jobs/$job" 2>/dev/null); then
    say "no reply"
    return
  fi
  code=${reply##*$'\n'}
  reply=${reply%$'\n'*}
  case $code in
    200) jq -r --argjson now "$(ms_now)" '
           def effect: if type == "string" then . else (to_entries[0] | "\(.key)(\(.value.attempt))") end;
           def state: to_entries[0]
             | if .key == "Leased"
               then "Leased(lease \(.value.lease), \(if .value.expires > $now then "live" else "past its expiry" end))"
               else .key end;
           "\(.job.state | state), attempts \(.job.attempts), effect \(.effect | effect)"' <<<"$reply" ;;
    404) say "unknown job" ;;
    *) say "HTTP $code" ;;
  esac
}

# query MODE SQL: run SQL against the journal, read-only. The temp table e
# has one row per event: seq, kind, job, and the event's fields as JSON in v.
query() {
  sqlite3 -readonly -batch "$1" "$DB" <<SQL
CREATE TEMP TABLE e AS
  SELECT seq, j.key AS kind, json_extract(j.value, '\$.job') AS job, j.value AS v
  FROM events, json_each(events.body) AS j;
$2
SQL
}
table() { query -table "$1"; }
value() { query -list "$1"; }

# The journal in one line each: its events, and its effects.
EVENTS_LINE="SELECT coalesce(group_concat(kind || '(' || job || CASE kind
    WHEN 'Leased' THEN ',' || json_extract(v, '\$.lease') || ',' || json_extract(v, '\$.attempt')
    ELSE '' END || ')', ' ' ORDER BY seq), '') FROM e;"
EFFECTS_LINE="SELECT coalesce(group_concat('(' || job || ',' || attempt || ')=' || CASE
    WHEN outcome IS NULL THEN 'none'
    WHEN outcome LIKE 'abandoned%' THEN 'abandoned'
    ELSE outcome END, ' ' ORDER BY job, attempt), '') FROM effects;"

# watch LIMIT_S JOB...: every 200 ms, sample each started process's view of
# each job, and the journal. Print the sample whenever it changes. Return once
# nothing has changed for QUIET_MS and every view is finished (Succeeded or
# Dead), unknown or down; or after LIMIT_S.
watch() {
  local limit=$1 sample last='' changed_at now name job v settled deadline
  shift
  deadline=$(($(ms_now) + limit * 1000))
  changed_at=$(ms_now)
  while :; do
    sample=''
    settled=1
    for name in "${STARTED[@]}"; do
      for job in "$@"; do
        v=$(view "$name" "$job")
        sample+="  ${name^^} sees job $job: $v"$'\n'
        case $v in Succeeded* | Dead* | down | "unknown job") ;; *) settled=0 ;; esac
      done
    done
    sample+="  events:  $(value "$EVENTS_LINE")"$'\n'
    sample+="  effects: $(value "$EFFECTS_LINE")"
    now=$(ms_now)
    if [[ "$sample" != "$last" ]]; then
      say "$(since)"
      say "$sample"
      last=$sample
      changed_at=$now
    elif ((settled && now - changed_at >= QUIET_MS)); then
      say "$(since) quiet: nothing has changed for $((QUIET_MS / 1000)) s"
      return 0
    fi
    if ((now > deadline)); then
      say "$(since) not quiet after $limit s; going on"
      return 0
    fi
    sleep 0.2
  done
}

# wait_for NAME JOB PATTERN LIMIT_S: poll until NAME's view of JOB matches.
wait_for() {
  local deadline v
  deadline=$(($(ms_now) + $4 * 1000))
  while :; do
    v=$(view "$1" "$2")
    # shellcheck disable=SC2053 # $3 is a glob pattern on purpose
    if [[ $v == $3 ]]; then
      say "$(since) ${1^^} sees job $2: $v"
      return 0
    fi
    if (($(ms_now) > deadline)); then
      say "$(since) gave up waiting; ${1^^} sees job $2: $v"
      return 1
    fi
    sleep 0.05
  done
}

# The journal at the end of a run: every event, every effect, and per job.
journal_report() {
  say "== events"
  table "SELECT seq, kind, job, CASE kind
      WHEN 'Submitted' THEN 'key ' || json_extract(v, '\$.idempotency_key')
      WHEN 'Leased' THEN 'lease ' || json_extract(v, '\$.lease') || ', attempt ' || json_extract(v, '\$.attempt')
      WHEN 'Succeeded' THEN 'result ' || json_extract(v, '\$.result')
      ELSE json_extract(v, '\$.reason') END AS detail
    FROM e ORDER BY seq;"
  say "== effects"
  table "SELECT job, attempt, intent_at - (SELECT min(intent_at) FROM effects) AS intent_ms_after_first,
      coalesce(outcome, 'NULL') AS outcome
    FROM effects ORDER BY job, attempt;"
  say "== per job"
  table "SELECT s.job,
      (SELECT count(*) FROM e x WHERE x.kind = 'Submitted' AND x.job = s.job) AS submitted,
      (SELECT count(*) FROM e x WHERE x.kind = 'Leased' AND x.job = s.job) AS leases,
      (SELECT count(*) FROM effects f WHERE f.job = s.job) AS intents,
      (SELECT count(outcome) FROM effects f WHERE f.job = s.job) AS outcomes,
      (SELECT count(*) FROM e x WHERE x.kind = 'Succeeded' AND x.job = s.job) AS succeeded
    FROM (SELECT DISTINCT job FROM e WHERE kind = 'Submitted') s ORDER BY s.job;"
  say "== the file"
  table "SELECT count(*) AS events, min(seq) AS first_seq, max(seq) AS last_seq,
      (SELECT integrity_check FROM pragma_integrity_check) AS integrity_check FROM events;"
}

# stderr NAME: what process NAME wrote to stderr.
stderr() {
  say "== ${1^^}'s stderr"
  if [[ -s "$DIR/$1.err" ]]; then
    cat "$DIR/$1.err"
  else
    say "(empty)"
  fi
}

# submit_one KEY: one client's submission, which xargs runs as
# `measure.sh submit-one KEY`. Every tenth key is a Notify job, whose effect
# is a line on ledgerd's stderr.
submit_one() {
  local key=$1 kind reply
  if ((10#${key#k} % 10 == 0)); then
    kind="{\"Notify\":{\"message\":\"$key\"}}"
  else
    kind="{\"SleepThenRecord\":{\"ms\":1,\"token\":\"$key\"}}"
  fi
  reply=$(curl -sS --max-time 30 -w ' %{http_code}' -H 'Content-Type: application/json' \
    --data "{\"idempotency_key\":\"$key\",\"kind\":$kind,\"max_attempts\":3}" "http://$TARGET/jobs")
  printf '%s %s\n' "$key" "$reply" >>"$REPLIES"
}

# Case 1: one process, many concurrent clients, duplicate keys.
one_process() {
  local jobs=${1:-1000} clients=${2:-32} i t0 t1 t2 open deadline
  fresh one-process
  build
  start a 127.0.0.1:0
  for ((i = 0; i < jobs; i++)); do printf 'k%04d\nk%04d\n' "$i" "$i"; done >"$DIR/keys"
  say "submitting $(wc -l <"$DIR/keys") requests for $jobs keys from $clients concurrent clients"
  say "every key is sent twice, back to back; every tenth key is a Notify job"
  export TARGET=${ADDR[a]} REPLIES=$DIR/replies
  t0=$(ms_now)
  xargs -P "$clients" -n 1 "$0" submit-one <"$DIR/keys"
  t1=$(ms_now)
  say "all $(wc -l <"$REPLIES") replies in after $((t1 - t0)) ms;" \
    "by then $(value "SELECT count(*) FROM e WHERE kind = 'Succeeded';") jobs had succeeded"
  deadline=$(($(ms_now) + 300000))
  while :; do
    open=$(value "SELECT count(*) FROM e WHERE seq IN (SELECT max(seq) FROM e GROUP BY job)
                  AND kind NOT IN ('Succeeded', 'Died');")
    ((open == 0)) && break
    if (($(ms_now) > deadline)); then
      say "$open jobs still unfinished after 300 s"
      break
    fi
    sleep 0.5
  done
  t2=$(ms_now)
  say "queue drained $((t2 - t0)) ms after the first submission"
  stop a

  say "== HTTP replies, by status code"
  awk '{ print $NF }' "$REPLIES" | sort | uniq -c
  say "== HTTP replies, by key"
  awk '{
      key = $1; match($2, /"job":[0-9]+/); job = substr($2, RSTART + 6, RLENGTH - 6)
      sent[key]++; if (!((key, job) in seen)) { seen[key, job] = 1; jobs[key]++ }
      if (!(job in ids)) { ids[job] = 1; distinct++ }
      if ($2 ~ /"created":true/) created[key]++
    }
    END {
      for (k in sent) { keys++; if (jobs[k] == 1) one_job++; if (created[k] == 1) once++ }
      printf "keys: %d; keys whose replies all name the same job: %d; keys created exactly once: %d; distinct job ids: %d\n",
        keys, one_job, once, distinct
    }' "$REPLIES"

  say "== events, by kind"
  table "SELECT kind, count(*) AS events FROM e GROUP BY kind ORDER BY min(seq);"
  say "== Submitted events per idempotency key"
  table "SELECT n AS submitted_events, count(*) AS keys FROM
      (SELECT count(*) AS n FROM e WHERE kind = 'Submitted' GROUP BY json_extract(v, '\$.idempotency_key'))
    GROUP BY n;"
  say "== effect intents per (job, attempt)"
  table "SELECT n AS intents, count(*) AS job_attempts FROM
      (SELECT count(*) AS n FROM effects GROUP BY job, attempt) GROUP BY n;"
  say "== per job: leases, intents, outcomes and Succeeded events"
  table "WITH ev AS (SELECT job, sum(kind = 'Leased') AS leases, sum(kind = 'Succeeded') AS succeeded
                     FROM e GROUP BY job),
              fx AS (SELECT job, count(*) AS intents, count(outcome) AS outcomes FROM effects GROUP BY job)
    SELECT leases, coalesce(intents, 0) AS intents, coalesce(outcomes, 0) AS outcomes, succeeded,
           count(*) AS jobs
    FROM ev LEFT JOIN fx USING (job) GROUP BY 1, 2, 3, 4;"
  say "== each Succeeded event's result against its own job's token or message"
  table "SELECT count(*) AS succeeded_events,
      sum(json_extract(x.v, '\$.result') = coalesce(json_extract(s.v, '\$.kind.SleepThenRecord.token'),
          'notified: ' || json_extract(s.v, '\$.kind.Notify.message'))) AS own_result
    FROM e x JOIN e s ON s.kind = 'Submitted' AND s.job = x.job WHERE x.kind = 'Succeeded';"
  say "== did the executor run while clients were still submitting?"
  table "SELECT (SELECT count(*) FROM e WHERE kind = 'Leased'
                 AND seq < (SELECT max(seq) FROM e WHERE kind = 'Submitted')) AS leases_before_last_submission,
                (SELECT count(*) FROM e WHERE kind = 'Submitted') AS submissions;"
  say "== Notify effects performed, from A's stderr: times each message was sent"
  sed -n 's/^ledgerd: notify: //p' "$DIR/a.err" | sort | uniq -c |
    awk '{ n[$1]++ } END { for (t in n) printf "sent %d time(s): %d messages\n", t, n[t] }'
  say "== A's other stderr lines"
  grep -v '^ledgerd: notify: ' "$DIR/a.err" || say "(none)"
}

# Case 1, extra: one process, an effect that runs past its own lease.
long_lease() {
  fresh long-lease
  build
  T0=$(ms_now)
  start a 127.0.0.1:0
  say "$(since) A: submit job 0, a 35 s sleep_then_record (the lease is 30 s), then job 1, a 1 ms one"
  say "A: $(post a '{"idempotency_key":"long","kind":{"SleepThenRecord":{"ms":35000,"token":"long"}},"max_attempts":3}')"
  say "A: $(post a '{"idempotency_key":"short","kind":{"SleepThenRecord":{"ms":1,"token":"short"}},"max_attempts":3}')"
  watch 60 0 1
  stderr a
  stop a
  journal_report
}

# Case 2: plan 04's crash test, exactly as CI runs it.
restart() {
  show "cargo test -p ledgerd --test crash_recovery -- --ignored --nocapture"
}

# Case 3: two processes on one journal.
two_processes() {
  local run=${1:?usage: measure.sh two-processes RUN [VARIANT]} variant=${2:-different-addr} b_addr left
  if [[ ! $variant =~ ^(different-addr|same-addr|both-submit)$ ]]; then
    say "unknown variant: $variant"
    return 2
  fi
  fresh "two-processes-$variant-$run"
  build
  T0=$(ms_now)
  start a 127.0.0.1:0
  b_addr=127.0.0.1:0
  if [[ $variant == same-addr ]]; then
    b_addr=${ADDR[a]}
  fi
  say "$(since) A: submit job 0, a 5 s sleep_then_record"
  say "A: $(post a "{\"idempotency_key\":\"long\",\"kind\":{\"SleepThenRecord\":{\"ms\":5000,\"token\":\"run-$run\"}},\"max_attempts\":3}")"
  wait_for a 0 '*effect InFlight(1)' 10
  left=$(($(value "SELECT json_extract(v, '\$.expires') FROM e WHERE kind = 'Leased' AND job = 0;") - $(ms_now)))
  say "$(since) A's lease on job 0 has $left ms left; starting B on the same LEDGER_DB"
  start b "$b_addr" || true
  watch 60 0
  if [[ $variant == both-submit ]]; then
    say "$(since) submit one new job to each process"
    say "A: $(post a '{"idempotency_key":"to-a","kind":{"SleepThenRecord":{"ms":500,"token":"from-a"}},"max_attempts":3}')"
    say "B: $(post b '{"idempotency_key":"to-b","kind":{"SleepThenRecord":{"ms":500,"token":"from-b"}},"max_attempts":3}')"
    watch 90 0 1
  fi
  say "== still running at the end? A: $(alive a && echo yes || echo no), B: $(alive b && echo yes || echo no)"
  stderr a
  stderr b
  stop a
  stop b
  journal_report
}

# Case 4: what can supply `now` to a lease decision.
clock() {
  show "grep -rnE 'SystemTime|Instant' crates/*/src"
  show "grep -rnE 'now\(\)' crates/*/src"
  show "grep -rnE 'Command::(Lease|ExpireLeases|Fail) \{' crates/*/src"
  show "grep -rnE 'recover\(|Ledger::open\(|next_work\(' crates/*/src crates/*/tests"
  show "grep -rnE 'expires|not_before' crates/ledger-core/src"
  show "grep -nE '\.route\(' crates/ledgerd/src/lib.rs"
  show "grep -n -A5 '^pub struct SubmitJob' crates/ledger-api/src/lib.rs"
  show "./scripts/ratchets/core-purity.sh ."
}

case ${1:-} in
  one-process) one_process "${@:2}" ;;
  long-lease) long_lease ;;
  restart) restart ;;
  two-processes) two_processes "${@:2}" ;;
  clock) clock ;;
  submit-one) submit_one "$2" ;;
  *)
    sed -n '2,23p' "$0"
    exit 2
    ;;
esac
