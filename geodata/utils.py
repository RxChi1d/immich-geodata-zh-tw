import logging
import os
import csv

logger = logging.getLogger("logger")
logger.setLevel(os.environ.get("LOG_LEVEL", "INFO"))  # 设置最低日志级别

console_handler = logging.StreamHandler()

# 设置日志格式
formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
console_handler.setFormatter(formatter)

# 添加处理器到 logger
logger.addHandler(console_handler)

GEODATA_HEADER = [
    "longitude",
    "latitude",
    "country",
    "admin_1",
    "admin_2",
    "admin_3",
    "admin_4",
]


def load_geo_data(file_path):
    result = {}
    if os.path.exists(file_path):
        # 读取 CSV 文件
        with open(file_path, mode="r", encoding="utf-8") as file:
            reader = csv.DictReader(file)

            # 遍历每行数据
            for row in reader:
                # 获取 (lon, lat) 作为键
                key = (str(row["longitude"]), str(row["latitude"]))

                # 将其他字段作为值存入字典
                value = {
                    "country": row["country"],
                    "admin_1": row["admin_1"],
                    "admin_2": row["admin_2"],
                    "admin_3": row["admin_3"],
                    "admin_4": row["admin_4"],
                }

                result[key] = value

    return result


def ensure_folder_exists(file_path):
    folder = os.path.dirname(file_path)
    if folder:
        os.makedirs(folder, exist_ok=True)


def load_alternate_name(file_path):
    priority = ["zh", "zh-Hans", "zh-SG", "zh-Hant", "zh-HK"]
    mapping = {}
    count = 0
    with open(file_path, "r", encoding="utf-8") as f:
        for line in f:
            count += 1
            if count % 1000000 == 0:
                logger.info(f"{count} 条 alternateName 数据已处理")
            parts = line.strip().split("\t")
            if len(parts) < 4:
                continue  # 忽略格式不正确的行

            _, number, lang, name = parts[:4]  # 提取第二个数字、语言代码和中文名

            # 检查是否符合优先级并更新映射关系
            if lang in priority:
                current_priority = priority.index(lang)
                if number not in mapping or current_priority < mapping[number][1]:
                    mapping[number] = (name, current_priority)

    # 返回最终映射字典，仅保留数字到中文名的映射
    return {key: value[0] for key, value in mapping.items()}
