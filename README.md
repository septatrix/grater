# Grater

> Nothing but a cheese grater for grades!

## Installation instructions

> [!NOTE]
> Most up-to-date informations can always be inferred from the GitHub Action
> which is used to build and deploy this project.

**Prerequisites**:

* `nodejs` >= 22.13
* `pnpm`
* `rust` >= 1.85.0
  * including the `wasm32-unknown-unknown` target

**Setup**:

Run `pnpm install` to install all dependencies.
Afterwards, run `pnpm exec wasm-pack build`
to generate the WASM package of grater.

You should now be ready to run `pnpm run dev`.

## Examination regulations

The grade calculation follows the RWTH regulations
(consolidated versions of 2025-09-18, [ÜPO](https://www.rwth-aachen.de/global/show_document.asp?id=aaaaaaaaddqknak) § 10):

| | Bachelor Informatik (PO 2022) | Master Informatik (PO 2023) |
| --- | --- | --- |
| Final thesis | 15 CP, weight 1.5 | 30 CP, weight 1 |
| Schwerpunktkolloquium | – | 3 CP, weight 4, cannot be struck |
| Striking of grades | one grade per area, up to 30 CP in total | a single grade, up to 15 CP |
| Excluded from the grade | Software-Projektpraktikum, non-technical electives, Praktikum Systemprogrammierung | – |

The final grade is truncated (not rounded) after the first decimal place.
Striking grades is only allowed
if all module exams were completed within the regular period of study,
which grater cannot verify.
