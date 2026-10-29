# Reruns on 92f6a80: stopped after two runs, a defect in the test change itself

Both whole-state runs failed after 4 checks with `ReferenceError: Cannot access 'work' before
initialization`. 92f6a80 declared a `const work` helper inside `main()`. That name shadows the module's
`work` (the run's directory) through the whole function, so the earlier uses of `work` threw.
`node --check` does not catch this. The reruns were stopped. The helper is renamed `totalWork` in the next
commit, and the reruns are in ../accept-int-<that commit>/. These logs are kept as they ran.
