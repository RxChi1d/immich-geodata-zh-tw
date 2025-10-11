import os
import sys
import polars as pl
from pathlib import Path

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.utils import logger, calculate_global_max_geoname_id
from core.schemas import CITIES_SCHEMA, ADMIN1_SCHEMA
from core.geodata import get_handler, get_all_handlers


def update_geodata(
    cities_file: str,
    extra_files: list[str],
    output_file: str,
    min_population: int = 100,
) -> None:
    """更新地理資料檔案，包含 admin1CodesASCII.txt 和 cities500.txt。

    此函數為地理資料更新流程的主要入口點，協調整個處理流程並確保資料完整性。
    處理順序遵循資料依賴關係：先建立 admin1 記錄，再處理參照這些記錄的 cities 資料。

    處理流程：
        1. 取得所有已註冊的 Handler 國家列表
        2. 計算全域最大 geoname_id（用於後續 ID 分配）
        3. 先更新 admin1CodesASCII.txt（建立一級行政區記錄）
        4. 再更新 cities500.txt（參照已建立的 admin1 記錄）
        5. 每個步驟都會更新並傳遞最大 ID，確保 ID 不衝突

    Args:
        cities_file: 原始的 cities500.txt 檔案路徑。
        extra_files: 額外的城市資料檔案路徑列表。
        output_file: 更新後的 cities500.txt 檔案儲存路徑。
        min_population: 加入額外資料的最低人口數要求（預設 100）。

    Returns:
        None

    Note:
        處理順序（admin1 → cities500）至關重要，確保 cities 資料的 admin1_code
        能正確參照到已存在的 admin1 記錄，避免孤兒參照問題。
    """
    # 取得已註冊的 Handler 國家列表
    handler_countries = get_all_handlers()
    logger.info(f"已註冊的 Handler 國家: {', '.join(handler_countries)}")

    # 初始化全域最大 ID（只計算一次）
    current_max_id = calculate_global_max_geoname_id()
    logger.info(f"初始全域最大 geoname_id: {current_max_id}")

    # 先更新 admin1CodesASCII.txt（建立 admin1 記錄）
    admin1_input = os.path.join("geoname_data", "admin1CodesASCII.txt")
    admin1_output = os.path.join("output", "admin1CodesASCII_optimized.txt")

    current_max_id = update_admin1_data(
        admin1_input=admin1_input,
        admin1_output=admin1_output,
        handler_countries=handler_countries,
        current_max_id=current_max_id,
    )

    # 再更新 cities500.txt（參照已建立的 admin1 記錄）
    current_max_id = update_cities500_data(
        cities_file=cities_file,
        extra_files=extra_files,
        output_file=output_file,
        min_population=min_population,
        handler_countries=handler_countries,
        current_max_id=current_max_id,
    )


def update_admin1_data(
    admin1_input: str,
    admin1_output: str,
    handler_countries: list[str],
    current_max_id: int,
) -> int:
    """更新 admin1CodesASCII.txt 檔案，使用 Handler 提供的自訂 admin1 資料。

    此函數負責 admin1CodesASCII.txt 的處理流程，為有 Handler 的國家生成並替換
    一級行政區記錄。這些記錄會被 cities500.txt 中的 admin1_code 欄位參照。

    處理流程：
        1. 讀取 admin1CodesASCII.txt
        2. 為每個有 Handler 的國家調用 generate_admin1_records() 生成新記錄
        3. 為每個國家分配獨立的 geoname_id 範圍（從 current_max_id 開始遞增）
        4. 移除該國舊的 admin1 資料（依據 id 欄位的國家代碼前綴）
        5. 將新資料插入到 DataFrame 前端
        6. 將更新後的資料寫入輸出檔案

    Args:
        admin1_input: 原始的 admin1CodesASCII.txt 檔案路徑。
        admin1_output: 更新後的 admin1CodesASCII.txt 檔案儲存路徑。
        handler_countries: 已註冊的 Handler 國家代碼列表（如 ['TW', 'JP']）。
        current_max_id: 當前全域最大 geoname_id（用於分配新 ID，避免衝突）。

    Returns:
        處理後的最大 geoname_id（供後續處理使用）。

    Note:
        此函數應在 update_cities500_data() 之前執行，確保 cities 資料能正確參照
        到已存在的 admin1 記錄，維持資料完整性。
    """
    logger.info("開始處理 admin1CodesASCII.txt")

    # 讀取原始 admin1 資料
    admin1_df = pl.read_csv(
        admin1_input, separator="\t", has_header=False, schema=ADMIN1_SCHEMA
    )

    max_id = current_max_id

    # 為每個有 Handler 的國家處理 admin1
    for country_code in handler_countries:
        try:
            handler_class = get_handler(country_code)
            handler = handler_class()

            # 計算此國家的起始 ID
            base_id = max_id + 1
            logger.info(f"為 {country_code} admin1 計算的 base_geoname_id: {base_id}")

            # 產生 admin1 記錄
            csv_path = f"meta_data/{country_code.lower()}_geodata.csv"
            new_admin1 = handler.generate_admin1_records(csv_path, base_id)

            # 計算使用的最大 ID
            max_id_used = new_admin1.select(
                pl.col("geoname_id").cast(pl.Int64).max()
            ).item()

            # 更新最大 ID
            max_id = max_id_used
            logger.info(
                f"{country_code} admin1 使用的 ID 範圍: {base_id} - {max_id_used}"
            )

            # 移除舊的該國資料
            admin1_df = admin1_df.filter(
                ~pl.col("id").str.starts_with(f"{country_code}.")
            )
            # 插入新資料（放在最前面）
            admin1_df = new_admin1.vstack(admin1_df)
            logger.info(f"已更新 {country_code} 的 admin1 資料")

        except ValueError as e:
            logger.warning(f"無法取得 {country_code} Handler: {e}")
        except Exception as e:
            logger.error(f"處理 {country_code} admin1 時發生錯誤: {e}")

    # 儲存 admin1CodesASCII_optimized.txt
    # 確保輸出資料夾存在
    Path(admin1_output).parent.mkdir(parents=True, exist_ok=True)
    admin1_df.write_csv(admin1_output, separator="\t", include_header=False)
    logger.info(f"admin1CodesASCII.txt 更新完成，儲存至 {admin1_output}")

    return max_id


