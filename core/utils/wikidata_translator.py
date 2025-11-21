"""通用的 Wikidata 地名翻譯工具。

此模組提供透過 Wikidata 查詢將地名從任意語言翻譯成繁體中文的功能。

主要特性：
- 支援任意來源語言到目標語言的翻譯
- 使用 Wikidata SPARQL 查詢和 API
- P131 (located in) 層級關係驗證
- 多層回退策略（zh-tw → zh-hant → zh → 簡轉繁 → en → 原文）
- 快取機制減少重複查詢
- 速率限制和重試機制
- 批次翻譯與進度顯示

使用範例：
    # 韓文翻譯成繁體中文
    translator = WikidataTranslator(
        source_lang='ko',
        target_lang='zh-tw',
        fallback_langs=['zh-hant', 'zh', 'en', 'ko'],
        cache_path='geoname_data/KR_wikidata_cache.json'
    )

    result = translator.translate('서울특별시')
    # {'translated': '首爾特別市', 'qid': 'Q8684', 'source': 'wikidata'}

    # 批次翻譯
    results = translator.batch_translate(['서울특별시', '부산광역시'])
"""

import json
import random
import time
from collections.abc import Callable, Iterable, Iterator, Sequence
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from types import MappingProxyType
from typing import Any, Mapping

import requests
from tqdm import tqdm

from core.utils import logger


class AdminLevel(str, Enum):
    """支援的行政層級。"""

    ADMIN_1 = "admin_1"
    ADMIN_2 = "admin_2"


def _normalize_text(value: Any) -> str:
    """將輸入值轉為乾淨字串。"""

    if value is None:
        return ""
    return str(value).strip()


def build_translation_item_id(
    level: AdminLevel,
    parent_chain: Sequence[str],
    original_name: str,
) -> str:
    """產生唯一 ID，方便快取與日誌。"""

    chain = [level.value]
    chain.extend(parent_chain)
    chain.append(original_name)
    return "/".join(part for part in chain if part)


@dataclass(frozen=True, slots=True)
class TranslationItem:
    """單筆翻譯需求。"""

    id: str
    level: AdminLevel
    original_name: str
    source_lang: str
    target_lang: str
    parent_chain: tuple[str, ...]
    metadata: Mapping[str, Any] = field(default_factory=dict)

    @classmethod
    def from_values(
        cls,
        *,
        level: AdminLevel,
        original_name: str,
        source_lang: str,
        target_lang: str,
        parent_chain: Sequence[str],
        metadata: Mapping[str, Any] | None = None,
    ) -> "TranslationItem":
        """依照統一規則建立 TranslationItem。"""

        normalized_name = _normalize_text(original_name)
        if not normalized_name:
            raise ValueError("original_name 不可為空字串")

        normalized_parent = tuple(_normalize_text(p) for p in parent_chain)
        if not normalized_parent or not normalized_parent[0]:
            raise ValueError("parent_chain 至少需要包含國家碼")

        item_id = build_translation_item_id(level, normalized_parent, normalized_name)
        safe_metadata = MappingProxyType(dict(metadata or {}))
        return cls(
            id=item_id,
            level=level,
            original_name=normalized_name,
            source_lang=_normalize_text(source_lang),
            target_lang=_normalize_text(target_lang),
            parent_chain=normalized_parent,
            metadata=safe_metadata,
        )


@dataclass(slots=True)
class DatasetStats:
    """描述資料集規模。"""

    level: AdminLevel
    total: int
    unique_parent: int
    source_lang: str
    target_lang: str


class TranslationDataset(Sequence[TranslationItem]):
    """可供批次翻譯使用的資料集。"""

    def __init__(
        self,
        items: Iterable[TranslationItem],
        *,
        level: AdminLevel,
        source_lang: str,
        target_lang: str,
        deduplicated: bool,
    ) -> None:
        self._items = list(items)
        self.level = level
        self.source_lang = source_lang
        self.target_lang = target_lang
        self.deduplicated = deduplicated
        self.total = len(self._items)

        for item in self._items:
            if item.level != level:
                raise ValueError("所有 TranslationItem 必須擁有相同的 level")

        self._unique_parents = {item.parent_chain for item in self._items}

    def __len__(self) -> int:  # type: ignore[override]
        return self.total

    def __getitem__(self, index: int) -> TranslationItem:  # type: ignore[override]
        return self._items[index]

    def __iter__(self) -> Iterator[TranslationItem]:  # type: ignore[override]
        return iter(self._items)

    def stats(self) -> DatasetStats:
        """回傳資料集摘要資訊。"""

        return DatasetStats(
            level=self.level,
            total=self.total,
            unique_parent=len(self._unique_parents),
            source_lang=self.source_lang,
            target_lang=self.target_lang,
        )

    def as_sorted(
        self, sorter: Callable[[TranslationItem], Any] | None = None
    ) -> list[TranslationItem]:
        """依指定排序鍵輸出項目。"""

        if sorter is None:
            return list(self._items)
        return sorted(self._items, key=sorter)


