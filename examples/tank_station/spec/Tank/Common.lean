/-!
# Tank station: shared vocabulary

Data types used by more than one machine. They own no behaviour.
-/
namespace Tank

/-- Classification of the tank's fill level. -/
inductive Level where
  | low
  | normal
  | high
  deriving DecidableEq, Repr

end Tank
