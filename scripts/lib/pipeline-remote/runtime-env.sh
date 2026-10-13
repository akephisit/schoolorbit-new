#!/usr/bin/env bash
# Decode the installer-owned dotenv format without evaluating shell expressions.
schoolorbit_runtime_env_value() {
    local key=$1 file=$2 line value decoded char count
    local -i index=0
    [[ $key =~ ^[A-Z][A-Z0-9_]*$ ]] || return 64
    if [[ ! -f $file || -L $file || $(stat -c %a "$file") != 600 ]]; then
        echo 'Private runtime environment file is unavailable' >&2
        return 65
    fi
    count=$(grep -c "^${key}=" "$file" || true)
    [[ $count == 1 ]] || {
        echo "Expected one runtime environment value: ${key}" >&2
        return 65
    }
    line="$(grep -m 1 "^${key}=" "$file")" || return
    value=${line#*=}
    if [[ ${#value} -lt 2 || ${value:0:1} != "'" || ${value: -1} != "'" ]]; then
        echo "Runtime environment value is not installer encoded: ${key}" >&2
        return 65
    fi
    value=${value:1:${#value}-2}
    decoded=
    while ((index < ${#value})); do
        char=${value:index:1}
        if [[ $char == \\ ]]; then
            ((index += 1))
            if ((index >= ${#value})); then
                echo "Runtime environment value has invalid escaping: ${key}" >&2
                return 65
            fi
            char=${value:index:1}
            if [[ $char != \\ && $char != "'" ]]; then
                echo "Runtime environment value has invalid escaping: ${key}" >&2
                return 65
            fi
        fi
        decoded+=$char
        ((index += 1))
    done
    printf '%s' "$decoded"
}
