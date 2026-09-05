#!/usr/bin/env bats

setup() {
    source "$BATS_TEST_DIRNAME/../../lib/schoolorbit-installer/remote/deployment_timing.sh"
}

@test "deployment timer reports a bounded phase duration" {
    export SCHOOLORBIT_TIMER_NOW=110

    run schoolorbit_timer_report image_pull 100

    [ "$status" -eq 0 ]
    [ "$output" = 'deployment_timing phase=image_pull seconds=10' ]
}

@test "deployment timer rejects an unsafe phase name" {
    export SCHOOLORBIT_TIMER_NOW=110

    run schoolorbit_timer_report 'image pull' 100

    [ "$status" -eq 64 ]
    [ "$output" = 'Invalid deployment timing phase' ]
}

@test "deployment timer rejects a non-integer start time" {
    export SCHOOLORBIT_TIMER_NOW=110

    run schoolorbit_timer_report image_pull yesterday

    [ "$status" -eq 64 ]
    [ "$output" = 'Invalid deployment timing start' ]
}

@test "deployment timer rejects an end time before the start" {
    export SCHOOLORBIT_TIMER_NOW=99

    run schoolorbit_timer_report image_pull 100

    [ "$status" -eq 65 ]
    [ "$output" = 'Deployment timing clock moved backwards' ]
}

@test "deployment timer obtains an integer epoch" {
    unset SCHOOLORBIT_TIMER_NOW

    run schoolorbit_timer_now

    [ "$status" -eq 0 ]
    [[ "$output" =~ ^[0-9]+$ ]]
}
