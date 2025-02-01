import os
import sys
from glob import glob

import regex
import opencc
import polars as pl

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.define import CITIES_SCHEMA, GEODATA_SCHEMA, ADMIN1_SCHEMA
from core.utils import ensure_folder_exists, logger, load_alternate_names

# 初始化簡繁轉換器
converter_t2s = opencc.OpenCC("t2s")
converter_s2t = opencc.OpenCC("s2t")


def find_duplicate_in_meta(meta_data):
    duplicated_entries = []

    for country, df in meta_data.items():
        # 計算 (longitude, latitude) 的出現次數
        duplicate_locs = (
            df.group_by(["longitude", "latitude"])
            .agg(pl.count().alias("count"))
            .filter(pl.col("count") > 1)  # 只保留重複的組合
            .select(["longitude", "latitude"])
        )
        
        # 如果有重複的 (longitude, latitude)，將完整資訊加入結果
        if not duplicate_locs.is_empty():
            duplicate_rows = df.join(duplicate_locs, on=["longitude", "latitude"], how="inner")
            duplicate_rows = duplicate_rows.with_columns(pl.lit(country).alias("country_code"))
            duplicated_entries.append(duplicate_rows)

    # 合併結果並顯示
    if duplicated_entries:
        duplicated_df = pl.concat(duplicated_entries)
        print(duplicated_df)
    else:
        print("No non-unique longitude/latitude found.")


def is_chinese(text):
    """
    判斷給定的文字是否為中文。

    Args:
        text (str): 要檢查的文字。

    Returns:
        bool: 如果文字是中文，返回 True，否則返回 False。
    """

    return bool(regex.match(r"^[\p{Script_Extensions=Han}-]+$", text))


def is_simplified_chinese(text):
    """
    判斷給定的文字是否為簡體中文。

    Args:
        text (str): 要檢查的文字。

    Returns:
        bool: 如果文字是簡體中文則返回True，否則返回False。
    """

    return is_chinese(text) and text == converter_t2s.convert(text)


def is_traditional_chinese(text):
    """
    判斷給定的文字是否為繁體中文。

    Args:
        text (str): 要檢查的文字。

    Returns:
        bool: 如果文字是繁體中文，返回 True，否則返回 False。
    """

    return is_chinese(text) and text == converter_s2t.convert(text)


def load_metadata_list(metadata_folder):
    """
    載入指定資料夾中的所有 CSV 檔案，並將其轉換為字典格式。

    Args:
        metadata_folder (str): 包含 CSV 檔案的資料夾路徑。

    Returns:
        dict: 一個字典，其中鍵為 CSV 檔案名稱（不含副檔名），值為對應的資料表。
    """

    metadata_dict = {}

    for file_path in glob(f"{metadata_folder}/*.csv"):
        key = os.path.splitext(os.path.basename(file_path))[0]
        metadata_dict[key] = pl.read_csv(
            file_path,
            schema=GEODATA_SCHEMA,
        )

    return metadata_dict


def process_multiple_names(row, res):
    """
    處理多個名稱的函式。如果 `res` 包含斜線 ("/")，則將其分割並去除空白，
    如果分割後的名稱集合只有一個唯一值，則將 `res` 設為該唯一值。
    最後，如果 `res` 有值，則更新 `row` 字典中的 "name" 和 "asciiname" 欄位。

    Args:
        row (dict): 包含名稱資訊的字典。
        res (str): 需要處理的名稱字串。

    Returns:
        None
    """

    if "/" in res:
        t = res.split("/")
        t = [i.strip() for i in t]
        if len(set(t)) == 1:
            res = t[0]
    if res:
        row["name"] = res
        row["asciiname"] = res


