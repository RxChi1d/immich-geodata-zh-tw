import os
import pandas as pd
from utils import logger, CITIES_HEADER


def update_taiwan_admin1(cities500_df):
    """
    以下操作針對 country code 是 TW 的資料進行：
    1. 讀取 new_admin1_map.csv
    2. 將 admin1_code 以 new_id 取代
    3. admin2_code 以 admin3_code 取代
    4. admin3_code 以 admin4_code 取代
    5. admin4_code 清空
    """
    logger.info("開始調整台灣的 Admin Code")

    # 讀取 new_admin1_map.csv
    admin1_map = pd.read_csv(
        os.path.join("output", "new_admin1_map.csv"),
        sep="\t",
        low_memory=False
    )

    # 只更新 country code 是 TW 的資料
    taiwan_df = cities500_df[cities500_df["country_code"] == "TW"]

    for index, row in taiwan_df.iterrows():
        old_id = row["admin2_code"] # TPE
        
        # admin1_map["old_id"]: TW.04.TPE (不一定是04)
        # 根據 old_id 和 admin1_map["old_id"] 找到對應的 new_id
        new_id = admin1_map[admin1_map["old_id"].str.contains(old_id)]["new_id"].values[0]
        id_num = new_id.split(".")[-1]
        
        # 更新 Admin Code
        cities500_df.at[index, "admin1_code"] = id_num
        cities500_df.at[index, "admin2_code"] = row["admin3_code"]
        cities500_df.at[index, "admin3_code"] = row["admin4_code"]
        cities500_df.at[index, "admin4_code"] = ""

    cities500_df.to_csv(
        os.path.join("output", "cities500_en.txt"),
        sep="\t",
        header=False,
        index=False
    )

    logger.info("台灣的 Admin Code 調整完成")
    
    return cities500_df


def update_cities500(cities_file, extra_file, output_file):
    logger.info("開始更新 cites500.txt")
    
    # 讀取 cites500.txt
    cities500_df = pd.read_csv(
        cities_file,
        sep="\t",
        header=None,
        names=CITIES_HEADER,
        low_memory=False
    )

    # 讀取 extra_data/TW.txt
    extra_df = pd.read_csv(
        extra_file,
        sep="\t",
        header=None,
        names=CITIES_HEADER,
        low_memory=False
    )

    count = 0
    for index, row in extra_df.iterrows():
        id_value = row["geonameid"]

        # 獲取人口數
        try:
            num_value = float(row["population"])
        except ValueError:
            continue  # 如果無法轉換為數字，跳過此行

        # 判斷是否滿足追加條件
        if id_value not in cities500_df["geonameid"].values and num_value >= MIN_POPULATION:
            cities500_df = cities500_df._append(row, ignore_index=True)
            count += 1

    logger.info(f"共 {count} 筆資料被追加，目前總共 {len(cities500_df)} 筆資料")

    # 將country是TW，且"admin2_code"為空的資料刪除
    cities500_df = cities500_df[(cities500_df["country_code"] != "TW") | (cities500_df["admin2_code"].notnull())]
    
    logger.info(f"移除台灣資料中 admin2_code 為空的資料，剩餘 {len(cities500_df)} 筆資料")
    
    # 調整台灣的 Admin Code
    cities500_df = update_taiwan_admin1(cities500_df)
    
    cities500_df.to_csv(output_file, sep="\t", header=False, index=False)
    
    logger.info(f"cites500.txt 更新完成，儲存至 {output_file}")


if __name__ == "__main__":
    MIN_POPULATION = 100

    # 文件路径
    data_base_folder = "./geoname_data"
    extra_data_folder = os.path.join(data_base_folder, "extra_data")
    output_folder = "./output"

    cities_file = os.path.join(data_base_folder, "cities500.txt")
    extra_file = os.path.join(extra_data_folder, "TW.txt")
    output_file = os.path.join(output_folder, "cities500_en.txt")

    update_cities500(cities_file, extra_file, output_file)
