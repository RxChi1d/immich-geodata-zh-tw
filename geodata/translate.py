import csv
import regex
from utils import (
    load_meta_data,
    ensure_folder_exists,
    logger,
    load_alternate_names,
    CITIES_HEADER,
)
import os
import sys
import pandas as pd
from glob import glob
from tqdm import tqdm

import opencc
from zhconv import convert

source_folder = "./geoname_data"
output_folder = "./output"


# 初始化簡繁轉換器
converter_t2s = opencc.OpenCC("t2s")
converter_s2t = opencc.OpenCC("s2t")


def is_chinese(text):
    return bool(regex.match(r"^\p{Script_Extensions=Han}+$", text))


def is_simplified_chinese(text):
    return is_chinese(text) and text == converter_t2s.convert(text)


def is_traditional_chinese(text):
    return is_chinese(text) and text == converter_s2t.convert(text)


def load_metadata_list(metadata_folder):
    """
    讀取所有詮釋資料檔案並轉換為字典格式。

    從指定的資料夾讀取所有 CSV 格式的詮釋資料檔案，並將每個檔案的內容載入到一個字典中。
    檔案名稱(不含副檔名)將作為字典的鍵值。

    參數:
        metadata_folder (str): 包含詮釋資料 CSV 檔案的資料夾路徑

    回傳:
        dict: 包含所有詮釋資料的字典，其中:
            - 鍵(key): 檔案名稱(不含副檔名)
            - 值(value): 對應檔案的詮釋資料內容

    範例:
        metadata_dict = load_metadata_list("./metadata/")
    """

    metadata_dict = {}

    for file_path in glob(f"{metadata_folder}/*.csv"):
        key = os.path.splitext(os.path.basename(file_path))[0]
        metadata_dict[key] = load_meta_data(file_path)

    return metadata_dict