def translate_cities500(
    metadata_folder, cities500_file, output_file, alternate_name_file
):
    """
    翻譯 cities500 檔案中的地名，並將結果儲存至指定的輸出檔案。

    Args:
        metadata_folder (str): 包含地名元數據的資料夾路徑。
        cities500_file (str): cities500 檔案的路徑。
        output_file (str): 翻譯後的結果儲存路徑。
        alternate_name_file (str): 包含地名替代名稱的檔案路徑。

    Returns:
        None

    Raises:
        SystemExit: 如果輸入檔案不存在，則終止程式並顯示錯誤訊息。
    """

    logger.info(f"開始翻譯 {cities500_file}")

    meta_data = load_metadata_list(metadata_folder)
    alternate_name = load_alternate_names(alternate_name_file)

    if not os.path.exists(cities500_file):
        logger.critical(f"輸入檔案 {cities500_file} 不存在")
        sys.exit(1)

    ensure_folder_exists(output_file)

    cities500_df = pl.read_csv(
        cities500_file,
        separator="\t",
        has_header=False,
        schema=CITIES_SCHEMA,
    )

    # 1.  先處理 meta_data 匹配
    def translate_from_metadata(row):
        country = row["country_code"]

        # 確保 country 存在於 meta_data
        if country not in meta_data:
            return None

        # 在 meta_data[country] DataFrame 中查找對應的 (longitude, latitude)
        result = meta_data[country].filter(
            (pl.col("longitude") == row["longitude"])
            & (pl.col("latitude") == row["latitude"])
        )

        # 如果有匹配的行，取出 admin_2
        if not result.is_empty():
            return result["admin_2"].item()

        return None  # 若無匹配則回傳 None


    # TODO: 1. JP不應該讀到, 2. JP中的重複值沒有處理
    find_duplicate_in_meta(meta_data)
    cities500_df = cities500_df.with_columns(
        pl.struct(["country_code", "longitude", "latitude"])
        .map_elements(translate_from_metadata, return_dtype=pl.String)
        .alias("translated_name")
    )

    # 2. 透過 alternate_name 進行翻譯
    cities500_df = (
        cities500_df.join(alternate_name, on="geoname_id", how="left")
        .with_columns(pl.col("name_right").alias("alternate_translated_name"))
        .drop("name_right")
    )

    # 3. 如果 `alternatenames` 存在，則檢查是否有簡體或繁體的名稱
    def extract_chinese_names(alt_names):
        if not alt_names:
            return None
        candidates = alt_names.split(",")
        chinese_words = [word for word in candidates if is_chinese(word)]
        traditional = next(
            (word for word in chinese_words if is_traditional_chinese(word)), None
        )
        simplified = next(
            (word for word in chinese_words if is_simplified_chinese(word)), None
        )
        return (
            traditional
            if traditional
            else (converter_s2t.convert(simplified) if simplified else None)
        )

    cities500_df = cities500_df.with_columns(
        pl.col("alternatenames")
        .map_elements(extract_chinese_names, return_dtype=pl.String)
        .alias("alternatenames_translated")
    )

    # 將 "" 轉換為 None，以便 coalesce 時能夠正確處理Ｆ
    cities500_df = cities500_df.with_columns(
        [
            pl.when(pl.col(col).cast(pl.String) == "")
            .then(None)
            .otherwise(pl.col(col))
            .alias(col)
            for col in cities500_df.schema
            if cities500_df.schema[col] == pl.String
        ]
    )

    # 4. 選擇最終翻譯名稱（優先順序: metadata > alternate > alternatenames）
    cities500_df = cities500_df.with_columns(
        pl.coalesce(
            [
                "translated_name",
                "alternate_translated_name",
                "alternatenames_translated",
            ]
        ).alias("final_name")
    ).drop(
        ["translated_name", "alternate_translated_name", "alternatenames_translated"]
    )

    # 5. 記錄未處理的行
    unprocessed = cities500_df.filter(pl.col("final_name").is_null())
    if not unprocessed.is_empty():
        # for row in unprocessed.iter_rows(named=True):
        #     logger.warning(f"未處理的行: {row['geoname_id']} {row['name']}")
        logger.warning(f"未翻譯的地名數量: {unprocessed.height}")

    # 6. 更新 name 和 asciiname
    cities500_df = cities500_df.with_columns(
        pl.coalesce(["final_name", "name"]).alias("name"),
        pl.coalesce(["final_name", "name"]).alias("asciiname"),
    ).drop("final_name")

    # 7. 紀錄空地名的行
    empty_names = cities500_df.filter(
        (pl.col("name").is_null()) | (pl.col("name") == "")
    )
    if not empty_names.is_empty():
        logger.error(f"空地名數量: {empty_names.height}")

    # 8. 儲存
    cities500_df.write_csv(output_file, separator="\t", include_header=False)

    logger.info(f"已翻譯 cities500，結果已儲存至 {output_file}")


