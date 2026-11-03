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
- **Untrusted state confers no authority.** Every write, replacement and
  deletion must be justified by the approved plan and a permitted transition. A
  journal can only *repeat* such a justification; it cannot add one (G7, G12).
  The lock is **evidence**, not authority: it tells L2R what it believes it wrote
  earlier, and so what to show a reviewer. It can make generation refuse, but it
  cannot make generation destroy anything. **Replacing or deleting an existing
  file requires the approval record to name that file and its exact content
  digest** (G14). The record lives beside the model, outside the output tree, and
  is reviewed like the model.
- **What is trusted.** The trust boundary is the repository's review process, not
  a cryptographic one: whoever can change the model, the Lean inventory and the
  approval record can direct generation. Who may do that is a governance decision
  ([`governance.md`](governance.md)). L2R adds no signing and no key management.
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
| G7 | Recovery has no authority beyond generation of the same approved plan from the same recorded prior lock. | `l2r recover` requires a current approval for the plan the journal names. The journal records the digest of the exact lock bytes it was assessed against; steps run only while that lock is still on disk. Each step must be a permitted transition (`Journal::authorize`, shared with `generate`): *create* only a planned path, with the plan's ownership and content; *replace* only a planned generated path whose prior lock entry is generated with exactly the step's `before` digest; *delete* only a no-longer-planned path the prior lock records as generated with that digest. Ownership must agree with the plan and the prior lock. Lost temporaries are re-staged from the plan itself. If the plan's own lock is already on disk, no step runs at all. | `faults.rs`: escaping path; forged `generated` and `implementation` ownership of an implementation file; forged ownership of a new file; forged `before` over an edited generated file; deletion of a never-recorded file and of an orphaned implementation file; prior lock swapped after the interruption; forged steps after the lock was written; `v0` journal; injected content; other plan; missing approval; lost temporaries |
| G8 | Implementation files are never replaced or deleted. | Creates use `linkat`, which never replaces an existing file. A step may replace or delete only a path the plan or prior lock records as generated (G7), and a file is deleted only if it carries L2R's generated-file marker (G13). If an implementer creates the file first, their file wins over the stub. | `faults.rs`: concurrent implementation file; forged journals; `hostile.rs`: forged lock entry; `evolution.rs` |
| G9 | A file that changed unexpectedly is never overwritten. | Each step re-reads the target immediately before acting and proceeds only if its digest is the expected before (or after) state. Otherwise the run stops with the journal in place. | `faults.rs`: edit during interruption |
| G10 | Two generations cannot interleave. | The journal is created exclusively (`linkat`); while it exists, `generate` refuses. | `faults.rs`: generate refused while a journal is pending |
| G11 | An approval applies to exactly one plan. | Writes require a record whose digest equals the current plan digest. The digest covers the model, the Lean inventory, the generator, the profiles and every file's content. | `safety.rs`, `evolution.rs`, `workflow.rs` |
| G12 | A lock is written only when it is true. | Before `generate` or `recover` writes the lock, a final-state check confirms that every planned generated file has its planned content, every planned implementation file exists, and no generated file of the prior lock that the plan no longer contains remains. A journal with omitted steps therefore stops with the journal in place instead of recording the plan as applied. | `faults.rs`: every step omitted; remaining steps omitted |
| G13 | L2R deletes only files that declare themselves generated. | Every generated file starts with an `@generated by L2R (` marker line; the planner refuses to plan one without it (`l2r_plan::declares_generated`, one shared definition). Assessment reports a conflict, and a commit step refuses, for a deletion whose target lacks the marker, whatever the lock records. | `hostile.rs`: forged lock entry naming a foreign file; `faults.rs`: consistently forged lock and journal |
| G14 | A forged or edited lock cannot authorize replacing or deleting a file. | `l2r approve` records, from the tree the reviewer's preview showed, every replacement and deletion as `{path, change, before-digest}` in `authorized_changes` of the `l2r.approval/v1` record. `apply` and `recover` refuse any replace or delete not named there with the digest the file has *now*; the journal check repeats this per step. The lock still has to agree (an edited file is refused as before), but agreeing is no longer sufficient. | `hostile.rs`: consistently forged marker, lock entry and digest; forged digest over a hand edit; a record that lacks the change; the reviewed path that does authorize it; an older record format |

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
- **A forged lock.** A forged lock cannot cause any write, replacement or
  deletion (G14). It can cause refusals, which are resolved by restoring the lock
  from review or by a reviewed migration (below). What remains is what the
  approval record's reviewers must catch: the preview lists each replacement and
  deletion with the digest of the content that would be destroyed; approving it
  without reading it authorizes it. The record's `recorded_by` is unverified text.
- **Content, not provenance, of a file at a listed path.** An authorized change
  names a file by path and content digest. Any file with that digest at that path
  may be replaced, whoever put it there. The record cannot tell L2R's output from
  an identical copy; it does not need to, because the reviewer approved
  destroying exactly that content.
- **Lost lock, no migration.** If the lock is missing or invalid, nothing it would
  have described is touched. A file at a planned path that no valid lock
  describes is "a file L2R did not generate" and generation refuses. The way out
  is a reviewed migration: delete or move the file yourself, or commit a corrected
  lock, and generate again. There is deliberately no flag that overrides this.
- **Moved-implementation refusal during recovery.** Generation refuses to create
  a fresh implementation stub while an implementation of the same name is
  orphaned. Recovery does not repeat that usability check. It may create a
  planned stub at an absent planned path, which the plan authorizes.
- **Recovery after the model changes.** `recover` only finishes the plan the
  journal names. If the model has changed since, restore it (the journal states
  the plan digest) or resolve by hand: delete the journal and the `.l2r-tmp.*`
  files it lists, then deal with the reported conflicts.
- **Platforms.** The boundary uses `openat`-family calls and is Unix-only.
  `linkat` requires a filesystem with hard links.
- **Journals from earlier versions.** A `l2r.journal/v0` journal (without a
  recorded prior lock) is refused. Finish it with the version that wrote it, or
  resolve by hand.
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

Since `l2r.approval/v1`, a record also carries `authorized_changes`: the
replacements and deletions of existing generated files that the reviewer's
preview showed, each with the digest of the content destroyed. Committing such a
record is how a team authorizes a migration; reviewing its diff shows exactly
which files are overwritten or removed. A record made against an up-to-date tree
lists nothing. When the plan changes, run `preview`, read the "permission to
destroy existing content" section, then `approve`. The list is consulted only at
the moment of writing; once the tree is current the entries are historical.
See [`governance.md`](governance.md) for the review rules this assumes.

## Implementation moves

Generation refuses whenever it would create a fresh implementation stub while
an implementation file with the same name is being orphaned, and names both
paths. This covers a renamed `rust_module`, a component moved under a new parent,
a renamed component id, and combinations of these. The alternative, creating a
fresh stub while the real work lies orphaned, invites lost or duplicated work.
Move the file (`git mv`), or delete it if it really is obsolete, and generate
again. When the orphan belongs to the same component, the message says it
moved. Otherwise it says it may have been renamed. (`evolution.rs`)
