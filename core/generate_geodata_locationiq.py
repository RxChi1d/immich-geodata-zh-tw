import os
import sys
import time
import argparse

import requests
from requests.adapters import HTTPAdapter, Retry
import polars as pl
from tqdm import tqdm

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.utils import logger, ensure_folder_exists
from core.schemas import CITIES_SCHEMA, GEODATA_SCHEMA
from core.geodata import get_handler

# 取得臺灣處理器的 MUNICIPALITIES
TaiwanHandler = get_handler("TW")
MUNICIPALITIES = TaiwanHandler.MUNICIPALITIES


s = requests.Session()

retries = Retry(total=5, backoff_factor=0.1, status_forcelist=[403, 500, 502, 503, 504])

s.mount("https://", HTTPAdapter(max_retries=retries))

# 預設值（若沒有設定，可能會在使用時出現錯誤）
LOCATIONIQ_API_KEY = None
LOCATIONIQ_QPS = 1


def set_locationiq_config(api_key, qps):
    """
    設定 LocationIQ 相關參數。

    Args:
        api_key (str): LocationIQ API Key
        qps (int): 每秒查詢次數限制
    """
    global LOCATIONIQ_API_KEY, LOCATIONIQ_QPS
    LOCATIONIQ_API_KEY = api_key
    LOCATIONIQ_QPS = qps


def get_loc_from_locationiq(lat, lon):
    """
    使用 LocationIQ API 根據經緯度取得地理位置資訊。

    Args:
        lat (float): 經度
        lon (float): 緯度

    Returns:
        dict: 包含地理位置資訊的字典，如果查詢失敗則返回 None。
    """

    url = "https://us1.locationiq.com/v1/reverse"

    params = {
        "lat": lat,
        "lon": lon,
        "format": "json",
        "accept-language": "zh,en",
        "normalizeaddress": 1,
        "normalizecity": 1,
        "key": LOCATIONIQ_API_KEY,
    }

    headers = {"accept": "application/json"}
    try:
        response = s.get(url, headers=headers, params=params)
        time.sleep(1.02 / LOCATIONIQ_QPS)
        if response.status_code == 200:
            return response.json()
    except requests.exceptions.RequestException as e:
        logger.error(f"{lat},{lon} 查詢失敗: {e}")
        pass
    return None


def save_to_csv(data: pl.DataFrame, output_file: str):
    """
    將資料儲存到CSV檔案中。

    Args:
        data (pl.DataFrame): 要儲存的資料框。
        output_file (str): 輸出的CSV檔案路徑。

    Returns:
        None: 此函數沒有回傳值。

    注意:
        - 如果資料框是空的，函數會直接返回，避免寫入空檔案。
        - 如果輸出的CSV檔案已經存在，會先讀取現有的資料並與新資料合併，然後一次性寫入（覆蓋舊檔案）。
    """

    if data.is_empty():
        return  # 避免寫入空檔案

    if os.path.exists(output_file):
        existing_data = pl.read_csv(output_file, schema=GEODATA_SCHEMA)
        data = existing_data.vstack(data)

    # 一次性寫入（覆蓋舊檔案）
    data.write_csv(output_file, include_header=True)


def reverse_query(coordinate):
    """
    根據提供的座標進行反向地理編碼查詢，並返回包含地理資訊的資料框。

    Args:
        coordinate (dict): 包含 "lat" 和 "lon" 的字典，分別代表緯度和經度。

    Returns:
        pl.DataFrame: 包含地理資訊的資料框，包括國家、省、市、區、鄰里等資訊。
        如果查詢失敗，返回 None。
    """

    response = get_loc_from_locationiq(coordinate["lat"], coordinate["lon"])

    if response:
        address = response["address"]

        return pl.DataFrame(
            {
                "latitude": [coordinate["lat"]],
                "longitude": [coordinate["lon"]],
                "country": [address["country"]],
                "admin_1": [address.get("state", "")],
                "admin_2": [address.get("city", address.get("county", ""))],
                "admin_3": [address.get("suburb", "")],
                "admin_4": [address.get("neighbourhood", "")],
            },
            schema=GEODATA_SCHEMA,
            strict=False,
        )

    else:
        return None


