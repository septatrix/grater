# Grater

> Nothing but a cheese grater for grades!

## Installation instructions

> [!NOTE]
> Most up-to-date informations can always be inferred from the GitHub Action
> which is used to build and deploy this project.

**Prerequisites**:

* `nodejs`
* `pnpm`
* `rust` >= 1.78.0
  * including the `wasm32-unknown-unknown` target

**Setup**:

Run `pnpm install` to install all dependencies.
Afterwards, run `pnpm exec wasm-pack build`
to generate the WASM package of grater.

You should now be ready to run `pnpm run dev`.
