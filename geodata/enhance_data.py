import os
import pandas as pd
from utils import logger, CITIES_HEADER


def update_admin1():
    """
    1. 將admin2取代為admin1
    """


def update_cities500(cities_file, extra_file, output_file):
    logger.info("開始更新 cites500.txt")
    
    # 讀取 cites500.txt
    cites500_df = pd.read_csv(
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
        id_value = row["ID"]

        # 獲取人口數
        try:
            num_value = float(row["Population"])
        except ValueError:
            continue  # 如果無法轉換為數字，跳過此行

        # 判斷是否滿足追加條件
        if id_value not in cites500_df["ID"].values and num_value >= MIN_POPULATION:
            cites500_df = cites500_df._append(row, ignore_index=True)
            count += 1

    logger.info(f"共 {count} 筆資料被追加，目前總共 {len(cites500_df)} 筆資料")

    # 將country是TW，且"Admin2 Code"為空的資料刪除
    cites500_df = cites500_df[(cites500_df["Country Code"] != "TW") | (cites500_df["Admin2 Code"].notnull())]
    
    cites500_df.to_csv(output_file, sep="\t", header=False, index=False)
    
    logger.info(f"移除台灣資料中 Admin2 Code 為空的資料，剩餘 {len(cites500_df)} 筆資料")
    logger.info("cites500.txt 更新完成")


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
