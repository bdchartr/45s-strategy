//! Baseline throughput: hands per second for a random-legal-move driver.
//!
//! Run with:
//!
//! ```text
//! cargo run --release --example bench_hands_per_sec
//! ```
//!
//! Why this matters: Stage-4 self-play RL needs games per core-second as a
//! budgeting number. Everything fancier (MCTS, networks) multiplies this.
//! We want to know the ceiling before we start adding cost.
//!
//! What this measures: end-to-end 4-player hands (bid, declare, discard,
//! play), picking a random legal action at every step. Card discards use the
//! same "3 weakest, non-trump-preferred" heuristic as the proptest driver
//! (see tests/proptest_invariants.rs) so we're not enumerating subsets.
//!
//! Caveats:
//! - Debug-mode numbers are ~10-20× slower; always `--release`.
//! - This is the pure rules engine; neural-net evaluation is not modelled.
//! - Single-threaded; actor-learner parallelism will multiply by core count.

use std::time::Instant;

use _engine::cards::Card;
use _engine::ranker;
use _engine::state::{Action, GameConfig, GameState, Move, Phase};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Drive one 4-player hand to HandComplete, returning the number of actions
/// applied. Uses `rng` to pick among legal actions at every step.
fn play_one_hand(mut g: GameState, rng: &mut ChaCha8Rng) -> (GameState, u32) {
    let mut steps = 0u32;
    loop {
        match g.phase() {
            Phase::Bidding | Phase::DeclareTrump | Phase::Play => {
                let seat = g.to_act().unwrap();
                let legal = g.legal_actions(seat);
                let idx = rng.gen_range(0..legal.len());
                g.apply(Move { seat, action: legal[idx].clone() }).unwrap();
                steps += 1;
            }
            Phase::Discard => {
                let bidder = g.bidder().unwrap();
                let trump = g.trump().unwrap();
                for seat in 0..g.num_players() {
                    if seat == bidder {
                        let mut hand = g.hand(seat).to_vec();
                        hand.sort_by_key(|c| {
                            (
                                if ranker::is_trump(*c, trump) { 1 } else { 0 },
                                ranker::strength(*c, trump),
                            )
                        });
                        let to_drop: Vec<Card> = hand.into_iter().take(3).collect();
                        g.apply(Move { seat, action: Action::Discard(to_drop) }).unwrap();
                    } else {
                        g.apply(Move { seat, action: Action::Discard(vec![]) }).unwrap();
                    }
                    steps += 1;
                }
            }
            Phase::DealerExtraDraw => unreachable!("4p doesn't reach DealerExtraDraw"),
            Phase::HandComplete | Phase::GameOver => break,
        }
    }
    (g, steps)
}

fn main() {
    // Warm up to force JIT-like effects (branch predictor, icache, allocator).
    let mut rng = ChaCha8Rng::seed_from_u64(0xF00D);
    for seed in 0..200u64 {
        let g = GameState::new_hand(
            seed,
            GameConfig { num_players: 4, target_score: 120, enable_30_for_60: true },
            0,
        );
        let _ = play_one_hand(g, &mut rng);
    }

    // Measured run.
    let target_hands: u32 = 10_000;
    let start = Instant::now();
    let mut total_steps: u64 = 0;
    let mut total_tricks: u64 = 0;
    for seed in 0..target_hands as u64 {
        let g = GameState::new_hand(
            seed,
            GameConfig { num_players: 4, target_score: 120, enable_30_for_60: true },
            0,
        );
        let (g, steps) = play_one_hand(g, &mut rng);
        total_steps += steps as u64;
        total_tricks += g.completed_tricks().len() as u64;
    }
    let elapsed = start.elapsed();

    let hps = target_hands as f64 / elapsed.as_secs_f64();
    let sps = total_steps as f64 / elapsed.as_secs_f64();
    println!("benchmark: 4-player random-legal driver (single-threaded)");
    println!("  hands:   {}", target_hands);
    println!("  elapsed: {:.3}s", elapsed.as_secs_f64());
    println!("  steps:   {} ({:.1} avg/hand)", total_steps, total_steps as f64 / target_hands as f64);
    println!("  tricks:  {}", total_tricks);
    println!();
    println!("  throughput: {:.0} hands/sec", hps);
    println!("              {:.0} actions/sec", sps);
    println!();
    println!("  (for self-play budgeting: multiply by core count for naive parallelism;");
    println!("   neural-net evaluation will dominate once wired in.)");
}
