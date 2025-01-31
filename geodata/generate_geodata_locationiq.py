import os
import time
from tqdm import tqdm
import csv
from utils import logger, load_meta_data, CITIES_HEADER, GEODATA_HEADER, MUNICIPALITIES, ensure_folder_exists
import requests
from requests.adapters import HTTPAdapter, Retry
import pandas as pd
import argparse


LOCATIONIQ_API_KEY = os.environ["LOCATIONIQ_API_KEY"]
LOCATIONIQ_QPS = int(os.environ.get("LOCATIONIQ_QPS", "1"))


s = requests.Session()

retries = Retry(total=5, backoff_factor=0.1, status_forcelist=[403, 500, 502, 503, 504])

s.mount("https://", HTTPAdapter(max_retries=retries))


def get_loc_from_locationiq(lat, lon):
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
    except:
        logger.error(f"{lat},{lon} failed to get location")
        pass
    return None


def process_file(cities500_file, output_file, country_code, overwrite=False):
    existing_data = load_meta_data(output_file) if not overwrite else {}

    cities_df = pd.read_csv(
        cities500_file, sep="\t", header=None, names=CITIES_HEADER, low_memory=False
    )

    admin1_map = pd.read_csv(
        os.path.join(
            os.path.dirname(os.path.dirname(output_file)), "tw_admin1_map.csv"
        ),
        sep=",",
        low_memory=False,
    )

    # 用 append 模式，避免超過 api 限制
    write_mode = "w" if overwrite else "a"
    ensure_folder_exists(output_file)
    with open(output_file, mode=write_mode, newline="", encoding="utf-8") as file:
        writer = csv.DictWriter(file, fieldnames=GEODATA_HEADER)

        # 如果文件是空的，寫入 header
        if file.tell() == 0:
            writer.writeheader()
            file.flush()

        specific_country_df = cities_df[cities_df["country_code"] == country_code]

        pbar = tqdm(specific_country_df.iterrows(), total=len(specific_country_df))
        for index, row in pbar:
            pbar.set_description(f"[City: {row['name']}]")

            loc = {"lon": str(row["longitude"]), "lat": str(row["latitude"])}

            if (loc["lon"], loc["lat"]) in existing_data:
                continue

            record = reverse_query(loc)

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
            # 臺灣特殊處理，調整行政區層級
            if country_code == "TW":
                if record["admin_2"] in MUNICIPALITIES:
                    admin_1 = f"TW.{row['admin1_code']}"
                    record["admin_1"] = admin1_map[admin1_map["new_id"] == admin_1][
                        "name"
                    ].values[0]
                    record["admin_2"] = record["admin_3"]
                    record["admin_3"] = record["admin_4"]
                    record["admin_4"] = ""

                else:
                    admin_1 = f"TW.{row['admin1_code']}"
                    record["admin_1"] = admin1_map[admin1_map["new_id"] == admin_1][
                        "name"
                    ].values[0]

            # 如果查詢成功，寫入文件
            if record:
                writer.writerows([record])
                file.flush()
            else:
                logger.error(f"查詢失敗，座標: {loc}")


def reverse_query(coordinate):
    response = get_loc_from_locationiq(coordinate["lat"], coordinate["lon"])

    if response:
        address = response["address"]

        record = {
            "latitude": coordinate["lat"],
            "longitude": coordinate["lon"],
            "country": address["country"],
            "admin_1": address.get("state", ""),
            "admin_2": address.get("city", address.get("county", "")),
            "admin_3": address.get("suburb", ""),
            "admin_4": address.get("neighbourhood", ""),
        }

        return record

    else:
        return None


def run():
    parser = argparse.ArgumentParser()
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--country_code", type=str, default="TW")
    parser.add_argument("--output_folder", type=str, default="./output")
    args = parser.parse_args()

    output_folder = "./output"
    meta_data_folder = os.path.join(args.output_folder, "meta_data")
    cities500_file = os.path.join(args.output_folder, "cities500_optimized.txt")

    logger.info(f"通過 LocationIQ 生成 {args.country_code} 的 metadata")

    if not os.path.exists(cities500_file):
        raise FileExistsError(f"{cities500_file} 不存在，請先下載。")

    output_file = os.path.join(meta_data_folder, f"{args.country_code}.csv")

    process_file(cities500_file, output_file, args.country_code, args.overwrite)


if __name__ == "__main__":
    run()
