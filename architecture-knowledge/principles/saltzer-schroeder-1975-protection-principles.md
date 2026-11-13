---
id: saltzer-schroeder-1975-protection-principles
title: Design principles for mechanisms that guard authority
source: J. H. Saltzer and M. D. Schroeder, "The Protection of Information in Computer Systems", Proceedings of the IEEE 63(9):1278–1308, September 1975, DOI 10.1109/PROC.1975.9939. Copy used https://web.mit.edu/Saltzer/www/publications/protection/ (the authors' HTML edition; section I.A.3 "Design Principles")
checked: every quote against the copy used
status: draft
related: parnas-1972-information-hiding, liskov-zilles-1974-abstract-data-types
---

## Source

- **Scope.** "Here are eight examples of design principles that apply particularly to protection mechanisms." Protection mechanisms are "the mechanisms that control access to information by executing programs." (*I.A.3, I.A.4*)
- **Status of the principles.** "these principles do not represent absolute rules--they serve best as warnings. If some part of a design violates a principle, the violation is a symptom of potential trouble, and the design should be carefully reviewed to be sure that the trouble has been accounted for or is unimportant." (*I.A.3*)
- **The eight principles** (*I.A.3*):
  - a) **Economy of mechanism.** "Keep the design as simple and small as possible." Because flaws that open unwanted access paths "will not be noticed during normal use", inspection is necessary, and "For such techniques to be successful, a small and simple design is essential."
  - b) **Fail-safe defaults.** "Base access decisions on permission rather than exclusion." "A design or implementation mistake in a mechanism that gives explicit permission tends to fail by refusing permission, a safe situation, since it will be quickly detected. On the other hand, a design or implementation mistake in a mechanism that explicitly excludes access tends to fail by allowing access, a failure which may go unnoticed in normal use."
  - c) **Complete mediation.** "Every access to every object must be checked for authority." It "forces a system-wide view of access control, which in addition to normal operation includes initialization, recovery, shutdown, and maintenance." It "requires that proposals to gain performance by remembering the result of an authority check be examined skeptically."
  - d) **Open design.** "The design should not be secret." The mechanisms "should not depend on the ignorance of potential attackers, but rather on the possession of specific, more easily protected, keys or passwords."
  - e) **Separation of privilege.** "Where feasible, a protection mechanism that requires two keys to unlock it is more robust and flexible than one that allows access to the presenter of only a single key." Separated keys "apply to any situation in which two or more conditions must be met before access should be permitted."
  - f) **Least privilege.** "Every program and every user of the system should operate using the least set of privileges necessary to complete the job." It "limits the damage that can result from an accident or error", and "if a mechanism can provide 'firewalls,' the principle of least privilege provides a rationale for where to install the firewalls."
  - g) **Least common mechanism.** "Minimize the amount of mechanism common to more than one user and depended on by all users." "Every shared mechanism (especially one involving shared variables) represents a potential information path between users and must be designed with great care".
  - h) **Psychological acceptability.** "It is essential that the human interface be designed for ease of use, so that users routinely and automatically apply the protection mechanisms correctly."

## Interpretation

The paper addresses protection mechanisms. We apply it, as an interpretation, to any part of a design that decides what may happen: what may be written, overwritten or deleted, which component may send to which, what a caller may do with a value. Those decisions are authority, whether or not anyone thinks of them as security.

1. **Every authority decision has one mechanism, small enough to inspect** (a, c). If the rule for *may this be changed?* is checked in several places, or scattered through the code that does the change, it cannot be inspected and it will drift. This is information hiding (Parnas 1972) applied to authority.
2. **Grant by explicit permission; deny by default** (b). A list of what is allowed fails safe when it is incomplete; a list of what is forbidden fails open.
3. **Check on every path, including the unusual ones** (c). Recovery, resumption, migration and maintenance paths are where checks are forgotten. A check remembered from an earlier step is a cache of authority and must be invalidated when the authority changes.
4. **Give each part only the authority its job needs** (f), and keep shared mechanisms that everyone depends on few and small (g).
5. **Use them as warnings, not rules**, as the authors say: a violation calls for a review, not an automatic rewrite.

## Application

**Questions for a design**
- What does this system decide about what may happen? For each such decision: which single mechanism makes it, and can it be read in one place?
- Is access granted by a list of what is allowed, or by a list of what is forbidden?
- Which paths reach the protected action (normal, recovery, resume, maintenance), and does each one pass the same check?
- Is any authority decision remembered (cached, recorded in a file, carried in a flag)? What invalidates it?
- Does any component hold more authority than its job needs?

**How a breach shows in code**
- Permission logic duplicated across call sites, or written as a growing list of exclusions.
- A recovery or retry path that skips the checks of the normal path.
- A stored *already authorized* marker with no rule for when it stops being valid.
- A shared helper or global state that every component depends on and that decides authority for all of them.

**In this repository** (read 2026-11).
- *Fail-safe defaults.* Overwriting or deleting a file requires the approval record to name the exact path, change and content digest (`crates/l2r_apply/src/approval.rs`, `Approval::authorizes`); anything not named is refused.
- *Complete mediation on the recovery path.* Generation and recovery pass every step through the same check, `Journal::authorize` (`crates/l2r_apply/src/journal.rs`). It holds the transition table and the approval record's entry for the file content. Generation runs it on its own journal before any write; `recover` runs it on the journal read back from disk before doing anything (`crates/l2r_apply/src/transaction.rs`). One rule, checked on every path.

## Rulings

None specific to this principle.
