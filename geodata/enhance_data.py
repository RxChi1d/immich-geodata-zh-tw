import os
from utils import logger

MIN_POPULATION = 100

# 文件路径
data_base_dir = "./geoname_data"
cities_file = os.path.join(data_base_dir, "cities500.txt")
extra_data_folder = os.path.join(data_base_dir, "extra_data/")

need_finer_admin_2_list = [
    "CN.23.12324204",  # 上海
    "CN.22.11876380",  # 北京
    "CN.33.8739734",  # 重庆
    "CN.28.12324202",  # 天津
]


def need_finer_data(data):
    feature_class = data[6]
    if feature_class not in ["P", "A"]:
        return False

    country_code = data[8]
    admin1_code = data[10]
    admin2_code = data[11]

    code = f"{country_code}.{admin1_code}.{admin2_code}"
    if code not in need_finer_admin_2_list:
        return False
    return True


# 读取 cities500.txt 中的所有 id
with open(cities_file, "r") as f:
    existing_ids = set(line.split("\t")[0] for line in f)

with open(cities_file, "a") as append_f:
    # 遍历 extra_data 文件夹中的所有 txt 文件
    for file_name in os.listdir(extra_data_folder):
        if file_name.endswith(".txt"):
            logger.info(f"开始处理 {file_name}")
            file_path = os.path.join(extra_data_folder, file_name)

            with open(file_path, "r") as f:
                c = 0
                for line in f:
                    columns = line.split("\t")

                    # 第一个 id
                    id_value = columns[0]

                    try:
                        num_value = float(columns[14])
                    except ValueError:
                        continue  # 如果无法转换为数字，跳过此行

                    # 判断是否满足追加条件
                    if id_value not in existing_ids and (
                        num_value >= MIN_POPULATION or need_finer_data(columns)
                    ):
                        c += 1
                        append_f.write(line)
                        existing_ids.add(id_value)  # 添加到已存在的 id 集合中

                logger.info(f"共 {c} 条数据被追加")