def process_multiple_names(row, res):
    """
    處理多個地名的函數。

    如果翻譯結果中包含斜線分隔的多個地名，此函數會進行以下處理：
    1. 將多個地名分割成列表
    2. 如果所有地名都相同，則只保留其中一個
    3. 更新行數據的name和asciiname欄位

    參數:
        row (dict): 包含地理數據的字典
        res (str): 包含一個或多個地名的字符串，可能以斜線分隔

    返回:
        None: 直接修改傳入的row字典
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
    """將城市資料檔案轉換為繁體中文格式。

    這個函數會讀取城市資料檔案，並將城市名稱轉換為繁體中文。轉換過程會優先使用元數據文件中的對應，
    其次使用備用名稱文件，最後嘗試從現有的替代名稱中尋找中文名稱。

    參數:
        metadata_folder (str): 包含元數據文件的資料夾路徑
        cities500_file (str): 原始城市資料檔案的路徑
        output_file (str): 輸出檔案的路徑
        alternate_name_file (str): 備用名稱檔案的路徑

    返回:
        None

    處理順序:
        1. 檢查是否可以從元數據中找到對應的翻譯
        2. 檢查是否可以從備用名稱檔案中找到對應的翻譯
        3. 檢查原始資料中的替代名稱是否包含中文名稱
        4. 如果都找不到對應的翻譯，則記錄警告信息

    注意:
        - 輸入檔案必須存在，否則程序將終止執行
        - 輸出檔案的資料夾路徑必須存在或可建立
        - 所有中文轉換都會轉為繁體中文格式
    """

    meta_data = load_metadata_list(metadata_folder)
    alternate_name = load_alternate_names(alternate_name_file)

    if not os.path.exists(cities500_file):
        logger.error(f"輸入檔案 {cities500_file} 不存在")
        sys.exit(1)

    ensure_folder_exists(output_file)

    cities500_df = pd.read_csv(
        cities500_file,
        sep="\t",
        header=None,
        names=CITIES_HEADER,
        low_memory=False,
        dtype=str,
    )

    pbar = tqdm(cities500_df.iterrows(), total=len(cities500_df), desc="Processing")
    for index, row in pbar:
        if (
            row["country_code"] in meta_data
            and (row["longitude"], row["latitude"]) in meta_data[row["country_code"]]
        ):
            location = meta_data[row["country_code"]][
                (row["longitude"], row["latitude"])
            ]
            res = location["admin_2"]
            res = convert(res, "zh-tw")

            process_multiple_names(row, res)

        elif row["geonameid"] in alternate_name:
            name = alternate_name[row["geonameid"]]
            name = convert(name, "zh-tw")

            row["name"] = name
            row["asciiname"] = name

        elif type(row["alternatenames"]) == str:
            try:
                candidates = row["alternatenames"].split(",")
            except AttributeError:
                logger.error(f"alternatenames: {row['alternatenames']} 不是字符串")
                sys.exit(1)

            chinese_words = [word for word in candidates if is_chinese(word)]
            simplified_word = next(
                (word for word in chinese_words if is_simplified_chinese(word)),
                None,
            )
            traditional_word = next(
                (word for word in chinese_words if is_traditional_chinese(word)),
                None,
            )

            if traditional_word:
                row["name"] = traditional_word
                row["asciiname"] = row["name"]

            elif simplified_word:
                row["name"] = convert(simplified_word, "zh-tw")
                row["asciiname"] = row["name"]

        else:
            logger.warning(f"未處理的行: {row['geonameid']} {row['name']}")

    cities500_df.to_csv(output_file, sep="\t", index=False, header=False)


def translate_admin1(input_file, alternate_name_file):
    """
    將行政區域名稱從簡體中文轉換為繁體中文。

    此函數讀取包含行政區域資訊的文件，並將其中的地名從簡體中文轉換為繁體中文。
    處理過程包括映射替代名稱並進行字符轉換。

    參數:
        input_file (str): 輸入文件的路徑，預期為以tab分隔的文本文件
        alternate_name_file (str): 包含替代名稱映射的文件路徑

    返回:
        None

    處理流程:
        1. 讀取輸入文件
        2. 從第4列獲取區域代碼
        3. 使用 alternate_name 進行映射
        4. 將映射後的名稱轉換為繁體中文
        5. 同時更新第2列和第3列的值
        6. 將處理後的數據保存到新文件

    錯誤處理:
        - 如果輸入文件不存在，程序將終止並記錄錯誤

    注意:
        輸出文件將保存在預設的輸出資料夾中，檔名會加上"_processed"後綴
    """

    logger.info(f"正在處理 {input_file}")

    if not os.path.exists(input_file):
        logger.error(f"輸入檔案 {input_file} 不存在")
        sys.exit(1)

    alternate_name = load_alternate_names(alternate_name_file)

    basename = os.path.splitext(os.path.basename(input_file))
    new_filename = basename[0] + "_processed" + basename[1]
    output_file = os.path.join(output_folder, new_filename)
    ensure_folder_exists(output_file)

    # 讀取文件，header=None 表示沒有標題列
    df = pd.read_csv(input_file, sep="\t", header=None, dtype=str)

    # 轉換流程
    # 1. 從第4列（索引3）獲取代碼
    # 2. 映射 alternate_name
    # 3. 轉換為繁體中文
    # 4. 同時更新第2列（索引1）和第3列（索引2）

    # 創建映射 Series
    mapped_values = df[3].map(alternate_name)

    # 應用繁體轉換，僅處理有效值
    converted = mapped_values.dropna().apply(lambda x: convert(x, "zh-tw"))

    # 創建有效值掩碼（非空且轉換後不為空）
    valid_mask = (mapped_values.notna()) & (converted.reindex(df.index).notna())

    # 更新對應列
    df.loc[valid_mask, 1] = converted.reindex(df.index)[valid_mask]
    df.loc[valid_mask, 2] = converted.reindex(df.index)[valid_mask]

    # 寫回文件
    df.to_csv(output_file, sep="\t", index=False, header=False)

    logger.info(f"已處理的檔案已儲存至 {output_file}")


def run():
    output_fodler = "./output"
    metadata_folder = os.path.join(output_fodler, "meta_data")

    # 翻譯 cities500
    cities500_file = os.path.join(output_fodler, "cities500_en.txt")
    output_file = os.path.join(output_fodler, "cities500_processed.txt")
    alternate_name_file = os.path.join(output_fodler, "alternate_chinese_name.json")

    translate_cities500(
        metadata_folder, cities500_file, output_file, alternate_name_file
    )

    # 翻譯 admin1 和 admin2
    admin1_file = os.path.join(source_folder, "admin1CodesASCII.txt")
    admin2_file = os.path.join(source_folder, "admin2Codes.txt")

    translate_admin1(admin1_file, alternate_name_file)
    translate_admin1(admin2_file, alternate_name_file)


if __name__ == "__main__":
    run()