def process_file(cities500_file, output_file, country_code, batch_size=100):
    """
    處理 cities500.txt 檔案，通過 LocationIQ 生成指定國家的 metadata，並將結果儲存到指定的輸出檔案。

    Args:
        cities500_file (str): cities500.txt 檔案的路徑。
        output_file (str): 輸出 metadata 的 CSV 檔案路徑。
        country_code (str): 指定國家的 ISO 3166-1 alpha-2 國家代碼。
        batch_size (int, optional): 每次寫入 CSV 的批次大小，預設為 100。

    Returns:
        None
    """

    logger.info(f"通過 LocationIQ 生成 {country_code} 的 metadata")
    logger.info(f"LocationIQ API Key: {LOCATIONIQ_API_KEY}")
    logger.info(f"LocationIQ QPS: {LOCATIONIQ_QPS}")

    # 嘗試讀取已存在的 meta_data (恢復進度)
    existing_data = (
        pl.read_csv(
            output_file,
            schema=GEODATA_SCHEMA,
        )
        if os.path.exists(output_file)
        else pl.DataFrame(schema=GEODATA_SCHEMA)
    )

    # 建立已查詢座標的 Hash Set，加快查找速度
    existing_coords = set(zip(existing_data["longitude"], existing_data["latitude"]))

    # 讀取 cities500.txt
    cities_df = pl.read_csv(
        cities500_file,
        separator="\t",
        has_header=False,
        schema=CITIES_SCHEMA,
    )

    # 讀取臺灣行政區對照表
    admin1_map = pl.read_csv(
        os.path.join("output", "tw_admin1_map.csv"),
    )

    # 篩選指定國家
    specific_country_df = cities_df.filter(pl.col("country_code") == country_code)

    # 初始化空 DataFrame 來儲存 API 查詢結果
    result_df = pl.DataFrame(schema=GEODATA_SCHEMA)
    pbar = tqdm(
        specific_country_df.iter_rows(named=True), total=specific_country_df.height
    )
    for row in pbar:
        pbar.set_description(f"查詢城市: {row['name']}")

        loc = {"lon": row["longitude"], "lat": row["latitude"]}

        # 如果座標已經存在，跳過查詢
        if (loc["lon"], loc["lat"]) in existing_coords:
            continue

        try:
            # 執行 API 查詢，返回 Polars DataFrame
            record_df = reverse_query(loc)

            # 如果 API 返回 None，則記錄錯誤並跳過
            if record_df is None or record_df.is_empty():
                logger.warning(
                    f"查詢失敗，geoname_id: {row['geoname_id']}, 座標: {loc}"
                )
                continue

            """
            1. 直轄市/省轄市
                1.1. admin_2 在列表中
                1.2. 根據 row 的 admin1_code ，在 admin1_map 的 new_id 中找到對應的中文名 (TW.{admin1_code})，填入 admin_1
                1.3. admin_3 的數值填入 admin_2
                1.4. admin_4 的數值填入 admin_3
                1.5. 空值填入 admin_4
            
            2. 省轄縣
                2.1. admin_2 不會在列表中
                2.2. 根據 row 的 admin1_code ，在 admin1_map 的 new_id 中找到對應的中文名 (TW.{admin1_code})，填入 admin_1
                
            """

            # 臺灣特殊處理
            if country_code == "TW":
                admin_1 = f"TW.{row['admin1_code']}"

                # 直轄市/省轄市
                if record_df["admin_2"].item() in MUNICIPALITIES:
                    record_df = record_df.with_columns(
                        pl.lit(
                            admin1_map.filter(pl.col("new_id") == admin_1)[
                                "name"
                            ].item()
                        ).alias("admin_1"),
                        pl.col("admin_3").alias("admin_2"),
                        pl.col("admin_4").alias("admin_3"),
                        pl.lit(None).alias("admin_4"),
                    )

                # 省轄縣
                else:
                    record_df = record_df.with_columns(
                        pl.lit(
                            admin1_map.filter(pl.col("new_id") == admin_1)[
                                "name"
                            ].item()
                        ).alias("admin_1")
                    )

            # 合併結果
            result_df = result_df.vstack(record_df)

            # 當 `batch_size` 達到指定值時，寫入 CSV
            if result_df.height >= batch_size:
                save_to_csv(result_df, output_file)
                result_df = pl.DataFrame(schema=GEODATA_SCHEMA)  # 清空 DataFrame

        except Exception as e:
            # API 出錯時，立即寫入當前累積的數據
            save_to_csv(result_df, output_file)

            logger.critical(f"API 錯誤: {e}，座標: {loc}")
            sys.exit(1)

    # 最後一次儲存剩餘的結果，確保剩餘資料被儲存
    if result_df.height > 0:
        save_to_csv(result_df, output_file)

    logger.info(f"已生成 {country_code} 的 metadata")


def test():
    parser = argparse.ArgumentParser()
    # 加入 LocationIQ 的參數
    parser.add_argument(
        "--locationiq-api-key", type=str, required=True, help="LocationIQ API Key"
    )
    parser.add_argument(
        "--locationiq-qps", type=int, default=1, help="LocationIQ 每秒查詢次數限制"
    )

    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--country-code", type=str, default="TW")
    parser.add_argument("--output-folder", type=str, default="./output")
    args = parser.parse_args()

    # 使用參數設定 LocationIQ 配置
    set_locationiq_config(args.locationiq_api_key, args.locationiq_qps)

    meta_data_folder = "./meta_data"
    cities500_file = os.path.join(args.output_folder, "cities500_optimized.txt")

    if not os.path.exists(cities500_file):
        logger.critical(f"{cities500_file} 不存在，請先下載。")
        sys.exit(1)

    output_file = os.path.join(meta_data_folder, f"{args.country_code}.csv")
    ensure_folder_exists(output_file)

    process_file(cities500_file, output_file, args.country_code, args.overwrite)


if __name__ == "__main__":
    logger.error(
        "請使用 main.py 作為主要接口，而非直接執行 generate_geodata_locationiq.py"
    )
