"""測試 Translation dataset、dataloader 與批次翻譯。"""

from pathlib import Path
import sys

import pytest

sys.path.append(str(Path(__file__).resolve().parents[2]))

from core.utils.wikidata_translator import (  # noqa: E402  # pylint: disable=C0413
    AdminLevel,
    TranslationDataLoader,
    TranslationDatasetBuilder,
    TranslationItem,
    WikidataTranslator,
)


def test_translation_item_id_is_unique_with_parent() -> None:
    """相同名稱但不同父層應生成不同 ID。"""

    item_a = TranslationItem.from_values(
        level=AdminLevel.ADMIN_2,
        original_name="城東區",
        source_lang="ko",
        target_lang="zh-hant",
        parent_chain=("KR", "首爾"),
    )
    item_b = TranslationItem.from_values(
        level=AdminLevel.ADMIN_2,
        original_name="城東區",
        source_lang="ko",
        target_lang="zh-hant",
        parent_chain=("KR", "京畿道"),
    )

    assert item_a.id != item_b.id


def test_dataset_builder_admin1_deduplicates() -> None:
    """Admin_1 builder 應自動去除重複名稱。"""

    records = [
        {"sidonm": "首爾"},
        {"sidonm": "首爾"},
        {"sidonm": "京畿道"},
    ]
    builder = TranslationDatasetBuilder(
        country_code="KR", source_lang="ko", target_lang="zh-hant"
    )

    dataset = builder.build_admin1(records, name_field="sidonm")

    assert len(dataset) == 2
    assert dataset.stats().unique_parent == 1


def test_dataset_builder_admin2_keeps_parent_chain() -> None:
    """Admin_2 builder 應保留父層資訊並允許同名不同父層。"""

    records = [
        {"sidonm": "首爾", "sggnm": "城東區", "row": 1},
        {"sidonm": "京畿道", "sggnm": "城東區", "row": 2},
    ]
    builder = TranslationDatasetBuilder(
        country_code="KR", source_lang="ko", target_lang="zh-hant"
    )

    dataset = builder.build_admin2(
        records,
        parent_field="sidonm",
        name_field="sggnm",
        metadata_fields=["row"],
    )

    assert len(dataset) == 2
    parent_sets = {item.parent_chain for item in dataset}
    assert parent_sets == {("KR", "首爾"), ("KR", "京畿道")}
    for item in dataset:
        assert "row" in item.metadata


def test_dataloader_reports_progress() -> None:
    """dataloader 應依 batch 更新進度回呼。"""

    builder = TranslationDatasetBuilder(
        country_code="KR", source_lang="ko", target_lang="zh-hant"
    )
    records = [{"sidonm": "首爾", "sggnm": f"第{i}區"} for i in range(4)]
    dataset = builder.build_admin2(
        records,
        parent_field="sidonm",
        name_field="sggnm",
    )

    progress_history: list[int] = []

    def progress(processed: int, total: int) -> None:
        progress_history.append(processed)
        assert total == len(dataset)

    loader = TranslationDataLoader(
        dataset,
        batch_size=2,
        progress_callback=progress,
    )

    batches = list(loader)

    assert len(batches) == 2
    assert progress_history == [2, 4]


@pytest.fixture()
def translator(monkeypatch: pytest.MonkeyPatch) -> WikidataTranslator:
    """建立乾淨的 WikidataTranslator 實例並封裝快取 I/O。"""

    tr = WikidataTranslator(
        source_lang="ko",
        target_lang="zh-tw",
        fallback_langs=["zh-tw"],
        cache_path=None,
        use_opencc=False,
    )
    tr.cache = tr._create_empty_cache()
    monkeypatch.setattr(tr, "_save_cache", lambda: None)
    return tr


def test_batch_translate_basic(
    monkeypatch: pytest.MonkeyPatch, translator: WikidataTranslator
) -> None:
    """batch_translate 應回傳以 item.id 為 key 的結果並考慮 parent_qids。"""

    builder = TranslationDatasetBuilder(
        country_code="KR", source_lang="ko", target_lang="zh-tw"
    )
    records = [
        {"sidonm": "首爾", "sggnm": "城東區"},
        {"sidonm": "釜山", "sggnm": "海雲臺區"},
    ]
    dataset = builder.build_admin2(records, parent_field="sidonm", name_field="sggnm")

    search_map = {
        "城東區": ["Q111"],
        "海雲臺區": ["Q222"],
    }
    labels_map = {
        "Q111": {"zh-tw": "城東區"},
        "Q222": {"zh-tw": "海雲臺區"},
    }

    monkeypatch.setattr(
        translator,
        "_search_wikidata",
        lambda name: list(search_map[name]),
    )
    monkeypatch.setattr(
        translator,
        "_batch_get_labels",
        lambda qids: {qid: labels_map[qid] for qid in qids},
    )
    monkeypatch.setattr(translator, "_batch_get_instance_of", lambda qids: {})

    def verify_p131(qid: str, parent_qid: str) -> bool:
        return qid == "Q111" and parent_qid == "PARENT1"

    monkeypatch.setattr(translator, "_verify_p131", verify_p131)

    parent_map = {
        dataset[0].id: "PARENT1",
        dataset[1].id: "PARENTX",
    }

    results = translator.batch_translate(
        dataset,
        batch_size=1,
        parent_qids=parent_map,
        show_progress=False,
    )

    assert results[dataset[0].id]["parent_verified"] is True
    assert results[dataset[1].id]["parent_verified"] is False
    assert results[dataset[0].id]["translated"] == "城東區"
    assert results[dataset[1].id]["translated"] == "海雲臺區"


