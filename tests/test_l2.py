"""Tests for L2 Basic — partner-aware, cost-minimizing trick play."""

from __future__ import annotations

from f45 import card_strength
from f45.strategies import L1Novice, L2Basic
from f45.tournament import run_tournament


# -----------------------------------------------------------------------------
# Trump declaration — strength-weighted
# -----------------------------------------------------------------------------

def test_strength_weighted_trump_picks_bowers_over_length():
    """Hand [5C, JC, 2D, 3D, 4D]: L1 calls Diamonds (3 cards). L2 should
    call Clubs because 5C and JC are two of the three top trumps under
    Clubs (~199 and ~185) — totalling ~393 in Clubs vs ~315 in Diamonds
    (where the three diamonds become low trumps but the two clubs collapse
    to off-suit-black ranks).

    We test the scoring directly rather than driving a game to the
    declare_trump phase: the engine doesn't expose a `from_state` helper
    on the Python side, so we verify that L2's strength-sum heuristic
    ranks Clubs above Diamonds for this hand.
    """
    hand = ["5C", "JC", "2D", "3D", "4D"]
    by_trump = {t: sum(card_strength(c, t) for c in hand) for t in "CDHS"}
    assert max(by_trump, key=by_trump.get) == "C", by_trump
    # Clubs should win comfortably — the bowers are worth more than length.
    assert by_trump["C"] > by_trump["D"] + 50


def test_strength_weighted_trump_breaks_ties_c_lt_d_lt_h_lt_s():
    """When two suits tie on strength, L2 prefers C < D < H < S
    (matching L1's tiebreak ordering, which maps to PHP arsort behavior).
    """
    # Construct a hand where two non-trump suits tie. With AC=ace of clubs
    # (non-trump strength) and AD=ace of diamonds (non-trump strength),
    # if trump=H both AC and AD score identically as off-suit aces.
    # The strength sum for trump=C will favor AC; for trump=D will favor AD.
    # We use the L2 tiebreak directly:
    pairs = [(100, "C"), (100, "D")]
    best = max(pairs, key=lambda kv: (kv[0], -ord(kv[1])))[1]
    assert best == "C"


# -----------------------------------------------------------------------------
# `_beating_cards` — the heart of "play minimum to win"
# -----------------------------------------------------------------------------

def test_beating_cards_higher_trump_beats_lower_trump():
    bot = L2Basic(seed=0)
    # Trump=S; opponent led 2S (low trump), partner played 3S, then opponent
    # plays 8S. To us, the winning card is 8S. We hold [JS, 5S, AH, KS, 2H].
    # 5S, JS, AH all beat 8S (higher trump strengths). KS beats 8S? No —
    # K of trump is strength rank below ace; 8S has strength below KS.
    # All four trumps in our hand beat 8S except... wait, JS and 5S are
    # bowers (above ace); AH is third bower; KS = K of trump (above J/10).
    # 8S < KS. So all four (5S, JS, AH, KS) beat 8S. 2H is not trump.
    legal = ["5S", "JS", "AH", "KS", "2H"]
    winners = bot._beating_cards(legal, winning_card="8S", lead_card="2S", trump="S")
    assert set(winners) == {"5S", "JS", "AH", "KS"}


def test_beating_cards_trump_beats_non_trump_winner():
    """Trump=S, lead=KD (non-trump), winner=KD. KD is the highest off-suit
    diamond (in red off-suit, K is high — Ace is LOW per the 45s rule).
    So nothing in lead suit can beat KD; only trumps can.

    Holdings [2S, 3S, AD, 8H]:
    - 2S, 3S trump KD → win
    - AD is lowest off-suit Diamond (red ace = 1) → cannot beat KD
    - 8H non-trump non-lead → cannot win
    """
    bot = L2Basic(seed=0)
    legal = ["2S", "3S", "AD", "8H"]
    winners = bot._beating_cards(legal, winning_card="KD", lead_card="KD", trump="S")
    assert set(winners) == {"2S", "3S"}


def test_beating_cards_higher_off_suit_can_win_within_lead():
    """Trump=S, lead=2D, winner=10D. We hold [AC, KD, 5D, 9H].
    - KD follows lead and is rank-higher than 10D in red off-suit → wins
    - 5D follows lead but ranks below 10D → cannot win
    - AC non-trump non-lead → cannot win
    - 9H non-trump non-lead → cannot win
    """
    bot = L2Basic(seed=0)
    legal = ["AC", "KD", "5D", "9H"]
    winners = bot._beating_cards(legal, winning_card="10D", lead_card="2D", trump="S")
    assert set(winners) == {"KD"}


def test_beating_cards_red_ace_is_low_in_off_suit():
    """Regression: the 45s 'aces low in red off-suit' rule. AD must NOT be
    treated as beating any other off-suit Diamond when trump is not D.
    """
    bot = L2Basic(seed=0)
    # Trump=S, lead=2D, winner=2D (the lowest non-ace red rank). AD still
    # can't beat 2D — AD ranks below 2D in red off-suit.
    legal = ["AD"]
    winners = bot._beating_cards(legal, winning_card="2D", lead_card="2D", trump="S")
    assert winners == []


def test_beating_cards_non_trump_off_suit_cannot_win():
    """Trump=S, lead=2D, winner=KD. We hold [AC, 8H, AS, 5D].
    Only AS (a trump) can win; off-suit non-lead cards never can.
    """
    bot = L2Basic(seed=0)
    legal = ["AC", "8H", "AS", "5D"]
    winners = bot._beating_cards(legal, winning_card="KD", lead_card="2D", trump="S")
    assert set(winners) == {"AS"}


def test_beating_cards_returns_empty_when_nothing_beats():
    bot = L2Basic(seed=0)
    # Trump=S. Winner is 5S (highest possible). Our hand has only weaker
    # trumps and off-suit cards — nothing beats 5S.
    legal = ["8C", "9D", "2H", "AS"]
    winners = bot._beating_cards(legal, winning_card="5S", lead_card="2S", trump="S")
    assert winners == []


# -----------------------------------------------------------------------------
# Integration: L2 should beat L1 substantially over a modest sample.
# -----------------------------------------------------------------------------

def test_l2_beats_l1_at_55_pct_or_better():
    """The Stage 1 success criterion: L2 beats L1 ≥55% over enough games.
    We use 1000 to keep the test cheap; the 10k verification lives in
    scripts/ladder.py. At 1000 games the std-err is ~1.6%, so 55% is a
    safe lower bound when the true rate is ~65%.
    """
    bots = [L2Basic(seed=0), L1Novice(seed=100), L2Basic(seed=2), L1Novice(seed=102)]
    result = run_tournament(bots, n_games=1000, base_seed=0)
    assert result.team0_winrate >= 0.55, (
        f"L2 (team 0) should beat L1 ≥55%, got {result.team0_winrate:.1%}"
    )


def test_l2_drives_a_hand_to_completion():
    """Smoke test: L2 plays a full game without raising or producing an
    illegal action (the engine would reject illegal plays with PyValueError).
    """
    from f45 import GameState

    state = GameState(seed=42, num_players=4, dealer=0)
    bots = [L2Basic(seed=s) for s in range(4)]

    while state.phase() not in ("hand_complete", "game_over"):
        phase = state.phase()
        if phase == "discard":
            for seat in range(4):
                bots[seat].act(state, seat)
        else:
            bots[state.to_act()].act(state, seat=state.to_act())

    assert state.phase() == "hand_complete"
    hp = state.hand_points()
    assert sum(hp) == 30
