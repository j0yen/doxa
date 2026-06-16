# doxa

Framework-neutral moral TBox — shared vocabulary for comparative ethical reasoning.

## TL;DR

To compare ethical philosophies deductively, they must reason over the *same*
vocabulary. `doxa` ships a BFO-grounded, framework-neutral **moral-domain TBox**:
15 classes (MoralAgent, MoralPatient, Action, Consequence, Intention, Duty, Virtue,
Right, Harm, Wellbeing, Maxim, Justice, RightAction, WrongAction, PermissibleAction)
and 8 object properties, compiled to OWL 2 DL via `ousia-forge`.

Every doxa framework module (next PRD) adds axioms over *this* vocabulary, making
frameworks commensurable. The core deliberately declares *what an action HAS*
(consequence, intention, maxim) but never *what makes it right* — that is each
framework's job.

## Subcommands

- `doxa build-core [--out core.owl]` — compile the neutral moral TBox to OWL 2 DL
- `doxa check-core` — validate the spec via `ousia-forge check`

Both subcommands resolve `ousia-forge` from `$PATH` or via `--forge <path>`.

## Install

```sh
cargo install --path . --locked
# or copy the release binary:
cargo build --release
install -m755 target/release/doxa ~/.local/bin/doxa
```

MSRV: 1.85

## Part of wintermute

`doxa` is part of the [wintermute](https://github.com/joeyen-atscale/wintermute)
agent-tooling ecosystem.

## License

MIT OR Apache-2.0