class TranslationDatasetBuilder:
    """負責從 tabular 資料建構 dataset。"""

    def __init__(
        self,
        *,
        country_code: str,
        source_lang: str,
        target_lang: str,
    ) -> None:
        self.country_code = _normalize_text(country_code)
        if not self.country_code:
            raise ValueError("country_code 不可為空")
        self.source_lang = source_lang
        self.target_lang = target_lang

    def build_admin1(
        self,
        data: Any,
        *,
        name_field: str,
        metadata_fields: Sequence[str] | None = None,
    ) -> TranslationDataset:
        """根據資料列產生 Admin_1 資料集。"""

        records = list(self._to_records(data))
        items: list[TranslationItem] = []
        seen: dict[str, TranslationItem] = {}
        for idx, row in enumerate(records):
            name = _normalize_text(row.get(name_field))
            if not name:
                raise ValueError(f"第 {idx} 列缺少 {name_field} 欄位")

            metadata = self._collect_metadata(row, metadata_fields, idx)
            item = TranslationItem.from_values(
                level=AdminLevel.ADMIN_1,
                original_name=name,
                source_lang=self.source_lang,
                target_lang=self.target_lang,
                parent_chain=(self.country_code,),
                metadata=metadata,
            )
            seen[item.id] = item

        items.extend(seen.values())
        dataset = TranslationDataset(
            items,
            level=AdminLevel.ADMIN_1,
            source_lang=self.source_lang,
            target_lang=self.target_lang,
            deduplicated=True,
        )
        unique_parent = len({item.parent_chain for item in dataset})
        logger.info(
            f"Admin_1 dataset 建構完成：total={dataset.total} unique_parent={unique_parent}"
        )
        return dataset

    def build_admin2(
        self,
        data: Any,
        *,
        parent_field: str,
        name_field: str,
        metadata_fields: Sequence[str] | None = None,
        deduplicate: bool = False,
    ) -> TranslationDataset:
        """根據資料列產生 Admin_2 資料集。"""

        records = list(self._to_records(data))
        items: list[TranslationItem] = []
        seen: dict[str, TranslationItem] = {}

        for idx, row in enumerate(records):
            parent_name = _normalize_text(row.get(parent_field))
            name = _normalize_text(row.get(name_field))
            if not parent_name or not name:
                raise ValueError(f"第 {idx} 列缺少 {parent_field} 或 {name_field}")

            metadata = self._collect_metadata(row, metadata_fields, idx)
            item = TranslationItem.from_values(
                level=AdminLevel.ADMIN_2,
                original_name=name,
                source_lang=self.source_lang,
                target_lang=self.target_lang,
                parent_chain=(self.country_code, parent_name),
                metadata=metadata,
            )

            if deduplicate:
                seen[item.id] = item
            else:
                items.append(item)

        if deduplicate:
            items = list(seen.values())

        dataset = TranslationDataset(
            items,
            level=AdminLevel.ADMIN_2,
            source_lang=self.source_lang,
            target_lang=self.target_lang,
            deduplicated=deduplicate,
        )
        unique_parent = len({item.parent_chain for item in dataset})
        logger.info(
            f"Admin_2 dataset 建構完成：total={dataset.total} unique_parent={unique_parent}"
        )
        return dataset

    def _collect_metadata(
        self,
        row: Mapping[str, Any],
        metadata_fields: Sequence[str] | None,
        row_index: int,
    ) -> Mapping[str, Any]:
        base = {"row_index": row_index}
        if metadata_fields:
            for field_name in metadata_fields:
                base[field_name] = row.get(field_name)
        return base

    def _to_records(self, data: Any) -> Iterable[Mapping[str, Any]]:
        """將 DataFrame 或 iterable 轉換為 dict 列表。"""

        if data is None:
            return []

        # 優先支援 Polars
        if hasattr(data, "to_dicts") and callable(data.to_dicts):
            return data.to_dicts()

        # pandas DataFrame
        if hasattr(data, "to_dict"):
            try:
                return data.to_dict(orient="records")  # type: ignore[arg-type]
            except TypeError:
                pass

        if isinstance(data, Iterable):
            return data

        raise TypeError("不支援的資料來源型別")


class TranslationDataLoader(Iterable[list[TranslationItem]]):
    """依 batch 產出 TranslationItem。"""

    def __init__(
        self,
        dataset: TranslationDataset,
        *,
        batch_size: int,
        sorter: Callable[[TranslationItem], Any] | None = None,
        progress_callback: Callable[[int, int], None] | None = None,
    ) -> None:
        if batch_size <= 0:
            raise ValueError("batch_size 必須大於 0")
        self.dataset = dataset
        self.batch_size = batch_size
        self.sorter = sorter
        self.progress_callback = progress_callback

    def __iter__(self) -> Iterator[list[TranslationItem]]:
        items = self.dataset.as_sorted(self.sorter)
        total = len(items)
        processed = 0

        for start in range(0, total, self.batch_size):
            batch = items[start : start + self.batch_size]
            yield batch
            processed += len(batch)
            if self.progress_callback:
                self.progress_callback(processed, total)


