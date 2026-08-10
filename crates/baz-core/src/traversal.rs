//! **How a run is walked** — the order the engine visits a queue in, which is
//! not the order the queue is in.
//!
//! # The decision this module exists to hold
//!
//! The owner, 2026-08-10: *"I think shuffle as a concept is more about going to
//! an unknown next track rather than actually mutating the track list if that
//! makes sense."*
//!
//! Shuffle shipped that morning as a **permutation**: turning it on reordered
//! the queue ahead of the cursor and turning it off restored a retained copy of
//! the order it had before. That made shuffle a property of *the list*. It is
//! now a property of **the walk**: the queue keeps the order the gesture gave
//! it, always and unconditionally, and what shuffle changes is which entry the
//! engine goes to next. Turning it off is trivial because nothing was ever
//! changed — there is nothing to put back, and no retained order to invalidate,
//! to spend, or to get out of step with an edit.
//!
//! ADR-0023's shuffle amendment is this decision written down.
//!
//! # The selection rule, in one sentence
//!
//! **With shuffle on a run plays a *bag*: one deterministic shuffled pass over
//! the run's entries, in which no entry repeats until every entry has played,
//! and when the bag is spent the run ends.**
//!
//! "Unknown next track" is satisfied by several rules and they do not behave
//! alike. Uniform random each time is the simplest — pick any entry, every
//! time — and it is the wrong one: it can play the same track twice in a row,
//! it can leave a track unheard across a whole album, and the listener has no
//! way to tell an unlucky run from a broken one. The bag is what people mean by
//! the word: everything gets played, nothing gets played twice, and the order is
//! the surprise. It is also the rule that makes *unknown* stop meaning
//! *annoying*.
//!
//! Concretely, a bag **is** a permutation of the queue's positions
//! ([`Traversal::play_order`]) — computed once, from a seed, and walked. That it
//! is computed in advance is not an implementation convenience; it is the whole
//! reason gaplessness survives (below), and it is what lets baz say what is
//! coming.
//!
//! # Where the decision lives, and why it is here rather than in the front end
//!
//! **In the engine, because baz is gapless.** Gapless means the engine knows the
//! next track before the current one ends: [`crate::engine`]'s producer decodes
//! one track ahead on a prefetch thread and splices it into the same ring, so a
//! boundary costs nothing. A shuffle that chose the next track when the current
//! one *ended* would be choosing after the moment the decision was needed, and
//! every boundary in a shuffled run would carry the gap this product is most
//! careful not to have.
//!
//! The front end cannot supply the answer either, and this is the part worth
//! being precise about. The only way a front end can tell the engine what plays
//! next is by sending the queue — and [`crate::protocol::Command::UpdateQueue`]
//! is documented to cost the *next* boundary its gaplessness (the edited-over
//! track is delivered to its end and the run then continues with a **fresh
//! decode** rather than a sample-accurate splice). One edit costs one boundary,
//! which is a fine price for an edit. A shuffle implemented that way would pay
//! it at **every** boundary.
//!
//! So the engine learns a traversal mode. This is a real change to ADR-0023 §8's
//! *"the engine's queue has no shuffle flag"* and the amendment says so rather
//! than working around it. What the engine gains is exactly one standing
//! property — the order it walks its queue in — and it is careful about what it
//! does **not** gain: no repeat flag, no continuation policy, nothing that
//! refills. A bag is finite. The run still ends in silence (ADR-0023 §5).
//!
//! # What baz says about it
//!
//! **Everything it knows.** The order is decided in advance, so baz *knows* what
//! is next, and this product does not conceal what it knows: the run column
//! marks the row that plays next and greys the entries the pass is already past,
//! and the bar's continuation counts the bag's remainder rather than the list's
//! tail. "Unknown" describes how the choice was made, not something withheld
//! from the listener.
//!
//! That is what makes the permutation-of-positions shape (rather than a private
//! random generator inside the producer) load-bearing: this function is pure and
//! public, so the front end computes the identical order from the identical seed
//! and the two surfaces cannot disagree about what is coming.
//!
//! # The two questions a bag has to answer
//!
//! - **What happens when the bag empties?** The run ends —
//!   [`crate::protocol::Event::QueueEnded`], exactly as an unshuffled run ends
//!   at its last track. Nothing is refilled and nothing is re-rolled. A fresh
//!   pass comes from a fresh gesture, which is where every other list in baz
//!   comes from too.
//! - **What does a manual jump do to it?** It moves the cursor **within** the
//!   bag and does not re-roll it: jump to a track and the pass continues from
//!   that track's place in the order, so entries earlier in the bag are passed
//!   over and entries later in it still come. The alternative — re-rolling on
//!   every jump — would mean the order shown on screen changed every time the
//!   listener touched a row, which is the opposite of saying what you know.
//!
//! # Determinism
//!
//! The order is a pure function of a seed and a length, so every arrangement is
//! reproducible in a test and identical on both sides of the protocol. The
//! generator is `SplitMix64` — sixteen lines and no dependency
//! (`docs/ENGINEERING.md`: a new dependency is a reviewed decision), it passes
//! `BigCrush`, and it is the generator the Rust ecosystem itself uses to seed
//! better ones. Nothing here is cryptographic and nothing needs to be: the
//! adversary is a listener who has heard *Spirit of Eden* twice this week.

