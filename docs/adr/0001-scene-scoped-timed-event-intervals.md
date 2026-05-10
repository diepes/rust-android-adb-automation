# Scene-scoped TimedEvent interval resets

TimedEvents linked to one or more Scenes reset their interval clock when the device transitions out of all their matching Scenes, and resume uninterrupted when moving between two Scenes that are both in their set. The first fire after Scene activation is delayed by one full interval — not immediate — so that automations are timed consistently from the start of a phase (e.g. a game round) rather than from an arbitrary previous execution.

The alternative — running all timers continuously regardless of Scene — was rejected because it produces unpredictable firing times relative to game-round boundaries. A TimedEvent with no Scenes set retains the unconditional continuous behaviour for cases where Scene-awareness is not needed.
