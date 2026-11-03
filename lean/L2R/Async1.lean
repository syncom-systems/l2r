import L2R.Async

/-!
# Asynchronous composition with bounded mailboxes and typed ports (`l2r.async/v1`)

This file is L2R's generic semantics for the `composite/v1` profile. It is a new
version beside `l2r.async/v0` (`lean/L2R/Async.lean`), which is unchanged. It
defines **what behaviour an approved composition permits** independently of any
scheduler, and a model of the scheduler that `composite/v1` generates, with the
theorem that every run of that scheduler is a permitted behaviour.

## What is different from `l2r.async/v0`

| Concern | v1 decision |
| --- | --- |
| Mailbox capacity | Every child has a capacity (`Network.cap`), part of the *semantics*. Every reachable configuration respects it (`Reachable.bounded`). |
| Overflow | Never discards. A delivery is **enabled only if every mailbox that receives one of its outputs has room** for all of them (backpressure, still atomic). A full target therefore *blocks* the sender. |
| Rejection at the boundary | A port send is permitted only if its target mailbox has room. The generated API returns an explicit rejection otherwise; the caller keeps the message. |
| Blocking | Visible, never silent. A configuration with pending messages and no enabled delivery is `Blocked`; the scheduler reports it as such (`Progress.blocked`) rather than as settled. Whether a system can reach it is a *property to prove or refute*. |
| External interface | Typed ports (`Network.Msg`, `target`, `inject`): the only way the environment can add a message, and each carries a Lean type and a total translation to the target's input. The structural contract of the interface is the types. |
| Admissibility | `Env`, an explicit parameter of every system theorem: a predicate on the *current configuration* and the message saying when the environment may send. It is an assumption about the world, kept apart from the structural contract. |
| Fairness | Not part of the semantics. Liveness theorems take explicit fairness hypotheses over traces (see `Trace`, `WeaklyFair`). |
| Failure | Still outside the model: loss, duplication, reordering, crashes, time. These are omissions, not assumptions. |

Messages produced by a translation that returns `none` are not lost to
overflow: `none` is the translation's own, Lean-visible meaning for "this
output means nothing to the receiver". No other path removes a message.

**Trust:** the correspondence between `RoundRobin` and the generated Rust is
established by Rust-to-Lean refinement proofs for the parts listed in
`docs/verification.md` and by bounded conformance testing for the rest, not by
this file.
-/
namespace L2R.Async1

open L2R.Async (update update_same update_other)

/-- An approved composition: children `0 … size-1` in identifier order,
mailbox capacities, and typed external ports. -/
structure Network where
  size : Nat
  State : Fin size → Type
  Input : Fin size → Type
  Output : Fin size → Type
  init : (c : Fin size) → State c
  step : (c : Fin size) → State c → Input c → State c × Option (Output c)
  /-- The messages one output becomes, in approved-edge order. -/
  route : (c : Fin size) → Output c → List ((d : Fin size) × Input d)
  /-- Mailbox capacity of each child. -/
  cap : Fin size → Nat
  /-- Number of input ports. -/
  ports : Nat
  /-- The Lean type of the messages a port accepts. -/
  Msg : Fin ports → Type
  /-- The child a port feeds. -/
  target : Fin ports → Fin size
  /-- The meaning of a port message to its target. Total: a port message is never discarded. -/
  inject : (p : Fin ports) → Msg p → Input (target p)

variable (N : Network)

/-- Every child's owned state and mailbox. -/
structure Config where
  state : (c : Fin N.size) → N.State c
  inbox : (c : Fin N.size) → List (N.Input c)

/-- Every child in its initial state, with empty mailboxes. -/
def initial : Config N := ⟨N.init, fun _ => []⟩

/-- No mailbox exceeds its capacity. -/
def Bounded (k : Config N) : Prop := ∀ c, (k.inbox c).length ≤ N.cap c

/-- Appends one message to its receiver's mailbox. -/
def post (k : Config N) (m : (d : Fin N.size) × N.Input d) : Config N :=
  ⟨k.state, update k.inbox m.1 (k.inbox m.1 ++ [m.2])⟩

/-- The messages a (possibly absent) output becomes. -/
def emitted (c : Fin N.size) : Option (N.Output c) → List ((d : Fin N.size) × N.Input d)
  | none => []
  | some o => N.route c o

