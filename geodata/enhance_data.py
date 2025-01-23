import os
from utils import logger

MIN_POPULATION = 1

# 文件路径
data_base_dir = "./geoname_data"
cities_file = os.path.join(data_base_dir, "cities500.txt")
extra_data_folder = os.path.join(data_base_dir, "extra_data/")

# 读取 cities500.txt 中的所有 id
with open(cities_file, "r") as f:
    existing_ids = set(line.split(",")[0] for line in f)

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
                    if id_value not in existing_ids and num_value >= MIN_POPULATION:
                        c += 1
                        append_f.write(line)
                        existing_ids.add(id_value)  # 添加到已存在的 id 集合中

                logger.info(f"共 {c} 条数据被追加")