def translate_admin1(input_file, alternate_name_file, output_folder):
    """
    將輸入的行政區域名稱文件進行翻譯並儲存至指定的輸出資料夾。

    Args:
        input_file (str): 輸入的行政區域名稱文件路徑。
        alternate_name_file (str): 替代名稱文件路徑，用於映射行政區域名稱。
        output_folder (str): 輸出資料夾路徑，翻譯後的文件將儲存在此資料夾中。

    Returns:
        None

    Raises:
        SystemExit: 當輸入檔案不存在時，會終止程式並顯示錯誤訊息。
    """

    logger.info(f"正在翻譯 {input_file}")

    if not os.path.exists(input_file):
        logger.critical(f"輸入檔案 {input_file} 不存在")
        sys.exit(1)

    alternate_name = load_alternate_names(alternate_name_file)

    new_filename = os.path.basename(input_file)
    new_filename = (
        new_filename.replace("_optimized", "_translated")
        if "_optimized" in new_filename
        else new_filename.replace(".txt", "_translated.txt")
    )
    output_file = os.path.join(output_folder, new_filename)
    ensure_folder_exists(output_file)

    # 讀取文件，header=None 表示沒有標題列
    df = pl.read_csv(
        input_file,
        separator="\t",
        has_header=False,
        schema=ADMIN1_SCHEMA,
    )

    # 轉換流程
    # 1. 從第4列（索引3）獲取代碼
    # 2. 映射 alternate_name
    # 3. 轉換為繁體中文
    # 4. 同時更新第2列（索引1）和第3列（索引2）

    # 創建映射 Series
    df = df.join(alternate_name, on="geoname_id", how="left")

    # 應用繁體轉換，僅處理有效值
    def convert_admin_name(name_right, name):
        if name_right is None or name_right == "":
            return name
        elif is_simplified_chinese(name_right):
            return converter_s2t.convert(name_right)
        else:
            return name_right

    df = df.with_columns(
        pl.struct(["name_right", "name"])
        .map_elements(
            lambda row: convert_admin_name(row["name_right"], row["name"]),
            return_dtype=pl.String,
        )
        .alias("name")
    ).drop("name_right")

    df = df.with_columns(pl.col("name").alias("asciiname"))

    # 寫回文件
    df.write_csv(output_file, separator="\t", include_header=False)

    logger.info(f"翻譯文件已儲存至 {output_file}")


def test():
    source_folder = "./geoname_data"
    output_folder = "./output"
    metadata_folder = os.path.join(output_folder, "meta_data")

    # 翻譯 cities500
    cities500_file = os.path.join(output_folder, "cities500_optimized.txt")
    output_file = os.path.join(output_folder, "cities500_translated.txt")
    alternate_name_file = os.path.join(output_folder, "alternate_chinese_name.json")

    ensure_folder_exists(output_file)

    translate_cities500(
        metadata_folder, cities500_file, output_file, alternate_name_file
    )

    # 翻譯 admin1 和 admin2
    admin1_file = os.path.join(output_folder, "admin1CodesASCII_optimized.txt")
    admin2_file = os.path.join(source_folder, "admin2Codes.txt")

    translate_admin1(admin1_file, alternate_name_file, output_folder)
    translate_admin1(admin2_file, alternate_name_file, output_folder)


if __name__ == "__main__":
    logger.error("請使用 main.py 作為主要接口，而非直接執行 translate.py")