/-- The messages in `ms` addressed to `d`, in order. -/
def msgsFor (d : Fin N.size) (ms : List ((d : Fin N.size) × N.Input d)) : List (N.Input d) :=
  ms.filterMap fun m => if h : m.1 = d then some (h ▸ m.2) else none

/-- Every mailbox of `k` has room for the messages `ms` addressed to it. -/
def fits (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) : Bool :=
  (List.finRange N.size).all fun d =>
    decide ((k.inbox d).length + (msgsFor N d ms).length ≤ N.cap d)

theorem fits_iff (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) :
    fits N k ms = true ↔ ∀ d, (k.inbox d).length + (msgsFor N d ms).length ≤ N.cap d := by
  simp [fits, List.all_eq_true]

/-- Atomic delivery of the oldest message in `c`'s mailbox.

`none` when the mailbox is empty **or when a receiving mailbox lacks room** for
the outputs: the delivery is then not enabled, and nothing changes. -/
def deliver (k : Config N) (c : Fin N.size) : Option (Config N) :=
  match k.inbox c with
  | [] => none
  | i :: rest =>
    let r := N.step c (k.state c) i
    let k1 : Config N := ⟨update k.state c r.1, update k.inbox c rest⟩
    if fits N k1 (emitted N c r.2) then
      some ((emitted N c r.2).foldl (post N) k1)
    else none

theorem foldl_post_state (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) :
    (ms.foldl (post N) k).state = k.state := by
  induction ms generalizing k with
  | nil => rfl
  | cons m ms ih => simp only [List.foldl]; rw [ih]; rfl

