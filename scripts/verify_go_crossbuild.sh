#!/usr/bin/env bash
# Cross-validate the Rust Signature V4 implementation against the official
# Volcengine Go SDK. This downloads the upstream Go SDK into a temp dir, drops
# a test into its `base` package (so it can reach the unexported signing
# functions), feeds the SAME fixed inputs used by the Rust unit tests in
# src/sign.rs, and asserts the official Go signer produces identical values.
#
# A green run proves three independent implementations (this Rust crate, the
# official Python SDK, and the official Go SDK) emit byte-for-byte identical
# signatures for the same request.
#
# Requirements: a Go toolchain (>=1.18). Network access to fetch the Go SDK.
# Usage: ./scripts/verify_go_crossbuild.sh
set -euo pipefail

if ! command -v go >/dev/null 2>&1; then
  echo "error: Go toolchain not found (need go >= 1.18)" >&2
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo ">> cloning official volc-sdk-golang ..."
git clone --depth 1 https://github.com/volcengine/volc-sdk-golang "$WORK/sdk" >/dev/null 2>&1

cat > "$WORK/sdk/base/verify_rust_vectors_test.go" <<'GO'
package base

import (
	"net/url"
	"testing"
)

// Same fixed inputs as the Rust unit tests in src/sign.rs.
func TestVerifyRustVectors(t *testing.T) {
	const (
		SK      = "dGVzdHNlY3JldGtleTAwMDAwMDAwMDAwMDAwMDA="
		XDate   = "20230515T101010Z"
		Date    = "20230515"
		Region  = "cn-north-1"
		Service = "cv"
		Host    = "visual.volcengineapi.com"
		Method  = "POST"
		Path    = "/"
		CT      = "application/json"
		Body    = `{"req_key":"high_aes_general_v21_L","prompt":"a cat"}`
	)

	want := map[string]string{
		"body_sha256":    "b89b2b4d469713b0e2b588447761edd2e542d8082ef37720605a71caf55f0559",
		"signing_key":    "f6b24c110dd2b7de26f037bfd00008afb9ae62f519bd7b1e9f4c7f2d4ad4508a",
		"signed_headers": "content-type;host;x-content-sha256;x-date",
		"canon_hash":     "d572f00ec33bd4497e54a3d67cd10bd2bc59e68e6044767731e6055c86bb0404",
		"signature":      "635ae6b8792921876ed6c2147a9d0c14f4e9c7983ae5d691e150cb752234545d",
	}
	check := func(name, got string) {
		if got == want[name] {
			t.Logf("[MATCH]    %s = %s", name, got)
		} else {
			t.Errorf("[MISMATCH] %s\n  go:   %s\n  rust: %s", name, got, want[name])
		}
	}

	bodyHash := hashSHA256([]byte(Body))
	check("body_sha256", bodyHash)

	sk := signingKeyV4(SK, Date, Region, Service)
	const hexdig = "0123456789abcdef"
	skHex := ""
	for _, b := range sk {
		skHex += string(hexdig[b>>4]) + string(hexdig[b&15])
	}
	check("signing_key", skHex)

	queryList := url.Values{}
	queryList.Set("Action", "CVProcess")
	queryList.Set("Version", "2022-08-31")
	param := RequestParam{
		IsSignUrl: false, Body: []byte(Body), Host: Host,
		Path: Path, Method: Method, QueryList: queryList,
	}
	meta := new(metadata)
	meta.date, meta.service, meta.region, meta.signedHeaders, meta.algorithm = Date, Service, Region, "", "HMAC-SHA256"
	meta.credentialScope = concat("/", meta.date, meta.region, meta.service, "request")
	requestSignMap := map[string][]string{
		"Content-Type": {CT}, "Host": {Host},
		"X-Date": {XDate}, "X-Content-Sha256": {bodyHash},
	}
	canonHash := hashedCanonicalRequestV4(param, meta, requestSignMap, bodyHash)
	check("signed_headers", meta.signedHeaders)
	check("canon_hash", canonHash)

	stringToSign := concat("\n", meta.algorithm, XDate, meta.credentialScope, canonHash)
	check("signature", signatureV4(sk, stringToSign))
}
GO

echo ">> running cross-validation against official Go signer ..."
( cd "$WORK/sdk" && GOFLAGS=-mod=mod go test ./base/ -run TestVerifyRustVectors -v )

echo
echo ">> PASS: Rust vectors match the official Volcengine Go SDK byte-for-byte."
