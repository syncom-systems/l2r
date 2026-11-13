# LikeC4 for the design picture

Checked against LikeC4 1.60.0. Install with `brew install likec4`. The diagram shows the design record to people. It holds no decision of its own (ruling R4).

## Commands

| Command | Use |
|---|---|
| `likec4 start architecture/` | Live preview in the browser, reloaded on every save. Offer it to the architect when one is present. |
| `likec4 validate architecture/` | Syntax and semantics. `scripts/c4check.py` runs it. |
| `likec4 export png architecture/` | Images, when asked. |

## Workspace layout

LikeC4 reads every `.c4` file under `architecture/` as one workspace and allows each element kind and tag to be defined once.

- `architecture/spec.c4`: the only `specification` block. Copy it from `assets/spec.c4`.
- `architecture/design.c4`: the system `designed`. Start it from `assets/design-template.c4`.
- View names start with the system id (`designed_…`).

## Conventions

- Each module's `description` starts with its record id and the decision it hides: `'M2. Hides D2: …'`.
- Each relation is a "uses" edge, titled `'uses: …'`. Its `entry` metadata names the record entry it pictures (`U1`). Calls that are not uses (Parnas 1979) may be drawn in a `dynamic view`, never as model relations.
- Tags: `#volatile` for a module that hides a decision likely to change, `#state_owner` for the one owner of a piece of mutable state, `#infrastructure` for facilities every level may use.

## Syntax that fails validation

- `tags ['volatile']` inside an element. Write `#volatile`.
- Short names in a `dynamic view`. Use the full path, `designed.framing`.
- Identifiers with `-` or a leading digit.
- Keywords as element names (`link`, `model`, `views`, `style`, `metadata`). Rename the element.
- An apostrophe inside a single-quoted title.