theorem foldl_post_inbox (k : Config N) (ms : List ((d : Fin N.size) × N.Input d)) (d : Fin N.size) :
    (ms.foldl (post N) k).inbox d = k.inbox d ++ msgsFor N d ms := by
  induction ms generalizing k with
  | nil => simp [msgsFor]
  | cons m ms ih =>
    simp only [List.foldl]
    rw [ih]
    obtain ⟨e, i⟩ := m
    by_cases h : e = d
    · subst h
      simp [post, msgsFor]
    · have h' : d ≠ e := fun x => h x.symm
      simp [post, msgsFor, h, update_other _ _ h']

/-- What an enabled delivery does, observed child by child: only `c`'s state
changes, by its own `step`; `c` loses its oldest message; each child receives,
in order, **every** message addressed to it, and there was room for all of them. -/
theorem deliver_some {k k' : Config N} {c : Fin N.size} (h : deliver N k c = some k') :
    ∃ i rest, k.inbox c = i :: rest ∧
      k'.state = update k.state c (N.step c (k.state c) i).1 ∧
      (∀ d, k'.inbox d = update k.inbox c rest d ++ msgsFor N d (emitted N c (N.step c (k.state c) i).2)) ∧
      (∀ d, (update k.inbox c rest d).length + (msgsFor N d (emitted N c (N.step c (k.state c) i).2)).length ≤ N.cap d) := by
  unfold deliver at h
  split at h
  · contradiction
  · rename_i i rest hin
    dsimp only at h
    split at h
    · rename_i hfit
      injection h with h
      subst h
      exact ⟨i, rest, hin, foldl_post_state N _ _, fun d => foldl_post_inbox N _ _ d,
        (fits_iff N _ _).mp hfit⟩
    · contradiction

/-- A delivery is not enabled exactly when the mailbox is empty or a receiver is full. -/
theorem deliver_none_iff (k : Config N) (c : Fin N.size) :
    deliver N k c = none ↔
      k.inbox c = [] ∨ ∃ i rest, k.inbox c = i :: rest ∧
        ¬ fits N ⟨update k.state c (N.step c (k.state c) i).1, update k.inbox c rest⟩
            (emitted N c (N.step c (k.state c) i).2) = true := by
  unfold deliver
  split
  · rename_i h; simp [h]
  · rename_i i rest h
    dsimp only
    constructor
    · intro hn
      right
      refine ⟨i, rest, h, ?_⟩
      intro hf
      simp [hf] at hn
    · rintro (h1 | ⟨i', rest', h2, h3⟩)
      · rw [h] at h1; cases h1
      · rw [h] at h2
        injection h2 with e1 e2
        subst e1; subst e2
        simp [h3]

theorem deliver_empty {k : Config N} {c : Fin N.size} (h : k.inbox c = []) : deliver N k c = none := by
  unfold deliver; rw [h]

/-- **No overflow.** A delivery keeps every mailbox within its capacity. -/
theorem deliver_bounded {k k' : Config N} {c : Fin N.size} (h : deliver N k c = some k') :
    Bounded N k' := by
  obtain ⟨i, rest, _, _, hin, hfit⟩ := deliver_some N h
  intro d
  rw [hin d]
  simpa [List.length_append] using hfit d

theorem post_state (k : Config N) (m : (d : Fin N.size) × N.Input d) : (post N k m).state = k.state := rfl

/-- The environment's admissibility predicate: when it may send a port message,
given what the system currently holds. It is an assumption about the world.
`anyEnv` is the weakest one: whenever the port has room. -/
abbrev Env := Config N → (p : Fin N.ports) → N.Msg p → Prop

/-- The environment may always send (if there is room). -/
def anyEnv : Env N := fun _ _ _ => True

/-- A port send: the message becomes an input of the port's target. -/
def inject (k : Config N) (p : Fin N.ports) (m : N.Msg p) : Config N :=
  post N k ⟨N.target p, N.inject p m⟩

/-- Labels of permitted steps. -/
inductive Label where
  | deliver (c : Fin N.size)
  | send (p : Fin N.ports) (m : N.Msg p)

/-- One permitted step, with its label: a delivery to a child, or a port send
that the environment may make **and the target has room for**. -/
inductive StepL (E : Env N) : Config N → Label N → Config N → Prop
  | deliver {k k' : Config N} (c : Fin N.size) :
      deliver N k c = some k' → StepL E k (.deliver c) k'
  | send (k : Config N) (p : Fin N.ports) (m : N.Msg p) :
      E k p m → (k.inbox (N.target p)).length < N.cap (N.target p) →
      StepL E k (.send p m) (inject N k p m)

/-- A permitted step. -/
def Step (E : Env N) (k k' : Config N) : Prop := ∃ l, StepL N E k l k'

/-- Configurations some permitted behaviour reaches. -/
inductive Reachable (E : Env N) : Config N → Prop
  | init : Reachable E (initial N)
  | step {k k' : Config N} : Reachable E k → Step N E k k' → Reachable E k'

/-- `k'` follows `k` by permitted steps. -/
inductive Star (E : Env N) : Config N → Config N → Prop
  | refl (k : Config N) : Star E k k
  | tail {k k' k'' : Config N} : Star E k k' → Step N E k' k'' → Star E k k''

theorem initial_bounded (_h : ∀ c, 0 ≤ N.cap c) : Bounded N (initial N) := by
  intro c; simp [initial]

theorem inject_bounded {k : Config N} (hb : Bounded N k) (p : Fin N.ports) (m : N.Msg p)
    (hroom : (k.inbox (N.target p)).length < N.cap (N.target p)) : Bounded N (inject N k p m) := by
  intro d
  by_cases h : d = N.target p
  · subst h; simp [inject, post]; omega
  · have h' : N.target p ≠ d := fun x => h x.symm
    simp [inject, post, update_other _ _ h]
    exact hb d

/-- **No overflow, ever.** Every permitted step preserves every capacity. -/
theorem step_bounded (E : Env N) {k k' : Config N} (hb : Bounded N k) (h : Step N E k k') :
    Bounded N k' := by
  obtain ⟨l, hl⟩ := h
  cases hl with
  | deliver c hd => exact deliver_bounded N hd
  | send p m _ hroom => exact inject_bounded N hb p m hroom

/-- Every reachable configuration respects every mailbox capacity. -/
theorem Reachable.bounded (E : Env N) {k : Config N} (h : Reachable N E k) : Bounded N k := by
  induction h with
  | init => exact initial_bounded N (fun _ => Nat.zero_le _)
  | step _ hs ih => exact step_bounded N E ih hs

/-- An inductive invariant of the permitted steps holds in every reachable configuration. -/
theorem reachable_invariant (E : Env N) (P : Config N → Prop) (h0 : P (initial N))
    (hs : ∀ k k', Reachable N E k → P k → Step N E k k' → P k') :
    ∀ k, Reachable N E k → P k := by
  intro k hk
  induction hk with
  | init => exact h0
  | step hr hst ih => exact hs _ _ hr ih hst

/-- A property preserved by every permitted step is kept by every continuation. -/
theorem star_invariant (E : Env N) (P : Config N → Prop)
    (hs : ∀ k k', P k → Step N E k k' → P k') :
    ∀ k k', P k → Star N E k k' → P k' := by
  intro k k' hk hstar
  induction hstar with
  | refl => exact hk
  | tail _ hst ih => exact hs _ _ ih hst

/-- Pending messages exist but no delivery is enabled: every non-empty mailbox is
blocked by a full receiver. The system is stuck until the environment changes
nothing (it cannot drain a mailbox). -/
def Blocked (k : Config N) : Prop :=
  (∃ c, k.inbox c ≠ []) ∧ ∀ c, deliver N k c = none

/-- Nothing is pending. -/
def Settled (k : Config N) : Prop := ∀ c, k.inbox c = []

/-!
## Executions, for liveness

A *trace* is an infinite sequence of permitted labelled steps. Liveness needs
fairness assumptions about which traces occur; they are **hypotheses** of a
theorem, never part of the semantics.
-/

/-- An infinite behaviour. -/
structure Trace (E : Env N) where
  cfg : Nat → Config N
  lab : Nat → Label N
  valid : ∀ t, StepL N E (cfg t) (lab t) (cfg (t + 1))

/-- Weak fairness of deliveries: a delivery that stays enabled from some time on
is eventually taken. -/
def WeaklyFair {E : Env N} (tr : Trace N E) : Prop :=
  ∀ t (c : Fin N.size), (∀ s, t ≤ s → (deliver N (tr.cfg s) c).isSome) →
    ∃ s, t ≤ s ∧ tr.lab s = .deliver c

/-- A set of labels is taken again and again (an assumption about the
environment, e.g. "timer expiries keep arriving"). -/
def Recurs {E : Env N} (tr : Trace N E) (q : Label N → Prop) : Prop :=
  ∀ t, ∃ s, t ≤ s ∧ q (tr.lab s)

/-!
### Fair termination by a ranking with helpful actions

If, while a condition `P` holds, a ranking `μ` never increases except by leaving
`P`, and from every configuration some *helpful* label is eventually taken (by
weak fairness of a delivery that stays enabled, or because the environment
recurs) and lowers the rank, then `P` cannot hold forever.
-/
theorem fair_termination {E : Env N} (tr : Trace N E) (hwf : WeaklyFair N tr)
    (q : Label N → Prop) (hq : Recurs N tr q)
    (P : Config N → Prop) (μ : Config N → Nat) (helper : Config N → Option (Fin N.size))
    -- while helped by a delivery `c`: other steps keep `c` helpful and enabled, or lower the rank
    (hpersist : ∀ s c, P (tr.cfg s) → helper (tr.cfg s) = some c → tr.lab s ≠ .deliver c →
        ¬ P (tr.cfg (s + 1)) ∨ μ (tr.cfg (s + 1)) < μ (tr.cfg s) ∨
        (μ (tr.cfg (s + 1)) = μ (tr.cfg s) ∧ helper (tr.cfg (s + 1)) = some c ∧
          (deliver N (tr.cfg (s + 1)) c).isSome))
    (henabled : ∀ s c, P (tr.cfg s) → helper (tr.cfg s) = some c → (deliver N (tr.cfg s) c).isSome)
    (hdone : ∀ s c, P (tr.cfg s) → helper (tr.cfg s) = some c → tr.lab s = .deliver c →
        ¬ P (tr.cfg (s + 1)) ∨ μ (tr.cfg (s + 1)) < μ (tr.cfg s))
    -- while helped by the environment (`helper = none`): other steps keep that, or lower the rank
    (hpersist_env : ∀ s, P (tr.cfg s) → helper (tr.cfg s) = none → ¬ q (tr.lab s) →
        ¬ P (tr.cfg (s + 1)) ∨ μ (tr.cfg (s + 1)) < μ (tr.cfg s) ∨
        (μ (tr.cfg (s + 1)) = μ (tr.cfg s) ∧ helper (tr.cfg (s + 1)) = none))
    (hdone_env : ∀ s, P (tr.cfg s) → helper (tr.cfg s) = none → q (tr.lab s) →
        ¬ P (tr.cfg (s + 1)) ∨ μ (tr.cfg (s + 1)) < μ (tr.cfg s)) :
    ∀ t, P (tr.cfg t) → ∃ s, t ≤ s ∧ ¬ P (tr.cfg s) := by
  intro t0 hP0
  refine Classical.byContradiction fun hcon => ?_
  -- P holds at every time from t0 on
  have hall : ∀ s, t0 ≤ s → P (tr.cfg s) := fun s hs =>
    Classical.byContradiction fun h => hcon ⟨s, hs, h⟩
  -- Every time has a later time of strictly smaller rank.
  have descent : ∀ s, t0 ≤ s → ∃ s', s ≤ s' ∧ μ (tr.cfg s') < μ (tr.cfg s) := by
    intro s hs
    refine Classical.byContradiction fun hno0 => ?_
    have hno : ∀ s', s ≤ s' → ¬ μ (tr.cfg s') < μ (tr.cfg s) := fun s' h1 h2 => hno0 ⟨s', h1, h2⟩
    -- until the helper acts, the rank is constant and the helper is unchanged
    have stay : ∀ n, μ (tr.cfg (s + n)) = μ (tr.cfg s) ∧ helper (tr.cfg (s + n)) = helper (tr.cfg s) ∧
        (∀ c, helper (tr.cfg s) = some c → (deliver N (tr.cfg (s + n)) c).isSome) ∧
        (∀ m, m < n → (match helper (tr.cfg s) with
          | some c => tr.lab (s + m) ≠ .deliver c
          | none => ¬ q (tr.lab (s + m)))) := by
      intro n
      induction n with
      | zero =>
        refine ⟨rfl, rfl, fun c hc => henabled s c (hall s hs) hc, fun m hm => absurd hm (Nat.not_lt_zero _)⟩
      | succ n ih =>
        obtain ⟨hmu, hhelp, hen, hnot⟩ := ih
        have hPn := hall (s + n) (by omega)
        -- the helper has not acted at step n: otherwise the rank would drop
        cases hh : helper (tr.cfg s) with
        | some c =>
          have hh' : helper (tr.cfg (s + n)) = some c := by rw [hhelp, hh]
          have hnotc : tr.lab (s + n) ≠ .deliver c := by
            intro hlab
            rcases hdone (s + n) c hPn hh' hlab with h | h
            · exact h (hall _ (by omega))
            · have := hno (s + n + 1) (by omega); omega
          rcases hpersist (s + n) c hPn hh' hnotc with h | h | ⟨h1, h2, h3⟩
          · exact absurd (hall _ (by omega)) h
          · have := hno (s + n + 1) (by omega); omega
          · rw [show s + (n + 1) = s + n + 1 from rfl]
            refine ⟨by omega, ?_, ?_, ?_⟩
            · exact h2
            · intro c' hc'
              injection hc' with hc'; subst hc'
              exact h3
            · intro m hm
              rcases Nat.lt_succ_iff_lt_or_eq.mp hm with h | h
              · have := hnot m h
                rw [hh] at this
                exact this
              · subst h; exact hnotc
        | none =>
          have hh' : helper (tr.cfg (s + n)) = none := by rw [hhelp, hh]
          have hnotq : ¬ q (tr.lab (s + n)) := by
            intro hlab
            rcases hdone_env (s + n) hPn hh' hlab with h | h
            · exact h (hall _ (by omega))
            · have := hno (s + n + 1) (by omega); omega
          rcases hpersist_env (s + n) hPn hh' hnotq with h | h | ⟨h1, h2⟩
          · exact absurd (hall _ (by omega)) h
          · have := hno (s + n + 1) (by omega); omega
          · rw [show s + (n + 1) = s + n + 1 from rfl]
            refine ⟨by omega, ?_, ?_, ?_⟩
            · exact h2
            · intro c hc; contradiction
            · intro m hm
              rcases Nat.lt_succ_iff_lt_or_eq.mp hm with h | h
              · have := hnot m h
                rw [hh] at this
                exact this
              · subst h; exact hnotq
    -- now the helper must act, which lowers the rank: contradiction
    cases hh : helper (tr.cfg s) with
    | some c =>
      have hwf' := hwf s c (fun u hu => by
        obtain ⟨_, _, hen, _⟩ := stay (u - s)
        have := hen c hh
        rwa [show s + (u - s) = u by omega] at this)
      obtain ⟨u, hu, hlab⟩ := hwf'
      have ⟨_, _, _, hnot⟩ := stay (u - s + 1)
      have := hnot (u - s) (by omega)
      rw [hh] at this
      simp only [show s + (u - s) = u by omega] at this
      exact this hlab
    | none =>
      obtain ⟨u, hu, hlab⟩ := hq s
      have ⟨_, _, _, hnot⟩ := stay (u - s + 1)
      have := hnot (u - s) (by omega)
      rw [hh] at this
      simp only [show s + (u - s) = u by omega] at this
      exact this hlab
  -- infinite strict descent in ℕ is impossible
  have : ∀ r, ∀ s, t0 ≤ s → μ (tr.cfg s) ≤ r → False := by
    intro r
    induction r with
    | zero =>
      intro s hs hr
      obtain ⟨s', _, hlt⟩ := descent s hs
      omega
    | succ r ih =>
      intro s hs hr
      obtain ⟨s', hs', hlt⟩ := descent s hs
      exact ih s' (by omega) (by omega)
  exact this _ t0 (Nat.le_refl _) (Nat.le_refl _)

/-!
## The `composite/v1` scheduler

A model of the generated `System` (one composite whose children are all
machines), written to mirror the generated Rust:

- `System::step_once` tries `child = (next + offset) % n` for `offset` in `0..n`
  and serves the first child whose delivery is enabled (non-empty mailbox and
  room for its outputs), then sets `next = (child + 1) % n`. It reports
  `Delivered`, `Settled` (nothing pending) or `Blocked` (pending but nothing
  enabled);
- `System::send_<port>` appends to the target's mailbox when it has room and
  returns an explicit rejection otherwise.
-/
namespace RoundRobin

/-- `System`: the configuration and the round-robin cursor. -/
structure Sched where
  cfg : Config N
  next : Nat

/-- `System::new`. -/
def start : Sched N := ⟨initial N, 0⟩

/-- The children `System::step_once` tries, in order. -/
def order (next : Nat) : List (Fin N.size) :=
  (List.range N.size).filterMap fun off =>
    if h : (next + off) % N.size < N.size then some ⟨(next + off) % N.size, h⟩ else none

/-- What `System::step_once` reports. -/
inductive Progress where
  | delivered (s : Sched N)
  | settled
  | blocked

/-- `System::step_once`. -/
def stepOnce (s : Sched N) : Progress N :=
  match (order N s.next).findSome? fun c => (deliver N s.cfg c).map fun k => (⟨k, (c.val + 1) % N.size⟩ : Sched N) with
  | some s' => .delivered s'
  | none => if (List.finRange N.size).all (fun c => (s.cfg.inbox c).isEmpty) then .settled else .blocked

/-- `System::send_<port>`: `none` is the explicit rejection (the target is full). -/
def send (s : Sched N) (p : Fin N.ports) (m : N.Msg p) : Option (Sched N) :=
  if (s.cfg.inbox (N.target p)).length < N.cap (N.target p) then
    some ⟨inject N s.cfg p m, s.next⟩
  else none

/-- Runs of the scheduler, with the caller's sends restricted to `E`. -/
inductive Run (E : Env N) : Sched N → Prop
  | start : Run E (start N)
  | step {s s' : Sched N} : Run E s → stepOnce N s = .delivered s' → Run E s'
  | send {s s' : Sched N} (p : Fin N.ports) (m : N.Msg p) :
      Run E s → E s.cfg p m → send N s p m = some s' → Run E s'

/-- Every scheduler step is a permitted delivery. -/
theorem stepOnce_sound (E : Env N) {s s' : Sched N} (h : stepOnce N s = .delivered s') :
    Step N E s.cfg s'.cfg := by
  unfold stepOnce at h
  split at h
  · rename_i s'' hs''
    injection h with h
    subst h
    obtain ⟨c, _, hc⟩ := List.exists_of_findSome?_eq_some hs''
    cases hs : deliver N s.cfg c with
    | none => simp [hs] at hc
    | some k =>
      simp [hs] at hc
      subst hc
      exact ⟨.deliver c, StepL.deliver c hs⟩
  · split at h <;> contradiction

theorem send_sound (E : Env N) {s s' : Sched N} (p : Fin N.ports) (m : N.Msg p)
    (he : E s.cfg p m) (h : send N s p m = some s') : Step N E s.cfg s'.cfg := by
  unfold send at h
  split at h
  · rename_i hroom
    injection h with h
    subst h
    exact ⟨.send p m, StepL.send _ p m he hroom⟩
  · contradiction

/-- **Trace inclusion.** Every configuration the scheduler reaches, under any
caller that sends only what `E` permits, is reachable by permitted behaviour. -/
theorem run_sound (E : Env N) {s : Sched N} (h : Run N E s) : Reachable N E s.cfg := by
  induction h with
  | start => exact Reachable.init
  | step _ hs ih => exact Reachable.step ih (stepOnce_sound N E hs)
  | send p m _ he hsend ih => exact Reachable.step ih (send_sound N E p m he hsend)

/-- Hence every system invariant proved over permitted behaviour holds for the scheduler. -/
theorem run_invariant (E : Env N) (P : Config N → Prop)
    (hP : ∀ k, Reachable N E k → P k) {s : Sched N} (h : Run N E s) : P s.cfg :=
  hP _ (run_sound N E h)

/-- The scheduler never overflows a mailbox. -/
theorem run_bounded (E : Env N) {s : Sched N} (h : Run N E s) : Bounded N s.cfg :=
  (run_sound N E h).bounded N E

/-- A rejected send changes nothing and is reported. -/
theorem send_rejected_iff (s : Sched N) (p : Fin N.ports) (m : N.Msg p) :
    send N s p m = none ↔ N.cap (N.target p) ≤ (s.cfg.inbox (N.target p)).length := by
  unfold send
  split <;> simp_all <;> omega

/-- `System::step_once` tries every child. -/
theorem mem_order (next : Nat) (c : Fin N.size) : c ∈ order N next := by
  have hn : 0 < N.size := Nat.lt_of_le_of_lt (Nat.zero_le _) c.isLt
  have hval : (next + (c.val + N.size - next % N.size) % N.size) % N.size = c.val := by
    rw [Nat.add_mod_mod, ← Nat.mod_add_mod]
    have h1 : next % N.size < N.size := Nat.mod_lt _ hn
    generalize next % N.size = r at *
    have : r + (c.val + N.size - r) = c.val + N.size := by omega
    rw [this, Nat.add_mod_right, Nat.mod_eq_of_lt c.isLt]
  unfold order
  rw [List.mem_filterMap]
  refine ⟨(c.val + N.size - next % N.size) % N.size, List.mem_range.mpr (Nat.mod_lt _ hn), ?_⟩
  split
  · exact congrArg some (Fin.ext hval)
  · rename_i hlt
    exact absurd (by rw [hval]; exact c.isLt) hlt

/-- **Progress, exactly.** The scheduler delivers whenever some delivery is
enabled: it reports `Settled` only when nothing is pending, and `Blocked` only
when something is pending and no delivery is enabled. -/
theorem stepOnce_delivered_iff (s : Sched N) :
    (∃ s', stepOnce N s = .delivered s') ↔ ∃ c, (deliver N s.cfg c).isSome := by
  unfold stepOnce
  constructor
  · rintro ⟨s', h⟩
    split at h
    · rename_i s'' hs''
      obtain ⟨c, _, hc⟩ := List.exists_of_findSome?_eq_some hs''
      exact ⟨c, by cases hd : deliver N s.cfg c <;> simp [hd] at hc ⊢⟩
    · split at h <;> contradiction
  · rintro ⟨c, hc⟩
    have hsome : ((order N s.next).findSome? fun c =>
        (deliver N s.cfg c).map fun k => (⟨k, (c.val + 1) % N.size⟩ : Sched N)).isSome := by
      rw [List.findSome?_isSome_iff]
      exact ⟨c, mem_order N s.next c, by simpa using hc⟩
    obtain ⟨s'', hs''⟩ := Option.isSome_iff_exists.mp hsome
    exact ⟨s'', by simp [hs'']⟩

theorem stepOnce_settled_iff (s : Sched N) :
    stepOnce N s = .settled ↔ Settled N s.cfg := by
  have key := stepOnce_delivered_iff N s
  unfold stepOnce at *
  constructor
  · intro h
    split at h
    · contradiction
    · rename_i hnone
      split at h
      · rename_i hall
        intro c
        have := (List.all_eq_true.mp hall) c (List.mem_finRange c)
        simpa using this
      · contradiction
  · intro hset
    have hno : ¬ ∃ c, (deliver N s.cfg c).isSome := by
      rintro ⟨c, hc⟩
      rw [deliver_empty N (hset c)] at hc
      simp at hc
    split
    · rename_i s'' hs''
      exact absurd (key.mp ⟨s'', by simp [hs'']⟩) hno
    · have hall : ((List.finRange N.size).all fun c => (s.cfg.inbox c).isEmpty) = true := by
        rw [List.all_eq_true]
        intro c _
        simp [hset c]
      rw [if_pos hall]

end RoundRobin

end L2R.Async1
