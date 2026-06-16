# Changelog

## v0.5.0 — 2026-06-16

`doxa reason <framework> --scenario <abox.ttl> [--explain]`: evaluates a moral scenario under a chosen normative framework via ousia-reason, returning RightAction/WrongAction/PermissibleAction/undetermined. Shipped trolley.ttl scenario fixture that yields different verdicts under consequentialism vs deontology. --explain prints the ordered axiom justification chain. Fallback to recorded ousia-reason output when CLI absent. 7 acceptance test functions green.

## v0.4.0 — 2026-06-16

doxa guard — pluralist action gating under an explicit, inspectable policy: unanimity, majority, framework:<n>, or lexical:<priority> aggregation of multi-framework verdicts. Exit codes: allow=0, flag=10, deny=20.

## v0.3.0 — 2026-06-16

Adds `doxa compare <fw...> [--scenario <abox>] [--format text|json]`: fan-out N frameworks via ousia-reason, computes consensus/conflict/abstentions matrix, text and JSON output.