class ProgressLogger:
    """控制 INFO log 的進度輸出頻率。"""

    def __init__(self, label: str) -> None:
        self.label = label
        self._last_percent = -1

    def __call__(self, processed: int, total: int) -> None:
        if total == 0:
            return
        percent = int((processed / total) * 100)
        if percent == self._last_percent:
            return
        if percent not in {0, 100} and percent % 5 != 0:
            return
        self._last_percent = percent
        logger.info(f"{self.label} 進度 {processed}/{total} ({percent}%)")


class BatchTranslationRunner:
    """協調批次翻譯三階段流程。"""

    def __init__(self, translator: "WikidataTranslator") -> None:
        self.translator = translator

    def run(
        self,
        dataset: TranslationDataset,
        *,
        batch_size: int,
        parent_qids: Mapping[str, str] | None,
        candidate_filter: "Callable[[str, dict], bool] | None",
        show_progress: bool,
    ) -> dict[str, dict]:
        if dataset.total == 0:
            logger.info(f"{dataset.level.value} 資料集為空，跳過翻譯")
            return {}

        parent_qids = parent_qids or {}
        logger.info(
            f"{dataset.level.value.upper()} 批次翻譯開始，筆數: {dataset.total}，batch_size={batch_size}"
        )

        progress_bar = None
        progress_logger = None

        def _noop_callback(processed: int, total: int) -> None:  # pragma: no cover
            del processed, total

        progress_callback: Callable[[int, int], None] = _noop_callback

        if show_progress:
            progress_bar = tqdm(
                total=dataset.total,
                desc=f"{dataset.level.value.upper()} 翻譯",
                unit="筆",
                leave=True,
            )

            def _progress_callback(processed: int, _: int) -> None:
                if progress_bar is not None:
                    progress_bar.update(processed - progress_bar.n)

            progress_callback = _progress_callback

        else:
            progress_logger = ProgressLogger(f"{dataset.level.value.upper()} 翻譯")

            def _logger_callback(processed: int, total: int) -> None:
                if progress_logger is not None:
                    progress_logger(processed, total)

            progress_callback = _logger_callback

        loader = TranslationDataLoader(
            dataset,
            batch_size=batch_size,
            progress_callback=progress_callback,
        )

        # === 階段 1 ===
        logger.info("階段 1/3: 搜尋 Wikidata 取得候選 QID...")
        search_results: dict[str, dict[str, Any]] = {}
        cache_hits = 0
        for batch in loader:
            for item in batch:
                cache_entry = self.translator.cache_store.get_translation(item)
                if cache_entry and "translated" in cache_entry:
                    search_results[item.id] = {
                        "item": item,
                        "cached": True,
                        "result": {
                            "translated": cache_entry.get(
                                "translated", item.original_name
                            ),
                            "qid": cache_entry.get("qid"),
                            "source": cache_entry.get("source", "cache"),
                            "used_lang": cache_entry.get("used_lang", "unknown"),
                            "parent_verified": cache_entry.get(
                                "parent_verified", False
                            ),
                        },
                    }
                    cache_hits += 1
                    continue

                candidate_qids = self.translator._search_wikidata(item)
                search_results[item.id] = {
                    "item": item,
                    "cached": False,
                    "qids": candidate_qids,
                }

        logger.info(f"階段 1 完成：快取命中 {cache_hits}/{dataset.total}")

        # === 階段 1.5: 候選過濾 ===
        if candidate_filter:
            logger.info("正在應用候選過濾器...")
            filtered_count = 0
            total_candidates = 0
            qids_for_filtering: list[str] = []

            for data in search_results.values():
                if data.get("cached"):
                    continue
                qids_list = data.get("qids", []) or []
                qids_for_filtering.extend(qids_list)

            filter_labels: dict[str, dict] = {}
            filter_instance_of: dict[str, list[str]] = {}
            if qids_for_filtering:
                filter_labels = self.translator._batch_get_labels(qids_for_filtering)
                filter_instance_of = self.translator._batch_get_instance_of(
                    qids_for_filtering
                )

            for data in search_results.values():
                if data.get("cached"):
                    continue
                qids_list = data.get("qids", []) or []
                if not qids_list:
                    continue
                original_count = len(qids_list)
                total_candidates += original_count
                filtered_qids = []
                item = data["item"]
                for qid in qids_list:
                    metadata = {
                        "qid": qid,
                        "labels": filter_labels.get(qid, {}),
                        "instance_of": filter_instance_of.get(qid, []),
                    }
                    if candidate_filter(item.original_name, metadata):
                        filtered_qids.append(qid)
                    else:
                        filtered_count += 1
                data["qids"] = filtered_qids

            if filtered_count > 0:
                logger.info(
                    f"候選過濾完成：從 {total_candidates} 個候選中排除 {filtered_count} 個"
                )

        # === 階段 2: 批次取得標籤 ===
        logger.info("階段 2/3: 批次取得所有 QID 的標籤...")
        all_qids: list[str] = []
        for data in search_results.values():
            if data.get("cached"):
                continue
            qids_list = data.get("qids", []) or []
            all_qids.extend(qids_list)

        all_labels: dict[str, dict] = {}
        if all_qids:
            all_labels = self.translator._batch_get_labels(all_qids)
            logger.info(f"成功取得 {len(all_labels)} 個 QID 的標籤")

        # === 階段 3: 處理結果 ===
        logger.info("階段 3/3: 處理翻譯結果...")
        result_bar = (
            tqdm(total=len(search_results), desc="處理結果", unit="筆", leave=True)
            if show_progress and search_results
            else None
        )
        results: dict[str, dict] = {}
        success_count = 0
        fallback_count = 0

        for item_id, data in search_results.items():
            item = data["item"]
            if data.get("cached"):
                results[item_id] = data.get("result", {})
                success_count += 1
                continue

            qids = data.get("qids", []) or []
            parent_qid = None
            if parent_qids:
                parent_qid = parent_qids.get(item_id) or parent_qids.get(
                    item.original_name
                )
            if not qids:
                result = {
                    "translated": item.original_name,
                    "qid": None,
                    "source": "original",
                    "used_lang": "original",
                    "parent_verified": False,
                }
                results[item_id] = result
                self.translator.cache_store.set_translation(item, result, parent_qid)
                fallback_count += 1
                continue

            selected_qid = None
            parent_verified = False

            if parent_qid:
                for qid in qids:
                    if self.translator._verify_p131(qid, parent_qid):
                        selected_qid = qid
                        parent_verified = True
                        break

            if not selected_qid:
                selected_qid = qids[0]

            labels = all_labels.get(selected_qid, {})
            translated, source, used_lang = self.translator._select_best_label(
                labels, item.original_name
            )

            result = {
                "translated": translated,
                "qid": selected_qid,
                "source": source,
                "used_lang": used_lang,
                "parent_verified": parent_verified,
            }
            results[item_id] = result
            success_count += 1

            self.translator.cache_store.set_translation(item, result, parent_qid)

            if result_bar is not None:
                result_bar.update(1)

        if result_bar is not None:
            result_bar.close()

        self.translator._flush_cache_if_needed(force=True)
        logger.info(
            f"{dataset.level.value.upper()} 批次翻譯完成：成功 {success_count}，回退 {fallback_count}，總筆數 {dataset.total}"
        )
        return results


