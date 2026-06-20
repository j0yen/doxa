# doxa

A framework-neutral, BFO-grounded moral ontology and a CLI that reasons over it — so different ethical theories can be compared on the same vocabulary, and an action can be gated under an explicit, inspectable policy.

To compare ethical theories deductively, they have to reason over the *same* terms. If consequentialism and deontology each define their own "harm" and "right action," any disagreement between them is just two vocabularies talking past each other. `doxa` separates the shared vocabulary from the theories that argue over it: a neutral moral-core TBox declares *what an action has* — a consequence, an intention, a maxim — but never *what makes it right*. Each framework module then adds its own axioms over that shared core. Because the referents are identical, the frameworks become commensurable: when they disagree about the trolley problem, the disagreement is real, and you can see exactly which axiom produces it.

## The model

- **Moral core** (`spec-core/`): 16 classes (MoralAgent, MoralPatient, Action, Consequence, Intention, Duty, Virtue, Vice, Right, Harm, Wellbeing, Maxim, Justice, RightAction, WrongAction, PermissibleAction) and 8 object properties, each grounded to a BFO 2020 parent. `RightAction` / `WrongAction` / `PermissibleAction` are *declared but left undefined* — the core states the shape of a moral situation and stops there.
- **Frameworks** (`spec-frameworks/`): three modules — `consequentialism`, `deontology`, `virtue-ethics` — each adding equivalence/subclass axioms that define what *its* theory counts as right or wrong, annotated with its philosophical grounding.
- **Scenarios** (`scenarios/`): ABox `.ttl` files describing a concrete case. `scenarios/trolley.ttl` is built to yield genuinely different verdicts across frameworks.

The core spec compiles to OWL 2 DL through [`ousia-forge`](https://github.com/j0yen/ousia-forge); reasoning is delegated to [`ousia-reason`](https://github.com/j0yen/ousia-reason) and gating to [`ousia-guard`](https://github.com/j0yen/ousia-guard).

## Install

```sh
cargo install --path . --locked
# or build and copy the release binary:
cargo build --release
install -m755 target/release/doxa ~/.local/bin/doxa
```

MSRV 1.85. The `build`, `compare`, `reason`, and `guard` subcommands shell out to the `ousia-*` tools; resolve each from `$PATH` or pass `--forge` / `--reason` / `--ousia-guard`. When a tool is absent, `compare`/`reason`/`guard` fall back to verdicts recorded in the scenario fixture so the pipeline stays demonstrable offline.

## Subcommands

| Command | Does |
|---|---|
| `doxa build-core [--out core.owl]` | Compile the neutral moral TBox to OWL 2 DL via `ousia-forge build` |
| `doxa check-core` | Validate the core spec via `ousia-forge check` |
| `doxa list [--format text\|json]` | List the available frameworks with their descriptions |
| `doxa build <fw> [--out f.owl]` / `doxa build --all` | Compile core + a framework's axioms (or every framework) to OWL/XML |
| `doxa reason <fw> --scenario <abox.ttl> [--explain]` | Evaluate one scenario under one framework → RightAction / WrongAction / PermissibleAction / undetermined; `--explain` prints the axiom chain |
| `doxa compare <fw...> [--scenario <abox.ttl>] [--format text\|json]` | Compare frameworks: with a scenario, an agreement/conflict matrix; without one, the structural difference in what each framework treats as decisive |
| `doxa guard --scenario <abox.ttl> --policy <policy>` | Aggregate per-framework verdicts into one `allow` / `flag` / `deny` |

## Quickstart

```sh
# See where two frameworks structurally diverge — no scenario needed
doxa compare consequentialism deontology

# Run the trolley problem under each, then compare
doxa reason consequentialism --scenario scenarios/trolley.ttl --explain
doxa reason deontology --scenario scenarios/trolley.ttl --explain
doxa compare consequentialism deontology virtue-ethics --scenario scenarios/trolley.ttl

# Gate the action under an explicit pluralist policy
doxa guard --scenario scenarios/trolley.ttl --policy unanimity
```

## guard — pluralist gating

`guard` turns a set of per-framework verdicts into a single decision under a policy you choose and can inspect:

- `unanimity` — `allow` only if every framework permits; otherwise `deny`/`flag`.
- `majority` — go with the plurality verdict.
- `framework:<name>` — defer to one framework.
- `lexical:<a,b,...>` — order frameworks by priority; the first decisive one wins.

Exit codes are part of the contract: `allow` = 0, `flag` = 10, `deny` = 20, so `guard` composes into scripts. `--explain` includes the per-framework axiom chains behind the decision.

## Where it fits

`doxa` is the moral-domain layer of the `ousia` ontology toolchain: [`ousia-forge`](https://github.com/j0yen/ousia-forge) compiles specs to OWL, [`ousia-reason`](https://github.com/j0yen/ousia-reason) classifies entailments, and [`ousia-guard`](https://github.com/j0yen/ousia-guard) gates actions. `doxa` supplies the ethics-specific vocabulary, frameworks, and CLI on top.

## License

MIT OR Apache-2.0