use serde::{Deserialize, Serialize};

/// **The order the engine walks its queue in.**
///
/// A standing property of the engine, set by
/// [`Command::SetTraversal`](crate::protocol::Command::SetTraversal) and
/// surviving every transport command exactly as the volume does. It never
/// touches the queue: [`Command::SetQueue`](crate::protocol::Command::SetQueue)
/// and [`Command::UpdateQueue`](crate::protocol::Command::UpdateQueue) remain
/// the only things that can change what a run holds or the order it is listed
/// in.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "traversal", rename_all = "snake_case")]
pub enum Traversal {
    /// Front to back, the order the queue is in — the default, and what an
    /// engine no front end has said anything to plays in.
    #[default]
    InOrder,
    /// A bag: one shuffled pass over the queue, nothing repeating until
    /// everything has played (module docs).
    Shuffled {
        /// Which pass. The same seed over the same length is the same order
        /// every time, on both sides of the protocol.
        ///
        /// A front end rolls a fresh one per **run** — the same seed over a
        /// re-played album would be the same shuffle twice, which is the one
        /// thing a listener would notice as wrong.
        seed: u64,
    },
}

impl Traversal {
    /// Whether this traversal is a shuffled one — the question the bar's
    /// crossed-arrows control asks and the only thing about a traversal that
    /// is persisted (`config.toml` stores the mode, never the seed: the seed
    /// belongs to a run, and a run does not survive a quit as a *shuffle*).
    #[must_use]
    pub const fn is_shuffled(self) -> bool {
        matches!(self, Self::Shuffled { .. })
    }

    /// **The queue positions this traversal visits, in the order it visits
    /// them** — a permutation of `0..len`, and the whole of what a traversal
    /// means.
    ///
    /// Total: `len` 0 is the empty order, `len` 1 is `[0]` under either mode,
    /// and every entry appears exactly once for every seed. That last property
    /// is what makes the bag a bag, and it is swept over seeds in the tests
    /// below rather than asserted about one of them.
    ///
    /// Called by the engine when the queue or the traversal changes, and by a
    /// front end that wants to draw what is coming. One function, so the two
    /// cannot disagree.
    #[must_use]
    pub fn play_order(self, len: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..len).collect();
        if let Self::Shuffled { seed } = self {
            shuffle_indices(&mut order, seed);
        }
        order
    }
}

