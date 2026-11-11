#!/usr/bin/env bash
# Source owners for current and historical disposable backend benchmark snapshots.
benchmark_owner_source() {
    local backend=$1 owner=$2 source
    local -a sources
    case "$backend/$owner" in
        */application) sources=(src/main.rs) ;;
        backend-school/academic) sources=(crates/school-academic-core-http/src/core/handlers.rs crates/school-academic-http/src/core/handlers.rs src/modules/academic/core/handlers.rs) ;;
        backend-school/attendance) sources=(crates/school-attendance/src/services/sessions.rs) ;;
        backend-school/certificates) sources=(crates/school-certificates-http/src/handlers.rs src/modules/certificates/handlers.rs) ;;
        backend-school/navigation) sources=(crates/school-navigation/src/services/menu_service.rs src/modules/menu/services/menu_service.rs) ;;
        backend-school/notifications) sources=(crates/school-notifications/src/publisher.rs src/services/notification.rs) ;;
        *) return 64 ;;
    esac
    for source in "${sources[@]}"; do
        if [[ -f "$source" ]]; then
            printf '%s\n' "$source"
            return 0
        fi
    done
    printf 'No source for benchmark owner %s/%s\n' "$backend" "$owner" >&2
    return 1
}

benchmark_owner_package() {
    local source=$1 package
    if [[ "$source" == crates/*/* ]]; then
        package=${source#crates/}
        printf '%s\n' "${package%%/*}"
    else
        printf '%s\n' "${2:-backend-school}"
    fi
}