class TranslationCacheStore:
    """集中管理 WikidataTranslator 使用的快取。"""

    VERSION = "1.0"

    def __init__(
        self,
        *,
        source_lang: str,
        target_lang: str,
        cache_path: Path | None,
    ) -> None:
        self.source_lang = source_lang
        self.target_lang = target_lang
        self.cache_path = cache_path
        self._dirty = 0
        self._last_flush = time.time()
        self.data = self._load()

    def _load(self) -> dict:
        if not self.cache_path or not self.cache_path.exists():
            return self._create_empty_payload()

        try:
            payload = json.loads(self.cache_path.read_text(encoding="utf-8"))
        except Exception as exc:  # pragma: no cover - I/O 失敗時回退
            logger.warning(f"快取載入失敗，將重新建立：{exc}")
            return self._create_empty_payload()

        version = payload.get("metadata", {}).get("version")
        if version != self.VERSION:
            self._backup_existing()
            return self._create_empty_payload()

        return payload

    def _backup_existing(self) -> None:
        if not self.cache_path or not self.cache_path.exists():
            return

        timestamp = datetime.now().strftime("%Y%m%d%H%M%S")
        backup_path = self.cache_path.with_suffix(
            f"{self.cache_path.suffix}.bak.{timestamp}"
        )
        try:
            self.cache_path.replace(backup_path)
            logger.info(f"已備份舊版快取至 {backup_path.name}")
        except Exception as exc:  # pragma: no cover - 失敗僅記錄
            logger.warning(f"舊版快取備份失敗：{exc}")

    def _create_empty_payload(self) -> dict:
        return {
            "metadata": {
                "source_lang": self.source_lang,
                "target_lang": self.target_lang,
                "created_at": datetime.now().isoformat(),
                "version": self.VERSION,
                "last_compacted_at": None,
            },
            "translations": {},
            "cache": {
                "search": {},
                "labels": {},
                "p131": {},
                "instance_of": {},
            },
            "indexes": {
                "by_name": {},
            },
        }

    def create_empty(self) -> dict:
        """提供測試快速產生全新快取字典。"""

        return self._create_empty_payload()

    def _ensure_index(self) -> None:
        self.data.setdefault("indexes", {}).setdefault("by_name", {})

    def _index_name(self, item: TranslationItem) -> None:
        self._ensure_index()
        by_name = self.data["indexes"]["by_name"]
        slots = by_name.setdefault(item.original_name, [])
        if item.id not in slots:
            slots.append(item.id)

    def get_translation(self, item: TranslationItem) -> dict | None:
        return self.data.get("translations", {}).get(item.id)

    def set_translation(
        self,
        item: TranslationItem,
        result: Mapping[str, Any],
        parent_qid: str | None,
    ) -> None:
        entry = {
            "original_name": item.original_name,
            "level": item.level.value,
            "parent_chain": list(item.parent_chain),
            "parent_qid": parent_qid,
            "cached_at": datetime.now().isoformat(),
        }
        entry.update(result)
        self.data.setdefault("translations", {})[item.id] = entry
        self._index_name(item)
        self.mark_dirty()

    def get_search_results(self, item: TranslationItem) -> list[str] | None:
        return self.data.get("cache", {}).get("search", {}).get(item.id)

    def set_search_results(self, item: TranslationItem, qids: list[str]) -> None:
        self.data.setdefault("cache", {}).setdefault("search", {})[item.id] = qids
        self.mark_dirty()

    def mark_dirty(self) -> None:
        self._dirty += 1
        self.flush_if_needed()

    def flush_if_needed(
        self,
        *,
        force: bool = False,
        max_dirty: int = 20,
        max_interval: float = 30.0,
    ) -> None:
        if not self.cache_path:
            self._dirty = 0
            self._last_flush = time.time()
            return

        now = time.time()
        if not force:
            if self._dirty < max_dirty and (now - self._last_flush) < max_interval:
                return

        self.save()
        self._dirty = 0
        self._last_flush = now

    def save(self) -> None:
        if not self.cache_path:
            return

        try:
            self.cache_path.parent.mkdir(parents=True, exist_ok=True)
            tmp_path = self.cache_path.with_suffix(self.cache_path.suffix + ".tmp")
            tmp_path.write_text(
                json.dumps(self.data, ensure_ascii=False, indent=2), encoding="utf-8"
            )
            tmp_path.replace(self.cache_path)
        except Exception as exc:  # pragma: no cover - I/O 失敗時記錄
            logger.warning(f"快取儲存失敗: {exc}")


