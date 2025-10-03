For user-facing goals refer to the design/DESIGN_DOCUMENT.md. If something critical is missing please tell me!
For more granular step-by-step plans refer to the design/PROTOTYPE_PLAN.md

### Checking

After every code change, vet the code both with `cargo check` and `cargo clippy` on all 3 crates.
Also verify that design documentation is still correct.


### Misc

- whenever you encounter a file above 300 Lines of Code, consider if it is a good idea to split it up
- in general if you think you have like 80% of the problem solved, and we're just talking about small compile-time errors pertaining to function signatures or bevy internals, or perhaps rust lifetime and aliasing issues, just early exit to conserve context space (and AI token usage). I can often fix these problems myself. Just give me a summary of the problems.