#!/bin/sh
# Renders the report of the health check and manages the issue of the
# repository.
#
# The report is the JSON document the check writes to
# `target/provider-health/report.json`. The script prints the table of the
# report, writes it to the summary of the job when it runs in a workflow, and
# keeps an issue labelled `provider-health` open while a source of the
# registry is unhealthy.
#
# The script exits with a non-zero status when a source is unhealthy.
set -eu

usage() {
    printf 'usage: scripts/provider-health.sh <report>\n' >&2
    printf '       <report> is the path of the JSON report of the check\n' >&2
}

if [ "$#" -ne 1 ]; then
    usage
    exit 2
fi

report="$1"

# The label the issue of an unhealthy source carries.
label='provider-health'

# The colour of the label.
colour='d73a4a'

# Runs a command of the GitHub CLI against the repository of the workflow.
gh_run() {
    verb="$1"
    shift
    if [ -n "${GITHUB_REPOSITORY:-}" ]; then
        gh "$verb" --repo "$GITHUB_REPOSITORY" "$@"
    else
        gh "$verb" "$@"
    fi
}

# Writes a text to the summary of the job, when the script runs in a workflow.
summary() {
    if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
        printf '%s\n' "$1" >> "$GITHUB_STEP_SUMMARY"
    fi
}

# The table of the report: one row per source, sorted by the vendor and then
# by the name of the entry.
table() {
    jq -r '
        def cells: map(if . == null then "" else tostring end);
        def row: "| " + (cells | join(" | ")) + " |";
        [ .sources[] ]
        | sort_by([.vendor, .name])
        | (["bot", "vendor", "source", "state", "prefixes", "detail"] | row),
          (["---", "---", "---", "---", "---", "---"] | row),
          (.[] | [.name, .vendor, .source, .state, .prefixes, .detail] | row)
    ' "$report"
}

# The number of the open issue that carries the label, if there is one.
open_issue() {
    gh_run issue list --label "$label" --state open --limit 1 --json number \
        --jq '.[0].number // empty'
}

# Creates the label of the issue when the repository does not carry it yet.
ensure_label() {
    if ! gh_run label list --limit 200 --json name --jq '.[].name' | grep -qx "$label"; then
        gh_run label create "$label" --color "$colour" \
            --description 'A source of the registry of the crate is unhealthy'
    fi
}

# A report that does not exist is a failure of the check, not a failure of a
# source: the state of the sources is unknown, and an unknown state opens no
# issue.
missing() {
    printf '%s\n' "$1"
    summary "$1"
    exit 1
}

if [ ! -f "$report" ]; then
    missing "the check produced no report: $report is missing"
fi

if ! jq -e '(.sources | type) == "array"' "$report" >/dev/null 2>&1; then
    missing "the check produced no report: $report is not a report"
fi

total=$(jq '.sources | length' "$report")
if [ "$total" -eq 0 ]; then
    missing "the check produced no report: $report holds no source"
fi

unhealthy=$(jq '[.sources[] | select(.state == "failed" or .state == "pending")] | length' "$report")

if [ "$unhealthy" -gt 0 ]; then
    title="provider health: $unhealthy of $total sources are failing"
else
    title="every source of the registry is healthy"
fi

body="$title

$(table)"

printf '%s\n' "$body"
summary "$body"

ensure_label
issue=$(open_issue || true)

if [ "$unhealthy" -gt 0 ]; then
    if [ -n "$issue" ]; then
        gh_run issue comment "$issue" --body "$body"
    else
        gh_run issue create --title "$title" --label "$label" --body "$body"
    fi
    exit 1
fi

if [ -n "$issue" ]; then
    gh_run issue comment "$issue" --body "$body"
    gh_run issue close "$issue"
fi
