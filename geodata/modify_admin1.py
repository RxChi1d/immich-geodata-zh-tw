import os
import pandas as pd

from utils import load_alternate_names

TAIWAN_ADMIN1 = {
    # 直轄市 (Special Municipalities)
    "臺北市": "TW.01",
    "新北市": "TW.02",
    "桃園市": "TW.03",
    "臺中市": "TW.04",
    "臺南市": "TW.05",
    "高雄市": "TW.06",
    # 省轄市 (Provincial Cities)
    "基隆市": "TW.07",
    "新竹市": "TW.08",
    "嘉義市": "TW.09",
    # 縣 (Counties)
    "宜蘭縣": "TW.10",
    "新竹縣": "TW.11",
    "苗栗縣": "TW.12",
    "彰化縣": "TW.13",
    "南投縣": "TW.14",
    "雲林縣": "TW.15",
    "嘉義縣": "TW.16",
    "屏東縣": "TW.17",
    "臺東縣": "TW.18",
    "花蓮縣": "TW.19",
    "澎湖縣": "TW.20",
    "金門縣": "TW.21",
    "連江縣": "TW.22",
}


def get_taiwan_admin1(admin1_path, admin2_path, output_path):
    """
    1. 讀取 admin1CodesASCII.txt 和 admin2Codes.txt
    2. 讀取 alternate_chinese_name.json (中文地名對照表)
    3. 從 admin2Codes.txt 中獲取臺灣的行政區的 geoname_id 和編號 (e.g. TW.03.TPQ)
    4. 根據 geoname_id 在 alternate_chinese_name.json 中找到對應的中文名稱
    5. 根據找到的中文名稱，在 TAIWAN_ADMIN1 找到新的 ID
    6. 將新的台灣 admin1 對應表存檔到 output_path (header = ["name", "new_id", "old_id", "geoname_id"])
    """
    output_folder = os.path.dirname(output_path)

    amdin1_data = pd.read_csv(
        admin1_path,
        sep="\t",
        header=None,
        names=["ID", "Name", "Name_ASCII", "Geoname_ID"],
    )

    admin2_data = pd.read_csv(
        admin2_path,
        sep="\t",
        header=None,
        names=["ID", "Name", "Name_ASCII", "Geoname_ID"],
    )

    alternate_names = load_alternate_names(
        os.path.join(output_folder, "alternate_chinese_name.json")
    )  # (key, value) = (Geoname_ID, Name)

    # 從 admin2Codes.txt 中獲取臺灣的行政區的 geoname_id 和編號 (e.g. TW.03.TPQ)
    admin1_df = admin2_data[admin2_data["ID"].str.startswith("TW.")]
    admin1_df = admin1_df.reset_index(drop=True)

    # 創建 new_admin1_df (header = ["name", "new_id", "old_id", "geoname_id"])
    new_admin1_df = pd.DataFrame(columns=["name", "en_name", "new_id", "old_id", "geoname_id"])
    for i in range(len(admin1_df)):
        geoname_id = admin1_df.loc[i, "Geoname_ID"]
        old_id = admin1_df.loc[i, "ID"]
        en_name = admin1_df.loc[i, "Name"]
        name = alternate_names[str(geoname_id)]
        new_id = TAIWAN_ADMIN1[name]

        new_admin1_df = new_admin1_df._append(
            {
                "name": name,
                "en_name": en_name,
                "new_id": new_id,
                "old_id": old_id,
                "geoname_id": geoname_id,
            },
            ignore_index=True,
        )

    # 依照 new_id 排序
    new_admin1_df = new_admin1_df.sort_values(by="new_id")

    new_admin1_df.to_csv(output_path, sep="\t", index=False)


def update_taiwan_admin1(admin1_path, new_admin1_map_path, output_path):
    """
    1. 讀取 admin1CodesASCII.txt 和 new_admin1_map.csv
    2. 將台灣的行政區劃資料更新到 admin1CodesASCII.txt
        3.1. 移除 admin1CodesASCII.txt 中 ID 開頭為 "TW." 的行政區劃資料
        3.2. 依照 new_admin1_map ，將新的行政區劃資料插入到 admin1CodesASCII.txt
        3.4. 將新的行政區劃資料存檔到 output_path
    """

    # ID, Name, Name_ASCII, Geoname_ID
    admin1_data = pd.read_csv(
        admin1_path,
        sep="\t",
        header=None,
        names=["ID", "Name", "Name_ASCII", "Geoname_ID"],
    )

    new_admin1_map = pd.read_csv(new_admin1_map_path, sep="\t")
    
    # 移除 admin1CodesASCII.txt 中 ID 開頭為 "TW." 的行政區劃資料
    admin1_data = admin1_data[~admin1_data["ID"].str.startswith("TW.")]

    # 依照 new_admin1_map ，將新的行政區劃資料插入到 admin1CodesASCII.txt
    for i in range(len(new_admin1_map)):
        new_id = new_admin1_map.loc[i, "new_id"]
        name = new_admin1_map.loc[i, "en_name"]
        name_ascii = new_admin1_map.loc[i, "en_name"]
        geoname_id = new_admin1_map.loc[i, "geoname_id"]

        admin1_data = admin1_data._append(
            {
                "ID": new_id,
                "Name": name,
                "Name_ASCII": name_ascii,
                "Geoname_ID": geoname_id,
            },
            ignore_index=True,
        )

    # save the new dataframe to the output path
    admin1_data.to_csv(output_path, sep="\t", index=False)


if __name__ == "__main__":
    data_folder = "geoname_data"
    output_folder = "output"

    admin1_path = os.path.join(data_folder, "admin1CodesASCII.txt")
    admin2_path = os.path.join(data_folder, "admin2Codes.txt")
    new_admin1_path = os.path.join(output_folder, "admin1CodesASCII_en.txt")
    new_admin1_map_path = os.path.join(output_folder, "new_admin1_map.csv")

    get_taiwan_admin1(admin1_path, admin2_path, new_admin1_map_path)
    update_taiwan_admin1(admin1_path, new_admin1_map_path, new_admin1_path)
