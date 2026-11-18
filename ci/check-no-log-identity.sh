#!/usr/bin/env bash
# Refuses Rust log calls that format identity material: a public key, a wallet
# or IP address, a nonce, a seed or a mnemonic. Those values must never reach a
# log in clear, on any platform. A call is flagged when an identifier it
# formats (an inline `{name}` capture or an argument expression) has one of
# those words as a segment of its snake_case or dotted name.
#
# A call that formats such a value on purpose, already redacted or known to be
# a non-identifying constant (a loopback, a documentation range), carries
# `no-log: <why>` in a comment on its first line or on the line above.
#
#   bash ci/check-no-log-identity.sh [path...]   tracked *.rs files by default
#
# Prints file:line and the call for each finding and exits 1 when there is one.
set -euo pipefail

if [ "$#" -gt 0 ]; then
	files=("$@")
else
	files=()
	while IFS= read -r file; do
		files+=("$file")
	done < <(git ls-files -- '*.rs')
fi
[ "${#files[@]}" -gt 0 ] || exit 0

awk '
function sensitive(id,    n, parts, i, seg, prev) {
	n = split(tolower(id), parts, /[_.]/)
	prev = ""
	for (i = 1; i <= n; i++) {
		seg = parts[i]
		if (seg ~ /^(pubkey|pubkeys|address|addresses|addr|addrs|ip|ips|ipv4|ipv6|nonce|nonces|seed|seeds|mnemonic|mnemonics)$/)
			return 1
		if (seg == "key" && (prev == "pub" || prev == "public" || prev == "private" || prev == "secret"))
			return 1
		prev = seg
	}
	return 0
}

# Identifiers that name a property of the value rather than the value itself.
function harmless(id) {
	return id ~ /(^|[_.])(len|count|family|kind|version|generation)$/ \
		|| id ~ /(^|[_.])(is|has|enable|disable)_[a-z0-9_]+$/
}

function check(call, file, line, waived,    rest, fmt, args, ids, id, inline) {
	if (waived) return
	# Split the format string from the arguments.
	if (!match(call, /"([^"\\]|\\.)*"/)) return
	fmt = substr(call, RSTART, RLENGTH)
	args = substr(call, RSTART + RLENGTH)
	# Inline captures in the format string.
	inline = fmt
	while (match(inline, /\{[A-Za-z_][A-Za-z0-9_]*(:[^}]*)?\}/)) {
		id = substr(inline, RSTART + 1, RLENGTH - 2)
		sub(/:.*/, "", id)
		if (sensitive(id) && !harmless(id)) { report(file, line, call); return }
		inline = substr(inline, RSTART + RLENGTH)
	}
	# Argument expressions: drop string literals, then look at each path.
	gsub(/"([^"\\]|\\.)*"/, "", args)
	while (match(args, /[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*(\(\))?/)) {
		id = substr(args, RSTART, RLENGTH)
		args = substr(args, RSTART + RLENGTH)
		sub(/\(\)$/, "", id)
		if (sensitive(id) && !harmless(id)) { report(file, line, call); return }
	}
}

function report(file, line, call) {
	gsub(/[ \t]+/, " ", call)
	printf "%s:%d: %s\n", file, line, substr(call, 1, 200)
	found = 1
}

FNR == 1 { collecting = 0; pending = 0 }
{
	text = $0
	if (!collecting) {
		# A waiver in the comment block right above the call applies to it.
		if (text ~ /^[ \t]*\/\//) {
			if (text ~ /no-log:/) pending = 1
			next
		}
		if (match(text, /(^|[^A-Za-z0-9_:])((log|tracing)::)?(trace|debug|info|warn|error)!\(/)) {
			collecting = 1
			start = FNR
			waived = (pending || text ~ /no-log:/)
			call = substr(text, RSTART)
		} else {
			pending = 0
			next
		}
	} else {
		call = call " " text
		if (text ~ /no-log:/) waived = 1
	}
	# Track parentheses outside string literals to find the end of the call.
	probe = call
	gsub(/"([^"\\]|\\.)*"/, "", probe)
	opens = gsub(/\(/, "(", probe)
	closes = gsub(/\)/, ")", probe)
	if (opens <= closes) {
		check(call, FILENAME, start, waived)
		collecting = 0
		pending = 0
	}
}
END { exit found ? 1 : 0 }
' "${files[@]}" && exit 0

echo "Log calls that format identity material (redact it, or say why with no-log: <why>):" >&2
exit 1
