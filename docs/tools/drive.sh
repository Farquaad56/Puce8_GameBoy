#!/usr/bin/env bash
# Runs the pipeline with Hermes Agent: one FRESH one-shot session per sub-task (small context).
# Stops at GATE / BLOCKED / FINISHED. Usage: docs/tools/drive.sh [max_sessions]
# Env: HERMES_PROFILE (default gb-emu), MAX_TURNS (default 80),
#      GB_CODER = opencode (default: Hermes delegates CODE/TEST tasks to OpenCode) | self
#      GB_OPENCODE_MODEL = optional provider/model passed to `opencode run --model`
cd "$(dirname "$0")/../.." || exit 2
MAX="${1:-200}"
PROFILE="${HERMES_PROFILE:-gb-emu}"
TURNS="${MAX_TURNS:-80}"
CODER="${GB_CODER:-opencode}"
for _ in $(seq 1 "$MAX"); do
  st=$(python3 docs/tools/task.py begin | tail -1)
  case "$st" in
    "STATE: RUN") ;;
    *) echo "$st"; python3 docs/tools/task.py status | head -3; exit 0 ;;
  esac
  role=$(python3 docs/tools/task.py role)
  echo ">>> session: role=$role coder=$CODER"
  skills="gb-$role"
  deleg=0
  if [ "$CODER" = "opencode" ] && { [ "$role" = "code" ] || [ "$role" = "test" ]; }; then
    skills="gb-$role,opencode"; deleg=1
  fi
  prompt=$(mktemp)
  {
    echo "Start with: python3 docs/tools/task.py show"
    if [ "$deleg" = 1 ]; then
      echo "Coder: opencode. Follow the 'Coder: opencode' procedure of your skill."
      [ -n "$GB_OPENCODE_MODEL" ] && echo "OpenCode model: $GB_OPENCODE_MODEL"
    else
      echo "Coder: self. Do only that task, following AGENTS.md. Finish with: task.py check, then task.py done."
    fi
    echo "Speak French to the user."
  } > "$prompt"
  hermes -p "$PROFILE" chat --oneshot -Q -s "$skills" --max-turns "$TURNS" --query-file "$prompt" \
    || echo "(hermes exited non-zero: the driver retries until the session limit)"
  rm -f "$prompt"
done