def test_batch_translate_candidate_filter(
    monkeypatch: pytest.MonkeyPatch, translator: WikidataTranslator
) -> None:
    """候選過濾器應能排除不需要的 QID。"""

    builder = TranslationDatasetBuilder(
        country_code="KR", source_lang="ko", target_lang="zh-tw"
    )
    records = [{"sidonm": "首爾", "sggnm": "問題區"}]
    dataset = builder.build_admin2(records, parent_field="sidonm", name_field="sggnm")

    monkeypatch.setattr(
        translator,
        "_search_wikidata",
        lambda name: ["Q_BAD", "Q_GOOD"],
    )
    monkeypatch.setattr(
        translator,
        "_batch_get_labels",
        lambda qids: {qid: {"zh-tw": qid} for qid in qids},
    )
    monkeypatch.setattr(
        translator,
        "_batch_get_instance_of",
        lambda qids: {qid: [] for qid in qids},
    )
    monkeypatch.setattr(translator, "_verify_p131", lambda qid, parent: False)

    def candidate_filter(_: str, metadata: dict) -> bool:
        return metadata["qid"].endswith("GOOD")

    results = translator.batch_translate(
        dataset,
        batch_size=2,
        parent_qids=None,
        show_progress=False,
        candidate_filter=candidate_filter,
    )

    assert results[dataset[0].id]["qid"] == "Q_GOOD"


def test_translate_respects_parent_qid(
    monkeypatch: pytest.MonkeyPatch, translator: WikidataTranslator
) -> None:
    """translate() 應利用 parent_qid 執行 P131 驗證。"""

    monkeypatch.setattr(
        translator,
        "_search_wikidata",
        lambda name: [f"Q_{name}"],
    )
    monkeypatch.setattr(
        translator,
        "_batch_get_labels",
        lambda qids: {qid: {"zh-tw": qid} for qid in qids},
    )
    monkeypatch.setattr(translator, "_batch_get_instance_of", lambda qids: {})

    verified: list[tuple[str, str]] = []

    def fake_verify(qid: str, parent_qid: str) -> bool:
        verified.append((qid, parent_qid))
        return parent_qid == "PARENT_OK"

    monkeypatch.setattr(translator, "_verify_p131", fake_verify)

    result = translator.translate("測試地名", parent_qid="PARENT_OK")

    assert result["parent_verified"] is True
    assert verified == [("Q_測試地名", "PARENT_OK")]


def test_batch_translate_parent_map(
    monkeypatch: pytest.MonkeyPatch, translator: WikidataTranslator
) -> None:
    """batch_translate 接收 dataset 時能套用 parent_qids。"""

    names = ["首爾", "釜山"]
    search_map = {name: [f"Q_{name}"] for name in names}

    monkeypatch.setattr(translator, "_search_wikidata", lambda name: search_map[name])
    monkeypatch.setattr(
        translator,
        "_batch_get_labels",
        lambda qids: {qid: {"zh-tw": qid} for qid in qids},
    )
    monkeypatch.setattr(translator, "_batch_get_instance_of", lambda qids: {})

    def fake_verify(qid: str, parent_qid: str) -> bool:
        return (qid, parent_qid) == ("Q_首爾", "PARENT1")

    monkeypatch.setattr(translator, "_verify_p131", fake_verify)

    records = [{"sidonm": name} for name in names]
    dataset = TranslationDatasetBuilder(
        country_code="KR", source_lang="ko", target_lang="zh-tw"
    ).build_admin1(records, name_field="sidonm")
    parent_map = {item.id: ("PARENT1" if "首爾" in item.original_name else "PARENTX") for item in dataset}

    results = translator.batch_translate(
        dataset,
        parent_qids=parent_map,
        show_progress=False,
    )

    for item in dataset:
        data = results[item.id]
        if item.original_name == "首爾":
            assert data["parent_verified"] is True
        else:
            assert data["parent_verified"] is False
