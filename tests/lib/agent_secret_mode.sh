#!/usr/bin/env bash
# The agent-secret mode: `X-Use-Owner-Secret`, `outlayer secrets set-for-agent`,
# `/wallet/v1/agent-secret/*`. A secret stored FOR an agent under the agent's own
# account, read with a header instead of a body `secrets_ref`.
#
# Every live case of that mode asks `agent_secret_mode <label>` before it runs,
# so the whole mode is one grep away and can be switched off in one place:
#
#   AGENT_SECRET_MODE=run    (default) the cases run as part of their suites
#   AGENT_SECRET_MODE=skip   the cases are noted and skipped
#
# Nothing outside those cases depends on the mode; the ordinary `secrets_ref`
# path is what every other case uses.
AGENT_SECRET_MODE="${AGENT_SECRET_MODE:-run}"

agent_secret_mode() { # agent_secret_mode <label>  → 0 to run, 1 to skip
  case "$AGENT_SECRET_MODE" in
    run) return 0 ;;
    skip)
      printf '  \033[90m%s SKIPPED: AGENT_SECRET_MODE=skip (agent-secret mode)\033[0m\n' "$1" >&2
      return 1 ;;
    *)
      echo "AGENT_SECRET_MODE must be 'run' or 'skip', not '$AGENT_SECRET_MODE'" >&2
      exit 1 ;;
  esac
}
