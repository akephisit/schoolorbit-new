# SchoolOrbit project instructions

Read [`.rules`](./.rules) before analysis or changes. It is the single authoritative development standard.

Active references:

- [Documentation index](./docs/README.md)
- [Testing](./docs/TESTING.md)
- [Operations](./docs/OPERATIONS.md)

High-risk invariants:

- never edit an applied migration;
- never store or log plaintext national IDs;
- use generated permission and API contracts;
- verify claims with the change-type matrix in `.rules`.
