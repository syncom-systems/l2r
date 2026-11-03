# Governance: who may change what

L2R is independent of any code-hosting provider. It records what was reviewed
(`l2r.approval/v1`) and refuses to write anything the record does not cover. It
cannot know who reviewed. That belongs to the repository's review process, which
this document describes in provider-neutral terms and then maps to one common
mechanism.

## Trust classes

| Class | Examples | Rule |
| --- | --- | --- |
| **Reviewed authority** | architecture model, Lean specifications and inventory, approval records, generation profiles and the generator | Changes need review by a human owner. These decide what L2R may write and destroy. |
| **Reviewed evidence** | committed `l2r.lock.json` | Review the diff as evidence of what was generated. It is not an authority; L2R never lets it authorize replacement or deletion. |
| **Untrusted** | anything in the output tree at run time (the working-tree lock, the journal, implementation files) | Treated as hostile input. Never confers authority. |

## Proposed mapping: CODEOWNERS

Where the host supports a CODEOWNERS file with required owner review, a
repository using L2R should require review of the reviewed-authority paths. A
proposal (for the repository owner to adopt, adjusting owners):

```
# Architecture, behavior specifications and what was approved to be written
/examples/*/architecture.toml          @OWNER
/examples/*/architecture.approval.json @OWNER
/examples/*/lean-inventory.json        @OWNER
/examples/*/spec/                      @OWNER
/lean/                                 @OWNER
/docs/                                 @OWNER

# Evidence: reviewed, but never an authority
/examples/*/scaffold/l2r.lock.json     @OWNER
```

This file is intentionally not committed as `.github/CODEOWNERS`: enabling it
requires naming real owners and turning on required review, which is the owner's
decision.

## What this does and does not give

- A forged working-tree lock cannot direct destruction, whatever the review
  setup (see `generation-boundary.md`, G14).
- Review of approval records is what makes an `authorized_changes` entry mean
  something. Without it, an agent that can run `l2r approve` can authorize
  anything its own preview shows. L2R states this and does not hide it.
- No signing, no key management and no identity system is part of L2R.