def update_cities500_data(
    cities_file: str,
    extra_files: list[str],
    output_file: str,
    min_population: int,
    handler_countries: list[str],
    current_max_id: int,
) -> int:
    """更新 cities500.txt 檔案，合併額外資料並使用 Handler 替換特定國家資料。

    此函數負責 cities500.txt 的完整處理流程，包括讀取、合併、去重與國家特化處理。
    處理的 cities 資料中的 admin1_code 欄位會參照到 admin1CodesASCII.txt 中的記錄。

    處理流程：
        1. 讀取 cities500.txt
        2. 合併額外資料並去重（調用 merge_extra_data）
        3. 使用 Handler 替換特定國家資料（調用 replace_with_handler_data）
        4. 將更新後的資料寫入輸出檔案

    Args:
        cities_file: 原始的 cities500.txt 檔案路徑。
        extra_files: 額外的城市資料檔案路徑列表（如 extra_data/TW.txt）。
        output_file: 更新後的 cities500.txt 檔案儲存路徑。
        min_population: 加入額外資料的最低人口數要求。
        handler_countries: 已註冊的 Handler 國家代碼列表（如 ['TW', 'JP']）。
        current_max_id: 當前全域最大 geoname_id（用於分配新 ID，避免衝突）。

    Returns:
        處理後的最大 geoname_id（供後續處理使用）。

    Note:
        此函數應在 update_admin1_data() 之後執行，確保 cities 資料的 admin1_code
        能正確參照到已建立的 admin1 記錄。
    """
    logger.info("開始更新 cities500.txt")

    # 讀取 cities500.txt
    cities500_df = pl.read_csv(
        cities_file,
        separator="\t",
        has_header=False,
        schema=CITIES_SCHEMA,
    )

    # 合併額外資料並去重
    cities500_df = merge_extra_data(cities500_df, extra_files, min_population)

    # 使用 Handler 替換特定國家資料
    cities500_df, max_id = replace_with_handler_data(
        cities500_df, handler_countries, current_max_id
    )

    # 儲存結果
    # 確保輸出資料夾存在
    Path(output_file).parent.mkdir(parents=True, exist_ok=True)
    cities500_df.write_csv(output_file, separator="\t", include_header=False)
    logger.info(
        f"cities500.txt 更新完成 ({cities500_df.height} 筆資料)，儲存至 {output_file}"
    )

    return max_id


