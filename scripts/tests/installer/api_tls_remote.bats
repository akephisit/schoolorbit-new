#!/usr/bin/env bats

setup() {
	export TEST_ROOT
	TEST_ROOT=$(mktemp -d)
	export SCHOOLORBIT_STACK_ROOT="$TEST_ROOT/stack"
	export BASE_DOMAIN=example.test
	export FIXTURE="$TEST_ROOT/fixture"
	export COMMAND_LOG="$TEST_ROOT/commands"
	export TLS_SCRIPT="$BATS_TEST_DIRNAME/../../lib/pipeline-remote/api-tls.sh"
	mkdir -p "$TEST_ROOT/bin" "$FIXTURE" "$SCHOOLORBIT_STACK_ROOT/nginx/ssl"
	openssl req -x509 -newkey rsa:2048 -nodes -keyout "$FIXTURE/ca.key" -out "$FIXTURE/ca.pem" \
		-days 30 -subj /CN=FixtureRoot >/dev/null 2>&1
	openssl req -new -newkey rsa:2048 -nodes -keyout "$FIXTURE/privkey.pem" -out "$FIXTURE/request.csr" \
		-subj /CN=admin-api.example.test >/dev/null 2>&1
	printf '%s\n' 'subjectAltName=DNS:admin-api.example.test,DNS:school-api.example.test' 'extendedKeyUsage=serverAuth' >"$FIXTURE/extensions"
	openssl x509 -req -in "$FIXTURE/request.csr" -CA "$FIXTURE/ca.pem" -CAkey "$FIXTURE/ca.key" \
		-CAcreateserial -out "$FIXTURE/leaf.pem" -days 20 -extfile "$FIXTURE/extensions" >/dev/null 2>&1
	cat "$FIXTURE/leaf.pem" "$FIXTURE/ca.pem" >"$FIXTURE/fullchain.pem"
	export SSL_CERT_FILE="$FIXTURE/ca.pem"
	cat >"$TEST_ROOT/bin/podman" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >>"$COMMAND_LOG"
case "$1" in
    run)
        test -s "$SCHOOLORBIT_STACK_ROOT/acme/config/credentials.ini"
        test "$(stat -c %a "$SCHOOLORBIT_STACK_ROOT/acme/config/credentials.ini")" = 600
        grep -Fq 'dns_cloudflare_api_token = private-fixture-token' "$SCHOOLORBIT_STACK_ROOT/acme/config/credentials.ini"
        [[ ! -e $TEST_ROOT/fail-issuance ]] || exit 1
        target="$SCHOOLORBIT_STACK_ROOT/acme/config/live/schoolorbit-api"
        mkdir -p "$target"
        cp "$FIXTURE/fullchain.pem" "$FIXTURE/privkey.pem" "$target/"
        ;;
    container) [[ -e $TEST_ROOT/nginx-exists ]] ;;
    exec)
        if [[ -e $TEST_ROOT/fail-reload && $* == *'nginx -s reload'* ]]; then
            rm "$TEST_ROOT/fail-reload"
            exit 1
        fi
        ;;
    *) exit 64 ;;
esac
SH
	chmod +x "$TEST_ROOT/bin/podman"
	export PATH="$TEST_ROOT/bin:$PATH"
}

teardown() {
	rm -rf "$TEST_ROOT"
}

stage_certificate() {
	printf '%s\n' '{"cloudflare_token":"private-fixture-token"}' | bash "$TLS_SCRIPT" stage
}

@test "DNS issuance stages public-trust pair without changing serving certificate or retaining token" {
	stage_certificate >"$TEST_ROOT/metadata"
	jq -e '.fingerprint | test("^[0-9a-f]{64}$")' "$TEST_ROOT/metadata"
	[ ! -e "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/schoolorbit-origin.pem" ]
	[ ! -e "$SCHOOLORBIT_STACK_ROOT/acme/config/credentials.ini" ]
	! grep -Fq private-fixture-token "$COMMAND_LOG"
	! grep -Fq private-fixture-token "$TEST_ROOT/metadata"
	grep -Fq 'certbot/dns-cloudflare:v5.8.0@sha256:' "$COMMAND_LOG"
}

@test "failed DNS validation removes ephemeral credentials and keeps existing material" {
	touch "$TEST_ROOT/fail-issuance"
	run stage_certificate
	[ "$status" -ne 0 ]
	[ ! -e "$SCHOOLORBIT_STACK_ROOT/acme/config/credentials.ini" ]
	[ ! -e "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/.api-tls-current" ]
	[[ $output != *private-fixture-token* ]]
}

@test "untrusted chains and mismatched private keys refuse staging" {
	export SSL_CERT_FILE=/dev/null
	run stage_certificate
	[ "$status" -ne 0 ]
	[ ! -e "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/.api-tls-current" ]
	export SSL_CERT_FILE="$FIXTURE/ca.pem"
	cp "$FIXTURE/ca.key" "$FIXTURE/privkey.pem"
	run stage_certificate
	[ "$status" -ne 0 ]
	[[ $output == *'do not match'* ]]
}

@test "activation exports stable filenames through one generation link and retry preserves fingerprint" {
	details=$(stage_certificate)
	fingerprint=$(jq -r .fingerprint <<<"$details")
	bash "$TLS_SCRIPT" activate "$fingerprint" >"$TEST_ROOT/active"
	[ "$(stat -Lc %a "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/schoolorbit-origin.key")" = 600 ]
	[ "$(readlink "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/.api-tls-current")" = "api-tls-generations/$fingerprint" ]
	details=$(stage_certificate)
	[ "$(jq -r .fingerprint <<<"$details")" = "$fingerprint" ]
	bash "$TLS_SCRIPT" status | jq -e --arg fingerprint "$fingerprint" '.fingerprint == $fingerprint'
}

@test "reload failure restores previous pair and leaves maintenance configuration untouched" {
	details=$(stage_certificate)
	fingerprint=$(jq -r .fingerprint <<<"$details")
	cp "$FIXTURE/fullchain.pem" "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/schoolorbit-origin.pem"
	cp "$FIXTURE/privkey.pem" "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/schoolorbit-origin.key"
	mkdir -p "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d"
	printf '%s\n' maintenance >"$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf"
	touch "$TEST_ROOT/nginx-exists" "$TEST_ROOT/fail-reload"
	run bash "$TLS_SCRIPT" activate "$fingerprint"
	[ "$status" -ne 0 ]
	[[ $(readlink "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/.api-tls-current") == api-tls-generations/legacy-* ]]
	[ "$(cat "$SCHOOLORBIT_STACK_ROOT/nginx/conf.d/school-api.conf")" = maintenance ]
	[ "$(openssl x509 -in "$SCHOOLORBIT_STACK_ROOT/nginx/ssl/schoolorbit-origin.pem" -outform DER | sha256sum)" = "$(openssl x509 -in "$FIXTURE/leaf.pem" -outform DER | sha256sum)" ]
}
