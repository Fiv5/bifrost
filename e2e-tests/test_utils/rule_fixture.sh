#!/bin/bash

rule_fixture_content() {
    local fixture_path="$1"
    shift || true

    local content
    content=$(<"$fixture_path")

    local pair key value
    for pair in "$@"; do
        key="${pair%%=*}"
        value="${pair#*=}"
        content="${content//__${key}__/$value}"
    done

    printf '%s' "$content"
}

render_rule_fixture_to_file() {
    local fixture_path="$1"
    local output_path="$2"
    shift 2 || true

    mkdir -p "$(dirname "$output_path")"
    rule_fixture_content "$fixture_path" "$@" > "$output_path"
}

custom_rule_config_from_fixture() {
    local fixture_path="$1"
    shift || true

    local content
    content=$(rule_fixture_content "$fixture_path" "$@")
    jq -Rn --arg rules "$content" '{mode:"custom",custom_rules:$rules}'
}
