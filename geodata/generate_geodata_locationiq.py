import requests
import time
import os
import sys
import csv
from utils import logger, GEODATA_HEADER, load_geo_data

LOCATIONIQ_API_KEY = os.environ["LOCATIONIQ_API_KEY"]
LOCATIONIQ_QPS = int(os.environ.get("LOCATIONIQ_QPS", "1"))

GEONAME_DATA_FILE = "./geoname_data/cities500.txt"

count = 0

from requests.adapters import HTTPAdapter, Retry

s = requests.Session()

retries = Retry(total=5, backoff_factor=0.1, status_forcelist=[403, 500, 502, 503, 504])

s.mount("https://", HTTPAdapter(max_retries=retries))


def get_loc_from_locationiq(lat, lon):
    # 示例：获取坐标的真实路径（假设已有实现）
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


def process_file(file_path, country_code, output_file, existing_data={}):
    # 打开并读取文件
    with open(file_path, "r", encoding="utf-8") as file:
        for line in file:
            fields = line.strip().split("\t")

            # 检查国家码
            if len(fields) > 17 and fields[8] == country_code:
                # print(hello)
                loc = {"lon": str(fields[5]), "lat": str(fields[4])}
                if (loc["lon"], loc["lat"]) in existing_data:
                    continue
                query_and_store(loc, output_file)
                global count
                count += 1
                # if count > 20:
                # break


def query_and_store(coordinate, output_file):
    # 调用 get_loc() 获取结果
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

        if record["admin_2"] == "":
            print(coordinate)

        with open(output_file, mode="a", newline="", encoding="utf-8") as file:
            writer = csv.DictWriter(file, fieldnames=GEODATA_HEADER)

            # 如果文件为空，写入表头
            if file.tell() == 0:
                writer.writeheader()

            # 写入数据
            writer.writerows([record])
            # logger.info(f"新增 {len(records)} 条记录")

    else:
        # 打印失败的坐标
        logger.error(f"查询失败，坐标: {coordinates_batch}")


def main():
    country_code = sys.argv[1]
    logger.info(f"待处理国家码：{country_code}")
    if not os.path.isfile(GEONAME_DATA_FILE):
        raise Exception(f"文件 '{GEONAME_DATA_FILE}' 不存在，请下载后重试。")
    output_file = os.path.join("data", f"{country_code}.csv")
    existing_data = load_geo_data(output_file)
    process_file(
        GEONAME_DATA_FILE, country_code, output_file, existing_data=existing_data
    )


if __name__ == "__main__":
    main()
