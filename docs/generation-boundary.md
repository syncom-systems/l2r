# The generation boundary: what L2R guarantees when it writes

This document states exactly what `l2r generate` and `l2r recover` guarantee,
under which assumptions, and which tests demonstrate each claim. If this text and
the tests disagree, the tests are authoritative and this text is a bug.

## Threat model

- The **architecture model, the Lean inventory and the approval record** are
  authored inputs. They are validated, but anyone who can edit them controls
  what is planned. That is why approval records must be governed by repository
  review (see [Approval records](#approval-records)).
- The **output tree** (the model's `output_dir`) is shared with implementers and
  agents. Its contents, including `l2r.lock.json` and `l2r.journal.json`, are
  **untrusted input**. They may be edited, malformed, or replaced by symbolic links.
- L2R does not defend against another process that is actively racing it with
  write access to the same tree. It does guarantee that such a process cannot make
  L2R write or delete **outside** the tree, overwrite an implementation file, or
  overwrite a file whose content L2R did not expect.

## Guarantees

| # | Guarantee | Mechanism | Tests |
| --- | --- | --- | --- |
| G1 | No file outside the output tree is read, written or deleted. | All tree access goes through one handle (`crates/l2r_apply/src/tree.rs`). It opens the root once, then resolves every path one segment at a time relative to directory descriptors (`openat` with `O_DIRECTORY \| O_NOFOLLOW`). A symbolic link anywhere below the root is refused, including one that appears after inspection. | `hostile.rs`: symlinked output dirs; `faults.rs`: a directory swapped for a symlink mid-generation; `safety.rs`: symlinked root |
| G2 | Every path that reaches the filesystem is validated, whatever its source. | Plan artifacts, lock entries and journal steps hold a `RelPath`, which can only be built through `RelPath::parse`: relative, no `.`/`..`/hidden segments, ASCII `[A-Za-z0-9_.-]`, length limits, not a reserved state-file name. Lock and journal parsing also reject duplicate and case-colliding paths. | `l2r_model` path tests; `hostile.rs`: `../`, absolute and self-referencing lock entries; `check` cannot probe outside files |
| G3 | Temporary files cannot be redirected. | Temporaries are created with `O_CREAT \| O_EXCL \| O_NOFOLLOW` under a per-run name. A planted file or link with that name causes a refusal, never a write through it. | `faults.rs`: links planted at the exact temporary names a run will use (the fault build fixes the run id); fails if the no-follow, exclusive create is removed |
| G4 | Conflicts are detected before anything is written. | Every action is assessed before staging. Conflicts include: a symlink, a non-file, a foreign file, an edited generated file, an invalid lock, a pending journal, an ownership change, a case collision, and a moved implementation. | `safety.rs`, `evolution.rs` |
| G5 | A staging failure changes no file. | New content is staged into temporaries first; on failure they are removed. New, empty parent directories may remain. | `faults.rs`: staging failure at each file |
| G6 | An interruption at any point leaves each file in its old or its new state, and the run can be finished. | Each step is a single `linkat` (create), `renameat` (replace) or `unlinkat` (delete). The journal, written before the first step, names the plan and every step's before/after digests. | `faults.rs`: interruption after every step 0…n+1; `crash.rs`: a real `SIGABRT` followed by the real `l2r recover` |
| G7 | Recovery writes only the approved plan's content. | `l2r recover` requires a current approval for the plan the journal names. It checks the journal's lock and every step's digest against that plan, and re-stages lost or damaged temporaries from the plan itself. | `faults.rs`: tampered journal (escaping path, forged implementation replace, injected content, other plan, missing approval); lost temporaries |
| G8 | Implementation files are never replaced or deleted. | Creates use `linkat`, which never replaces an existing file. A journal step may replace or delete only a generated file. If an implementer creates the file first, their file wins over the stub. | `faults.rs`: concurrent implementation file; tampered journal; `evolution.rs` |
| G9 | A file that changed unexpectedly is never overwritten. | Each step re-reads the target immediately before acting and proceeds only if its digest is the expected before (or after) state. Otherwise the run stops with the journal in place. | `faults.rs`: edit during interruption |
| G10 | Two generations cannot interleave. | The journal is created exclusively (`linkat`); while it exists, `generate` refuses. | `faults.rs`: generate refused while a journal is pending |
| G11 | An approval applies to exactly one plan. | Writes require a record whose digest equals the current plan digest. The digest covers the model, the Lean inventory, the generator, the profiles and every file's content. | `safety.rs`, `evolution.rs`, `workflow.rs` |

## What is not guaranteed

- **Transactional atomicity across files.** Between steps, a reader can observe a
  mix of old and new files. The guarantee is *roll-forward recoverability*
  (G6, G7), not isolation.
- **Durability across power loss.** Files and directories are `fsync`ed at each
  step, but whether that survives power loss depends on the filesystem and
  hardware honouring it. Process interruption is tested; power loss is not.
- **Races with an active writer.** The check of a target's digest and the
  subsequent rename are two system calls. A concurrent writer can change the file
  between them; the effect is confined to the tree (G1) and to generated files (G8).
- **Recovery after the model changes.** `recover` only finishes the plan the
  journal names. If the model has changed since, restore it (the journal states
  the plan digest) or resolve by hand: delete the journal and the `.l2r-tmp.*`
  files it lists, then deal with the reported conflicts.
- **Platforms.** The boundary uses `openat`-family calls and is Unix-only.
  `linkat` requires a filesystem with hard links.
- **Leftover temporaries after a crash during staging** (before the journal
  exists) are not cleaned up automatically. Their names start with `.l2r-tmp.`
  and they are never read.

## Approval records

`l2r approve --digest D --by NAME` writes a record **only if** `D` is the
current plan digest. The record proves that *someone* recorded `D`; `recorded_by`
is unverified text, and an agent can run `l2r approve` as easily as a person.
**A record is not authorization.** Authorization has to come from the
repository's governance, for example a required human review of changes to
`*.approval.json`. That is a maintainer decision, discussed in ADR-0003. The
example's record is a fixture recorded by an agent, and says so.

## Implementation moves

Generation refuses whenever it would create a fresh implementation stub while
an implementation file with the same name is being orphaned, and names both
paths. This covers a renamed `rust_module`, a component moved under a new parent,
a renamed component id, and combinations of these. The alternative, creating a
fresh stub while the real work lies orphaned, invites lost or duplicated work.
Move the file (`git mv`), or delete it if it really is obsolete, and generate
again. When the orphan belongs to the same component, the message says it
moved. Otherwise it says it may have been renamed. (`evolution.rs`)