try:
    from opencc import OpenCC

    OPENCC_AVAILABLE = True
except ImportError:
    OPENCC_AVAILABLE = False
    logger.warning("OpenCC 未安裝，簡體中文轉繁體功能將不可用")


class WikidataTranslator:
    """通用的 Wikidata 地名翻譯工具。

    Attributes:
        source_lang: 來源語言代碼（如 'ko', 'vi', 'th'）
        target_lang: 目標語言代碼（如 'zh-tw', 'zh-hant'）
        fallback_langs: 回退語言列表
        cache_path: 快取檔案路徑
        use_opencc: 是否使用 OpenCC 簡轉繁
    """

    # API 端點
    WDQS_URL = "https://query.wikidata.org/sparql"
    WDACT_URL = "https://www.wikidata.org/w/api.php"
    ZHWIKI_URL = "https://zh.wikipedia.org/w/api.php"

    # 速率限制（秒）
    THROTTLE_WDQS = 0.8
    THROTTLE_WDACT = 0.2
    THROTTLE_ZHWIKI = 0.2

    # 重試設定
    MAX_RETRIES = 5

    def __init__(
        self,
        source_lang: str,
        target_lang: str,
        fallback_langs: list[str] | None = None,
        cache_path: str | None = None,
        use_opencc: bool = True,
    ):
        """初始化翻譯工具。

        Args:
            source_lang: 來源語言代碼（如 'ko'）
            target_lang: 目標語言代碼（如 'zh-tw'）
            fallback_langs: 回退語言列表（預設為 ['zh-hant', 'zh', 'en', source_lang]）
            cache_path: 快取檔案路徑（預設不使用快取）
            use_opencc: 是否使用 OpenCC 簡轉繁（預設 True）
        """
        self.source_lang = source_lang
        self.target_lang = target_lang
        self.fallback_langs = fallback_langs or [
            "zh-hant",
            "zh",
            "en",
            source_lang,
        ]
        self.use_opencc = use_opencc and OPENCC_AVAILABLE

        # 初始化 OpenCC
        if self.use_opencc:
            try:
                self.opencc = OpenCC("s2twp")  # 簡體轉繁體（台灣用語）
            except Exception as e:
                logger.warning(f"OpenCC 初始化失敗: {e}")
                self.use_opencc = False

        # 初始化 HTTP Session
        self.session = requests.Session()
        self.session.headers.update(
            {"User-Agent": "immich-geodata-zh-tw/1.0 (Wikidata Translation Tool)"}
        )

        # 初始化快取
        self.cache_path = Path(cache_path) if cache_path else None
        self.cache_store = TranslationCacheStore(
            source_lang=self.source_lang,
            target_lang=self.target_lang,
            cache_path=self.cache_path,
        )
        self.cache = self._load_cache()

    def _create_empty_cache(self) -> dict:
        """提供兼容舊測試的空白快取。"""

        return self.cache_store.create_empty()

    def _load_cache(self) -> dict:
        """取得目前快取內容。"""

        return self.cache_store.data

    def _save_cache(self) -> None:
        self.cache_store.save()

    def _flush_cache_if_needed(
        self,
        *,
        force: bool = False,
        max_dirty: int = 20,
        max_interval: float = 30.0,
    ) -> None:
        """依照髒污次數或時間間隔決定是否儲存快取。"""

        self.cache_store.flush_if_needed(
            force=force, max_dirty=max_dirty, max_interval=max_interval
        )

    def _mark_cache_dirty(self) -> None:
        self.cache_store.mark_dirty()

    def _request_json(
        self, url: str, params: dict | None = None, throttle: float = 0.0
    ) -> dict:
        """發送 HTTP 請求並回傳 JSON（含重試機制）。

        Args:
            url: 請求 URL
            params: 請求參數
            throttle: 請求後延遲秒數

        Returns:
            JSON 回應

        Raises:
            requests.RequestException: 請求失敗
        """
        last_err = None
        for attempt in range(self.MAX_RETRIES):
            try:
                response = self.session.get(url, params=params, timeout=30)

                # 處理 429 (Too Many Requests)
                if response.status_code == 429:
                    retry_after = int(response.headers.get("Retry-After", 5))
                    logger.warning(f"速率限制，等待 {retry_after} 秒後重試...")
                    time.sleep(retry_after)
                    continue

                response.raise_for_status()

                # 速率限制
                if throttle > 0:
                    time.sleep(throttle)

                return response.json()

            except requests.RequestException as e:
                last_err = e
                base_wait_time = 2 * (attempt + 1)
                # Reason: 加入 ±20% 抖動避免多個請求同時重試（羊群效應）
                jitter = random.uniform(-0.2, 0.2)
                wait_time = base_wait_time * (1 + jitter)
                logger.warning(
                    f"請求失敗（第 {attempt + 1} 次），等待 {wait_time:.2f} 秒後重試..."
                )
                time.sleep(wait_time)

        raise last_err  # type: ignore

    def _wdqs(self, query: str) -> dict:
        """執行 Wikidata SPARQL 查詢。"""
        params = {"query": query, "format": "json"}
        return self._request_json(
            self.WDQS_URL, params=params, throttle=self.THROTTLE_WDQS
        )

    def _wd_api(self, params: dict) -> dict:
        """呼叫 Wikidata API。"""
        p = {"format": "json", **params}
        return self._request_json(
            self.WDACT_URL, params=p, throttle=self.THROTTLE_WDACT
        )

    def _zhwiki_convert_title(self, title: str) -> str:
        """使用中文維基百科的 converttitles API 轉換標題。"""
        try:
            js = self._request_json(
                self.ZHWIKI_URL,
                params={
                    "action": "query",
                    "format": "json",
                    "converttitles": 1,
                    "titles": title,
                },
                throttle=self.THROTTLE_ZHWIKI,
            )

            query = js.get("query", {})

            # 檢查是否有轉換結果
            if "converted" in query:
                return query["converted"][0].get("to", title)

            # 備用：從 pages 取得標題
            pages = query.get("pages", {})
            if pages and isinstance(pages, dict):
                return next(iter(pages.values())).get("title", title)

            return title

        except Exception as e:
            logger.warning(f"Wikipedia 標題轉換失敗: {e}")
            return title

    def _search_wikidata(self, item: TranslationItem, limit: int = 7) -> list[str]:
        """搜尋 Wikidata 實體，並以 context-aware key 快取結果。"""

        cached_qids = self.cache_store.get_search_results(item)
        if cached_qids is not None:
            return cached_qids

        try:
            js = self._wd_api(
                {
                    "action": "wbsearchentities",
                    "search": item.original_name,
                    "language": self.source_lang,
                    "uselang": self.source_lang,
                    "type": "item",
                    "limit": str(limit),
                }
            )

            qids = [entry["id"] for entry in js.get("search", [])]

            self.cache_store.set_search_results(item, qids)
            return qids

        except Exception as e:
            logger.warning(f"Wikidata 搜尋失敗 ({item.original_name}): {e}")
            return []

    def _verify_p131(self, candidate_qid: str, parent_qid: str) -> bool:
        """驗證候選 QID 是否位於父級 QID 之內（P131 關係）。

        Args:
            candidate_qid: 候選實體 QID
            parent_qid: 父級實體 QID

        Returns:
            是否符合 P131 關係
        """
        cache_key = f"{candidate_qid}_{parent_qid}"
        # 檢查快取（新路徑）
        if cache_key in self.cache.get("cache", {}).get("p131", {}):
            return self.cache["cache"]["p131"][cache_key]

        try:
            query = f"ASK {{ wd:{candidate_qid} (wdt:P131)+ wd:{parent_qid} . }}"
            result = self._wdqs(query)
            answer = bool(result.get("boolean"))

            # 快取驗證結果（新路徑）
            self.cache.setdefault("cache", {}).setdefault("p131", {})[cache_key] = (
                answer
            )
            self._mark_cache_dirty()

            return answer

        except Exception as e:
            logger.warning(f"P131 驗證失敗 ({candidate_qid}, {parent_qid}): {e}")
            return False

    def _get_labels(self, qid: str) -> dict:
        """取得實體的多語言標籤。

        Args:
            qid: Wikidata QID

        Returns:
            語言代碼 -> 標籤的對照表
        """
        # 檢查快取（新路徑）
        if qid in self.cache.get("cache", {}).get("labels", {}):
            return self.cache["cache"]["labels"][qid]

        try:
            # 構建語言列表（目標語言 + 回退語言）
            langs = [self.target_lang] + self.fallback_langs
            langs_str = "|".join(set(langs))

            js = self._wd_api(
                {
                    "action": "wbgetentities",
                    "ids": qid,
                    "props": "labels|sitelinks",
                    "languages": langs_str,
                }
            )

            entity = js.get("entities", {}).get(qid, {})
            labels_data = entity.get("labels", {})
            sitelinks = entity.get("sitelinks", {})

            # 整理標籤
            labels = {lang: labels_data[lang]["value"] for lang in labels_data}

            # 加入中文維基百科標題（如果有）
            zhwiki_title = sitelinks.get("zhwiki", {}).get("title")
            if zhwiki_title:
                labels["zhwiki"] = zhwiki_title

            # 快取標籤（新路徑）
            self.cache.setdefault("cache", {}).setdefault("labels", {})[qid] = labels
            self._mark_cache_dirty()

            return labels

        except Exception as e:
            logger.warning(f"取得標籤失敗 ({qid}): {e}")
            return {}

    def _batch_get_labels(
        self, qids: list[str], batch_size: int = 50
    ) -> dict[str, dict]:
        """批次取得多個 QID 的標籤。

        利用 Wikidata wbgetentities API 支援一次查詢多個實體（最多 50 個）的特性，
        大幅減少 API 請求次數。

        Args:
            qids: QID 列表
            batch_size: 每批查詢數量（Wikidata API 限制最多 50）

        Returns:
            {qid: {language: label}} 對照表
        """
        # 步驟 1: 去重
        unique_qids = list(set(qids))

        # 步驟 2: 檢查快取，過濾未快取的 QID
        uncached_qids = [
            qid
            for qid in unique_qids
            if qid not in self.cache.get("cache", {}).get("labels", {})
        ]

        # 步驟 3: 分批查詢（每批最多 50 個）
        if uncached_qids:
            logger.info(
                f"需要批次查詢 {len(uncached_qids)} 個 QID 的標籤"
                f"（分 {(len(uncached_qids) + batch_size - 1) // batch_size} 批）"
            )

            for i in range(0, len(uncached_qids), batch_size):
                batch = uncached_qids[i : i + batch_size]
                ids_str = "|".join(batch)  # Q8684|Q41164|Q515

                try:
                    # 構建語言列表（目標語言 + 回退語言）
                    langs = [self.target_lang] + self.fallback_langs
                    langs_str = "|".join(set(langs))

                    # 批次 API 請求
                    # Reason: 使用 | 分隔多個 QID，一次請求取得多個實體的標籤
                    js = self._wd_api(
                        {
                            "action": "wbgetentities",
                            "ids": ids_str,
                            "props": "labels|sitelinks",
                            "languages": langs_str,
                        }
                    )

                    # 解析結果並快取
                    for qid, entity in js.get("entities", {}).items():
                        labels_data = entity.get("labels", {})
                        sitelinks = entity.get("sitelinks", {})

                        # 整理標籤
                        labels = {
                            lang: labels_data[lang]["value"] for lang in labels_data
                        }

                        # 加入中文維基百科標題（如果有）
                        zhwiki_title = sitelinks.get("zhwiki", {}).get("title")
                        if zhwiki_title:
                            labels["zhwiki"] = zhwiki_title

                        # 快取標籤
                        self.cache.setdefault("cache", {}).setdefault("labels", {})[
                            qid
                        ] = labels
                        self._mark_cache_dirty()

                    logger.debug(
                        f"批次 {i // batch_size + 1}: 成功查詢 {len(batch)} 個 QID"
                    )

                except Exception as e:
                    logger.warning(
                        f"批次取得標籤失敗（批次 {i // batch_size + 1}）: {e}"
                    )
                    # Reason: 批次查詢失敗時繼續處理下一批，避免全部失敗
                    continue

        # 步驟 4: 回傳所有 QID 的標籤（含快取）
        return {
            qid: self.cache["cache"]["labels"][qid]
            for qid in unique_qids
            if qid in self.cache["cache"]["labels"]
        }

    def _batch_get_instance_of(
        self, qids: list[str], batch_size: int = 50
    ) -> dict[str, list[str]]:
        """批次取得多個 QID 的 P31（instance of）屬性。

        Args:
            qids: QID 列表
            batch_size: 每批查詢數量（Wikidata API 限制最多 50）

        Returns:
            {qid: [P31_qid1, P31_qid2, ...]} 對照表
        """
        # 步驟 1: 去重
        unique_qids = list(set(qids))

        # 步驟 2: 檢查快取，過濾未快取的 QID
        uncached_qids = [
            qid
            for qid in unique_qids
            if qid not in self.cache.get("cache", {}).get("instance_of", {})
        ]

        # 步驟 3: 分批查詢（每批最多 50 個）
        if uncached_qids:
            logger.info(
                f"需要批次查詢 {len(uncached_qids)} 個 QID 的 P31 屬性"
                f"（分 {(len(uncached_qids) + batch_size - 1) // batch_size} 批）"
            )

            for i in range(0, len(uncached_qids), batch_size):
                batch = uncached_qids[i : i + batch_size]
                ids_str = "|".join(batch)

                try:
                    # 批次 API 請求
                    js = self._wd_api(
                        {
                            "action": "wbgetentities",
                            "ids": ids_str,
                            "props": "claims",
                            "languages": "en",  # P31 不需要多語言
                        }
                    )

                    # 解析結果並快取
                    for qid, entity in js.get("entities", {}).items():
                        claims = entity.get("claims", {})
                        p31_claims = claims.get("P31", [])

                        # 提取 P31 的 QID 列表
                        instance_of_qids = []
                        for claim in p31_claims:
                            try:
                                mainsnak = claim.get("mainsnak", {})
                                if mainsnak.get("snaktype") == "value":
                                    datavalue = mainsnak.get("datavalue", {})
                                    if datavalue.get("type") == "wikibase-entityid":
                                        p31_qid = datavalue["value"]["id"]
                                        instance_of_qids.append(p31_qid)
                            except (KeyError, TypeError):
                                continue

                        # 快取 P31 資訊
                        self.cache.setdefault("cache", {}).setdefault(
                            "instance_of", {}
                        )[qid] = instance_of_qids
                        self._mark_cache_dirty()

                    logger.debug(
                        f"批次 {i // batch_size + 1}: 成功查詢 {len(batch)} 個 QID 的 P31"
                    )

                except Exception as e:
                    logger.warning(
                        f"批次取得 P31 失敗（批次 {i // batch_size + 1}）: {e}"
                    )
                    # Reason: 批次查詢失敗時繼續處理下一批，避免全部失敗
                    continue

        # 步驟 4: 回傳所有 QID 的 P31（含快取）
        return {
            qid: self.cache["cache"]["instance_of"][qid]
            for qid in unique_qids
            if qid in self.cache["cache"]["instance_of"]
        }

    def _select_best_label(self, labels: dict, name: str) -> tuple[str, str, str]:
        """從多語言標籤中選擇最佳翻譯。

        Args:
            labels: 語言代碼 -> 標籤的對照表
            name: 原始名稱（作為最終備案）

        Returns:
            (翻譯結果, 來源標記, 使用的語言)
        """
        # 1. 優先使用目標語言
        if self.target_lang in labels:
            return labels[self.target_lang], "wikidata", self.target_lang

        # 2. 使用回退語言
        for lang in self.fallback_langs:
            if lang in labels:
                # 如果是簡體中文（zh），嘗試轉繁體
                if lang == "zh" and self.use_opencc:
                    try:
                        traditional = self.opencc.convert(labels[lang])
                        return traditional, "opencc", "zh→zh-tw"
                    except Exception as e:
                        logger.warning(f"OpenCC 轉換失敗: {e}")
                        return labels[lang], "wikidata-zh", "zh"
                return labels[lang], f"wikidata-{lang}", lang

        # 3. 使用中文維基百科標題（並轉換成繁體）
        if "zhwiki" in labels:
            try:
                converted = self._zhwiki_convert_title(labels["zhwiki"])
                return converted, "zhwiki-convert", "zhwiki"
            except Exception:
                return labels["zhwiki"], "zhwiki", "zhwiki"

        # 4. 最終備案：使用原始名稱
        return name, "original", "original"

    def translate(
        self,
        name: str,
        parent_qid: str | None = None,
        instance_of_qid: str | None = None,
    ) -> dict:
        """翻譯單一地名（沿用批次核心邏輯）。"""

        _ = instance_of_qid  # 保留參數以維持 API 一致性
        normalized_name = _normalize_text(name)
        if not normalized_name:
            raise ValueError("name 不可為空字串")

        item = TranslationItem.from_values(
            level=AdminLevel.ADMIN_1,
            original_name=normalized_name,
            source_lang=self.source_lang,
            target_lang=self.target_lang,
            parent_chain=(self.source_lang.upper() or "GENERIC",),
        )
        dataset = TranslationDataset(
            [item],
            level=AdminLevel.ADMIN_1,
            source_lang=self.source_lang,
            target_lang=self.target_lang,
            deduplicated=True,
        )
        parent_map = {item.id: parent_qid} if parent_qid else None
        results = self.batch_translate(
            dataset,
            batch_size=1,
            parent_qids=parent_map,
            show_progress=False,
        )
        return results.get(
            item.id,
            {
                "translated": normalized_name,
                "qid": None,
                "source": "original",
                "used_lang": "original",
                "parent_verified": False,
            },
        )

    def batch_translate(
        self,
        dataset: TranslationDataset,
        *,
        batch_size: int = 20,
        parent_qids: Mapping[str, str] | None = None,
        show_progress: bool = True,
        candidate_filter: "Callable[[str, dict], bool] | None" = None,
    ) -> dict[str, dict]:
        """針對 TranslationDataset 執行批次翻譯。"""

        runner = BatchTranslationRunner(self)
        return runner.run(
            dataset,
            batch_size=batch_size,
            parent_qids=parent_qids,
            candidate_filter=candidate_filter,
            show_progress=show_progress,
        )
