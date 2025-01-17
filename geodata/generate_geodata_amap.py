import requests
import time
import os
import sys
import csv
from utils import logger, GEODATA_HEADER, load_geo_data

AMAP_API_KEY = os.environ["AMAP_API_KEY"]
AMAP_QPS = int(os.environ.get("AMAP_QPS", "3"))
AMAP_BATCH_SIZE = int(os.environ.get("AMAP_BATCH_SIZE", "20"))

GEONAME_DATA_FILE = "./geoname_data/cities500.txt"


def get_loc_from_amap(loc_list):
    # 示例：获取坐标的真实路径（假设已有实现）
    loc = "|".join([f"{loc['lon']},{loc['lat']}" for loc in loc_list])
    r = f"https://restapi.amap.com/v3/geocode/regeo?output=json&location={loc}&key={AMAP_API_KEY}&radius=1000&extensions=all&batch=true"
    # print(r)
    resp = requests.get(r)
    time.sleep(1.02 / AMAP_QPS)
    return resp.json()


def process_file(file_path, country_code, output_file, existing_data={}):
    batch_size = AMAP_BATCH_SIZE
    coordinates_batch = []

    # 打开并读取文件
    with open(file_path, "r", encoding="utf-8") as file:
        for line in file:
            fields = line.strip().split("\t")

            # 检查国家码
            if len(fields) > 17 and fields[8] == country_code:
                # print(hello)
                loc = {"lon": str(fields[5]), "lat": str(fields[4])}
                if (loc["lon"], loc["lat"]) not in existing_data:
                    coordinates_batch.append(loc)

                # 如果达到批量大小，执行查询
                if len(coordinates_batch) == batch_size:
                    query_and_store(coordinates_batch, output_file)
                    coordinates_batch = []

        # 处理剩余的坐标
        if coordinates_batch:
            query_and_store(coordinates_batch, output_file)


def query_and_store(coordinates_batch, output_file):
    # 调用 get_loc() 获取结果
    response = get_loc_from_amap(coordinates_batch)

    if response.get("status") == "1":
        regeocode_data = response["regeocodes"]
        results = [
            {
                "latitude": coord["lat"],
                "longitude": coord["lon"],
                "address_component": data["addressComponent"],
            }
            for coord, data in zip(coordinates_batch, regeocode_data)
        ]

        # 保存到文件
        if results:
            records = []
            for item in results:
                # if item["address_component"]["province"] == []:
                #     continue
                x = {
                    "latitude": item["latitude"],
                    "longitude": item["longitude"],
                    "country": item["address_component"]["country"],
                    "admin_1": item["address_component"]["province"],
                    "admin_2": (
                        item["address_component"]["city"]
                        if item["address_component"]["city"]
                        else item["address_component"]["district"]
                    ),
                    "admin_3": item["address_component"]["district"]
                    if item["address_component"]["district"]
                    else item["address_component"]["city"],
                    "admin_4": item["address_component"]["township"],
                }
                for k in x:
                    if x[k] == []:
                        x[k] = ""
                records.append(x)

            with open(output_file, mode="a", newline="", encoding="utf-8") as file:
                writer = csv.DictWriter(file, fieldnames=GEODATA_HEADER)

                # 如果文件为空，写入表头
                if file.tell() == 0:
                    writer.writeheader()

                # 写入数据
                writer.writerows(records)
                logger.info(f"新增 {len(records)} 条记录")

        else:
            # 打印失败的坐标
            logger.error(f"查询失败，坐标: {coordinates_batch}，响应：{response}")


def main():
    country_code = "CN" if len(sys.argv) == 1 else sys.argv[1]
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
