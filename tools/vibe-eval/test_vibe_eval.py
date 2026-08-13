import tempfile
import unittest
from pathlib import Path

import vibe_eval


class HarnessTests(unittest.TestCase):
    def test_corpus_fingerprint_matches_rust_baseline(self) -> None:
        tracks = [{
            "id": "one",
            "path": "/music/one.flac",
            "title": "One",
            "artist": "Artist",
            "album": "Album",
            "genre": "Jazz",
            "tags": ["warm", "live"],
        }]
        self.assertEqual(
            vibe_eval.corpus_fingerprint(tracks),
            "fnv1a64:80b1207e4f81faab",
        )

    def test_candidate_selects_its_own_manifest_and_identity(self) -> None:
        self.assertEqual(vibe_eval.artifact_manifest("dclap", None).name, "artifacts.json")
        self.assertEqual(vibe_eval.artifact_manifest("laion", None).name, "artifacts-laion.json")
        self.assertNotEqual(
            vibe_eval.candidate_system("dclap"),
            vibe_eval.candidate_system("laion"),
        )
        with self.assertRaisesRegex(ValueError, "requires an.*LAION".casefold()):
            vibe_eval.require_candidate_index(
                {"system": vibe_eval.candidate_system("dclap")},
                "laion",
                Path("index.json"),
            )

    def test_window_sampling_keeps_both_ends_and_the_middle(self) -> None:
        samples = vibe_eval.SEGMENT_SAMPLES * 10
        full = vibe_eval.window_starts(samples, None)
        sampled = vibe_eval.window_starts(samples, 3)
        self.assertEqual(sampled[0], full[0])
        self.assertEqual(sampled[-1], full[-1])
        self.assertIn(sampled[1], full)

    def test_corrupt_artifact_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "model").write_bytes(b"wrong")
            manifest = root / "manifest.json"
            vibe_eval.write_json(
                manifest,
                {
                    "schema": 1,
                    "files": [{"name": "model", "bytes": 5, "sha256": "0" * 64}],
                },
            )
            with self.assertRaisesRegex(ValueError, "sha256"):
                vibe_eval.verify_artifacts(root, manifest)

    def test_blind_mapping_is_separate_and_reproducible(self) -> None:
        request = {"id": "one", "request": {"query": "warm"}, "ranking": ["track"]}
        runs = [
            {"system": "baseline", "corpus_ids": ["track"], "corpus_fingerprint": "same", "results": [request]},
            {"system": "semantic", "corpus_ids": ["track"], "corpus_fingerprint": "same", "results": [request]},
        ]
        first = vibe_eval.make_blind(runs, 7, 20)
        second = vibe_eval.make_blind(runs, 7, 20)
        self.assertEqual(first, second)
        self.assertNotIn("system", str(first[0]))
        self.assertIn("semantic", str(first[1]))

    def test_blind_refuses_different_corpora(self) -> None:
        request = {"id": "one", "request": {"query": "warm"}, "ranking": []}
        runs = [
            {"system": "one", "corpus_ids": ["a"], "corpus_fingerprint": "one", "results": [request]},
            {"system": "two", "corpus_ids": ["b"], "corpus_fingerprint": "two", "results": [request]},
        ]
        with self.assertRaisesRegex(ValueError, "same non-empty corpus"):
            vibe_eval.make_blind(runs, 7, 20)

    def test_blind_refuses_reused_ids_for_different_tracks(self) -> None:
        request = {"id": "one", "request": {"query": "warm"}, "ranking": []}
        runs = [
            {"system": "one", "corpus_ids": ["a"], "corpus_fingerprint": "first", "results": [request]},
            {"system": "two", "corpus_ids": ["a"], "corpus_fingerprint": "second", "results": [request]},
        ]
        with self.assertRaisesRegex(ValueError, "same exact corpus"):
            vibe_eval.make_blind(runs, 7, 20)

    def test_metadata_control_prefers_matching_terms_then_diversifies(self) -> None:
        tracks = [
            {"id": "one", "title": "Blue", "artist": "A", "album": "First", "genre": "jazz"},
            {"id": "two", "title": "Red", "artist": "A", "album": "Second", "genre": "metal"},
            {"id": "three", "title": "Green", "artist": "B", "album": "Third", "genre": "jazz"},
        ]
        ranking = vibe_eval.metadata_ranking(tracks, {"query": "gentle jazz"}, 2)
        self.assertEqual(ranking, ["one", "three"])

    def test_random_control_is_reproducible_and_diversity_matched(self) -> None:
        tracks = [
            {"id": "one", "artist": "A", "album": "First"},
            {"id": "two", "artist": "A", "album": "Second"},
            {"id": "three", "artist": "A", "album": "Third"},
            {"id": "four", "artist": "B", "album": "First"},
            {"id": "five", "artist": "B", "album": "Fourth"},
            {"id": "six", "artist": "C", "album": "Fifth"},
        ]
        first = vibe_eval.random_diverse_ranking(tracks, 6, 42)
        self.assertEqual(first, vibe_eval.random_diverse_ranking(tracks, 6, 42))
        selected = [next(track for track in tracks if track["id"] == track_id) for track_id in first]
        self.assertTrue(all(left["artist"] != right["artist"] for left, right in zip(selected, selected[1:])))
        self.assertLessEqual(sum(track["artist"] == "A" for track in selected), 2)
        # Fresh albums win whenever they remain eligible under the artist rules.
        self.assertEqual(len({track["album"] for track in selected[:4]}), 4)

    def test_random_control_seed_changes_by_request_without_reading_the_request(self) -> None:
        self.assertEqual(vibe_eval.request_seed(7, "one"), vibe_eval.request_seed(7, "one"))
        self.assertNotEqual(vibe_eval.request_seed(7, "one"), vibe_eval.request_seed(7, "two"))

    def test_scoring_restores_system_identity(self) -> None:
        ratings = {
            "relevance": 2,
            "coherence": 2,
            "transitions": 2,
            "diversity": 2,
            "rediscovery": 2,
            "replay": 2,
        }
        ballot = {
            "schema": 1,
            "items": [{
                "id": "one",
                "preferred": "B",
                "candidates": [
                    {"code": "A", "ratings": ratings},
                    {"code": "B", "ratings": {**ratings, "relevance": 5}},
                ],
            }],
        }
        key = {
            "schema": 1,
            "items": [{"id": "one", "mapping": [
                {"code": "A", "system": "baseline"},
                {"code": "B", "system": "semantic"},
            ]}],
        }
        score = vibe_eval.score_ballot(ballot, key)
        self.assertEqual(score["systems"]["semantic"]["preferred"], 1)
        self.assertEqual(score["systems"]["semantic"]["means"]["relevance"], 5)

    def test_scoring_refuses_an_incomplete_ballot(self) -> None:
        ballot = {
            "schema": 1,
            "items": [{
                "id": "one",
                "preferred": None,
                "candidates": [{"code": "A", "ratings": {name: None for name in vibe_eval.RATING_DIMENSIONS}}],
            }],
        }
        key = {"schema": 1, "items": [{"id": "one", "mapping": [{"code": "A", "system": "baseline"}]}]}
        with self.assertRaisesRegex(ValueError, "preferred candidate"):
            vibe_eval.score_ballot(ballot, key)

    def test_scoring_refuses_a_missing_rating(self) -> None:
        ratings = {name: 3 for name in vibe_eval.RATING_DIMENSIONS}
        ratings["replay"] = None
        ballot = {
            "schema": 1,
            "items": [{
                "id": "one",
                "preferred": "A",
                "candidates": [{"code": "A", "ratings": ratings}],
            }],
        }
        key = {"schema": 1, "items": [{"id": "one", "mapping": [{"code": "A", "system": "baseline"}]}]}
        with self.assertRaisesRegex(ValueError, "replay: rating is required"):
            vibe_eval.score_ballot(ballot, key)

    def test_materialized_playlists_keep_system_identity_out(self) -> None:
        ballot = {
            "schema": 1,
            "items": [{
                "id": "warm",
                "request": {"query": "warm"},
                "candidates": [{"code": "A", "ranking": ["one"]}],
            }],
        }
        corpus = {
            "schema": 1,
            "tracks": [{"id": "one", "path": "/music/one.flac"}],
        }
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "listening"
            self.assertEqual(vibe_eval.materialize_ballot(ballot, corpus, output), 1)
            playlist = (output / "01-warm" / "A.m3u8").read_text(encoding="utf-8")
            self.assertIn("/music/one.flac", playlist)
            self.assertNotIn("semantic", playlist)
            with self.assertRaises(FileExistsError):
                vibe_eval.materialize_ballot(ballot, corpus, output)


if __name__ == "__main__":
    unittest.main()
