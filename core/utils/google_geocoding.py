"""Google Geocoding API 通用客戶端。

此模組提供 Google Geocoding API 的通用封裝，支援：
- 正向地理編碼（地址 → 座標）
- 反向地理編碼（座標 → 地址）
- 自動速率限制控制
- 地址組件提取

使用範例：
    client = GoogleGeocodingClient(api_key="YOUR_API_KEY")

    # 反向地理編碼
    data = client.geocode(latlng=(37.5742, 126.9770), language="zh-TW")
    admin1 = client.extract_component(data, ["administrative_area_level_1"])
"""

import time

import requests

from core.utils import logger


class GoogleGeocodingClient:
    """Google Geocoding API 客戶端（支援正向與反向編碼）。

    此類別封裝 Google Geocoding API 的呼叫邏輯，提供：
    - 自動速率限制（每秒最多 50 次請求）
    - 錯誤處理與重試機制
    - 靈活的地址組件提取

    Attributes:
        api_key: Google Cloud API 金鑰
        session: HTTP 連線會話
        request_count: 已執行的 API 請求次數
        last_request_time: 上次請求的時間戳記
    """

    BASE_URL = "https://maps.googleapis.com/maps/api/geocode/json"
    RATE_LIMIT_DELAY = 0.021  # 每秒最多 50 次請求（稍微保守）

    def __init__(self, api_key: str):
        """初始化客戶端。

        Args:
            api_key: Google Cloud API 金鑰
        """
        self.api_key = api_key
        self.session = requests.Session()
        self.request_count = 0
        self.last_request_time = 0.0

    def _rate_limit(self) -> None:
        """速率限制控制（每秒最多 50 次）。

        在每次 API 呼叫前執行，確保不超過速率限制。
        """
        current_time = time.time()
        elapsed = current_time - self.last_request_time
        if elapsed < self.RATE_LIMIT_DELAY:
            time.sleep(self.RATE_LIMIT_DELAY - elapsed)
        self.last_request_time = time.time()

    def geocode(
        self,
        address: str | None = None,
        latlng: tuple[float, float] | None = None,
        language: str = "zh-TW",
        region: str | None = None,
    ) -> dict | None:
        """正向或反向地理編碼。

        Args:
            address: 要查詢的地址（正向編碼時使用）
            latlng: 座標 (latitude, longitude)（反向編碼時使用）
            language: 回應語言（預設為繁體中文）
            region: 區域偏好（如 "kr" 代表韓國）

        Returns:
            API 回應的 JSON 資料，若失敗則回傳 None

        Raises:
            ValueError: 同時提供 address 和 latlng 或兩者皆無時拋出

        使用範例:
            # 正向編碼
            data = client.geocode(address="서울특별시", region="kr")

            # 反向編碼
            data = client.geocode(latlng=(37.5742, 126.9770), language="zh-TW")
        """
        # Reason: 確保只使用一種編碼方式
        if (address is None and latlng is None) or (
            address is not None and latlng is not None
        ):
            raise ValueError("必須提供 address 或 latlng 其中之一（不可同時提供）")

        self._rate_limit()

        # 建立 API 請求參數
        params = {
            "language": language,
            "key": self.api_key,
        }

        if address:
            params["address"] = address
        elif latlng:
            # Reason: Google API 要求格式為 "lat,lng"
            params["latlng"] = f"{latlng[0]},{latlng[1]}"

        if region:
            params["region"] = region

        try:
            response = self.session.get(self.BASE_URL, params=params, timeout=10)
            response.raise_for_status()
            data = response.json()

            self.request_count += 1

            if data["status"] == "OK":
                return data
            elif data["status"] == "ZERO_RESULTS":
                logger.warning(f"查無結果: {address or latlng}")
                return None
            else:
                logger.error(f"API 錯誤 ({data['status']}): {address or latlng}")
                return None

        except requests.exceptions.RequestException as e:
            logger.error(f"請求失敗: {address or latlng} - {e}")
            return None

    def extract_component(
        self,
        data: dict | None,
        component_types: list[str],
    ) -> str | None:
        """從 API 回應中提取指定類型的地址組件。

        根據 Google Geocoding API 官方文件：
        - results 陣列按照「最精確到最不精確」排序
        - 第一個符合類型的結果通常是最準確的
        - 優先選擇包含繁體中文字符的組件

        策略：
        1. 按 results 順序遍歷（優先處理最精確的結果）
        2. 對於每個指定類型，收集所有候選組件
        3. 優先選擇包含繁體中文字符的組件
        4. 若無中文組件，回傳第一個符合類型的組件

        Args:
            data: Google Geocoding API 回應資料
            component_types: 地址組件類型列表（按優先順序排列）
                           如 ["locality", "sublocality_level_1"]

        Returns:
            符合條件的繁體中文地名，若未找到則回傳 None

        使用範例:
            # 提取 admin_1（廣域市/道）
            admin1 = client.extract_component(
                data, ["administrative_area_level_1"]
            )

            # 提取 admin_2（市/區/郡），支援多種類型
            admin2 = client.extract_component(
                data, ["locality", "sublocality_level_1"]
            )
        """
        if not data or "results" not in data or not data["results"]:
            return None

        # Reason: 按 component_types 優先順序遍歷
        for component_type in component_types:
            candidates: list[str] = []

            # Reason: 按 results 順序收集符合類型的候選（保持 API 的精確度排序）
            for result in data["results"]:
                for component in result.get("address_components", []):
                    if component_type in component.get("types", []):
                        long_name = component.get("long_name")
                        # Reason: 避免重複候選
                        if long_name and long_name not in candidates:
                            candidates.append(long_name)

            if not candidates:
                continue

            # Reason: 優先選擇包含繁體中文字符的組件（Unicode 範圍 U+4E00 到 U+9FFF）
            for candidate in candidates:
                if any("\u4e00" <= char <= "\u9fff" for char in candidate):
                    return candidate

            # Reason: 若無中文組件，回傳第一個候選（最精確的結果）
            return candidates[0]

        return None