/// **Fisher–Yates over a list of positions.**
///
/// In place, unbiased, and deterministic: the same seed over the same list is
/// the same order every time, which is what lets a test pin an arrangement
/// instead of asserting that a shuffle is "random enough".
fn shuffle_indices(order: &mut [usize], seed: u64) {
    let mut state = seed;
    for index in (1..order.len()).rev() {
        order.swap(index, pick(&mut state, index));
    }
}

/// A uniform position in `0..=upper`, from `state`.
///
/// The modulus is `upper + 1`, which is at most the list's length, so the
/// remainder is a valid index by construction and the conversion back cannot
/// lose anything. The modulo bias over a range this small against 2^64 is not
/// measurable; the alternative is rejection sampling for a record player.
fn pick(state: &mut u64, upper: usize) -> usize {
    let span = u64::try_from(upper).unwrap_or(u64::MAX).saturating_add(1);
    usize::try_from(next(state) % span).unwrap_or(0)
}

/// `SplitMix64` — the whole of baz's randomness (module docs).
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    /// **A bag holds every entry exactly once** — the property that makes it a
    /// bag rather than a die, for every seed and every length.
    ///
    /// Swept rather than pinned: a traversal that could lose a position or
    /// visit one twice would be a different bug on every seed, and the
    /// listener-visible symptom (a track never played, a track played twice)
    /// is the same one in both directions.
    #[test]
    fn a_shuffled_pass_visits_every_position_exactly_once() {
        for len in 0..24_usize {
            for seed in 0..64_u64 {
                let order = Traversal::Shuffled { seed }.play_order(len);
                assert_eq!(order.len(), len, "len {len} seed {seed}");
                let seen: HashSet<usize> = order.iter().copied().collect();
                assert_eq!(seen.len(), len, "len {len} seed {seed} repeated a position");
                assert!(
                    order.iter().all(|&at| at < len),
                    "len {len} seed {seed} left the queue"
                );
            }
        }
    }

    /// **In order is the identity**, which is what makes every reading in the
    /// product one code path: the shuffled case and the plain case are the same
    /// walk over a different permutation, not two walks.
    #[test]
    fn walking_in_order_is_the_queue_itself() {
        assert_eq!(Traversal::InOrder.play_order(5), [0, 1, 2, 3, 4]);
        assert!(Traversal::InOrder.play_order(0).is_empty());
        assert_eq!(Traversal::InOrder.play_order(1), [0]);
        assert!(!Traversal::InOrder.is_shuffled());
        assert!(Traversal::Shuffled { seed: 1 }.is_shuffled());
        // The default is the one an engine nobody has spoken to plays in.
        assert_eq!(Traversal::default(), Traversal::InOrder);
    }

    /// Deterministic, and actually shuffling.
    #[test]
    fn the_same_seed_is_the_same_pass_and_different_seeds_differ() {
        let of = |seed| Traversal::Shuffled { seed }.play_order(6);
        assert_eq!(of(0x5EED), of(0x5EED));
        // Over 6! = 720 orders, a fixed pair that collided would be a defect in
        // the generator rather than luck.
        assert_ne!(of(9), of(10));
        // And it does not simply hand back the queue.
        assert!(
            (0..64_u64).any(|seed| of(seed) != Traversal::InOrder.play_order(6)),
            "no seed in 64 produced a different order"
        );
        // Degenerate lengths are the identity rather than a panic.
        assert!(Traversal::Shuffled { seed: 3 }.play_order(0).is_empty());
        assert_eq!(Traversal::Shuffled { seed: 3 }.play_order(1), [0]);
    }

    /// **A queue this long is a queue baz ships** — `Play all` over a
    /// five-figure library — so the order is computed for one here rather than
    /// assumed to scale.
    #[test]
    fn a_five_figure_run_is_an_ordinary_bag() {
        let order = Traversal::Shuffled { seed: 7 }.play_order(40_000);
        let seen: HashSet<usize> = order.iter().copied().collect();
        assert_eq!(seen.len(), 40_000);
    }
}