def merge_extra_data(
    cities500_df: pl.DataFrame,
    extra_files: list[str],
    min_population: int,
) -> pl.DataFrame:
    """合併額外資料到 cities500 並進行去重處理。

    處理流程：
        1. 讀取額外的城市資料檔案
        2. 篩選額外資料（geoname_id 不重複且人口數 >= min_population）
        3. 合併新資料到 cities500.txt
        4. 去重處理：對於相同座標，保留人口數最大者（人口相同則保留 geoname_id 最小者）

    Args:
        cities500_df: 原始的 cities500 DataFrame。
        extra_files: 額外的城市資料檔案路徑列表（如 extra_data/TW.txt）。
        min_population: 加入額外資料的最低人口數要求。

    Returns:
        合併並去重後的 DataFrame。
    """
    # 讀取 extra_data/country_code.txt
    extra_df = pl.DataFrame(schema=CITIES_SCHEMA)
    for file in extra_files:
        if Path(file).exists():
            extra_df = extra_df.vstack(
                pl.read_csv(
                    file,
                    separator="\t",
                    has_header=False,
                    schema=CITIES_SCHEMA,
                )
            )
            logger.info(f"讀取額外資料檔案: {file}")
        else:
            logger.warning(f"額外資料檔案不存在，跳過: {file}")

    # 篩選條件：
    #   - `geoname_id` 不在 `cities500_df` 的 `geoname_id` 中
    #   - `population` 大於等於 `min_population`
    filtered_extra_df = extra_df.filter(
        ~pl.col("geoname_id").is_in(cities500_df["geoname_id"])  # geoname_id 不能已存在
        & (pl.col("population") >= min_population)  # 人口數須 >= MIN_POPULATION
    )

    # 合併新資料到 `cities500_df`
    cities500_df = cities500_df.vstack(filtered_extra_df)
    logger.info(f"成功新增 {filtered_extra_df.height} 行數據到 cities500.txt")

    # 檢查是否有重複座標的資料
    logger.info("開始處理重複座標的資料")
    initial_rows = cities500_df.height
    duplicates_check = cities500_df.group_by(["latitude", "longitude"]).agg(
        pl.max("population").alias("population_max"),
        pl.min("geoname_id").alias("geoname_id_min"),
    )

    cities500_df = (
        cities500_df.join(
            duplicates_check,
            on=["latitude", "longitude"],
            how="inner",
        )
        .filter(
            (pl.col("population") == pl.col("population_max"))
            & (pl.col("geoname_id") == pl.col("geoname_id_min"))
        )
        .select(cities500_df.columns)
    )
    final_rows = cities500_df.height
    if initial_rows > final_rows:
        logger.info(f"處理重複座標完成，移除了 {initial_rows - final_rows} 筆資料")
    else:
        logger.info("未發現需要處理的重複座標資料")

    return cities500_df


def replace_with_handler_data(
    cities500_df: pl.DataFrame,
    handler_countries: list[str],
    current_max_id: int,
) -> tuple[pl.DataFrame, int]:
    """使用 Handler 替換特定國家的資料。

    處理流程：
        1. 為每個有 Handler 的國家調用 replace_in_dataset()
        2. 動態分配 geoname_id 範圍，確保不衝突
        3. 追蹤並回傳使用的最大 ID

    Args:
        cities500_df: 待處理的 cities500 DataFrame。
        handler_countries: 已註冊的 Handler 國家代碼列表（如 ['TW', 'JP']）。
        current_max_id: 當前全域最大 geoname_id（用於分配新 ID，避免衝突）。

    Returns:
        處理後的 DataFrame 與最大 geoname_id 的 tuple。
    """
    max_id = current_max_id
    for country_code in handler_countries:
        try:
            handler_class = get_handler(country_code)
            handler = handler_class()

            # 計算此國家的起始 ID
            base_id = max_id + 1

            # 替換資料並取得使用的最大 ID
            cities500_df, max_id_used = handler.replace_in_dataset(
                cities500_df, base_geoname_id=base_id
            )

            # 更新最大 ID
            max_id = max_id_used
            logger.info(
                f"已使用 {country_code} Handler 替換 cities500 資料 "
                f"(ID 範圍: {base_id} - {max_id_used})"
            )
        except ValueError as e:
            logger.warning(f"無法取得 {country_code} Handler: {e}")

    return cities500_df, max_id


def test():
    min_population = 100

    # 文件路径
    data_base_folder = "./geoname_data"
    extra_data_folder = os.path.join(data_base_folder, "extra_data")
    output_folder = "./output"
    country_code = ["TW", "JP"]

    cities_file = os.path.join(data_base_folder, "cities500.txt")
    extra_files = [
        os.path.join(extra_data_folder, f"{code}.txt") for code in country_code
    ]
    output_file = os.path.join(output_folder, "cities500_optimized.txt")

    update_geodata(cities_file, extra_files, output_file, min_population)


if __name__ == "__main__":
    logger.error("請使用 main.py 作為主要接口，而非直接執行 enhance_data.py")
