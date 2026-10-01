#!/usr/bin/env bats

setup() {
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/personnel_preflight.sh"
    export SCHOOLORBIT_INTERNAL_API_SECRET=synthetic-secret-must-not-appear
    PAYLOAD="$BATS_TEST_TMPDIR/payload.json"
    RESPONSE_FILE="$BATS_TEST_TMPDIR/response.json"
    jq -n '{success:true,data:{passed:true,totalSchools:2,schools:[
      {subdomain:"sandbox",passed:true,checks:[{code:"education_mapped",passed:true,count:0}]},
      {subdomain:"school",passed:true,checks:[{code:"education_mapped",passed:true,count:3}]}
    ]}}' >"$PAYLOAD"
}

curl() {
    if [[ ${CURL_FAIL:-0} == 1 ]]; then return 22; fi
    [[ " $* " == *'X-Internal-Caller: school-release'* ]] || return 64
    [[ " $* " == *"X-Internal-Secret: $SCHOOLORBIT_INTERNAL_API_SECRET"* ]] || return 64
    cp "$PAYLOAD" "$RESPONSE_FILE"
}

podman() {
    [[ "$1 $2 $3 $4" == 'run --rm -i test/jq' ]] || return 64
    shift 4
    command jq "$@"
}

release_gate() {
    schoolorbit_personnel_preflight test/jq "$RESPONSE_FILE" || return
    printf '%s\n' migration-started
}

@test "personnel gate accepts complete successful evidence for every tenant" {
    run release_gate

    [ "$status" -eq 0 ]
    [ "$output" = migration-started ]
}

@test "personnel gate refuses incomplete empty negative and failed evidence" {
    local original expression
    original=$(cat "$PAYLOAD")
    for expression in \
        '.success=false' \
        '.data.passed=false' \
        '.data.totalSchools=3' \
        '.data.totalSchools=0 | .data.schools=[]' \
        '.data.schools[1].passed=false' \
        '.data.schools[1].checks=[]' \
        '.data.schools[1].checks[0].passed=false' \
        '.data.schools[1].checks[0].count=-1'; do
        jq "$expression" <<<"$original" >"$PAYLOAD"
        run release_gate

        [ "$status" -ne 0 ]
        [[ $output != *migration-started* ]]
        [[ $output == *'maintenance remains enabled'* ]]
    done
}

@test "personnel gate prints bounded diagnostics without raw rows or credentials" {
    jq '.data.passed=false | .data.schools[1].passed=false
      | .data.schools[1].checks[0]={code:"education_unmapped",passed:false,count:1}
      | .data.schools[1].rawRow="synthetic-private-row-must-not-appear"' "$PAYLOAD" >"$PAYLOAD.next"
    mv "$PAYLOAD.next" "$PAYLOAD"
    run release_gate

    [ "$status" -ne 0 ]
    [[ $output == *'subdomain=school code=education_unmapped count=1'* ]]
    [[ $output != *synthetic-private-row* ]]
    [[ $output != *synthetic-secret* ]]
    [[ $output != *migration-started* ]]
}

@test "personnel gate refuses an unavailable endpoint or credential" {
    CURL_FAIL=1
    run release_gate
    [ "$status" -ne 0 ]
    [[ $output == *'request failed'* ]]
    [[ $output != *migration-started* ]]

    CURL_FAIL=0
    unset SCHOOLORBIT_INTERNAL_API_SECRET
    run release_gate
    [ "$status" -ne 0 ]
    [[ $output == *'credential is unavailable'* ]]
    [[ $output != *migration-started* ]]
}

@test "personnel gate refuses malformed responses without printing their content" {
    printf '%s\n' synthetic-private-invalid-response >"$PAYLOAD"
    run release_gate

    [ "$status" -ne 0 ]
    [[ $output == *'maintenance remains enabled'* ]]
    [[ $output != *synthetic-private* ]]
    [[ $output != *migration-started* ]]
}
