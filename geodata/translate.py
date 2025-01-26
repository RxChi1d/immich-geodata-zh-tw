import csv
from utils import load_geo_data, ensure_folder_exists, logger, load_alternate_name
import os
import sys
import pandas as pd

# import opencc
from zhconv import convert

source_folder = "./geoname_data"
output_folder = "./output"

# load alternate chinese name mapping
alternate_name = load_alternate_name(
    os.path.join(output_folder, "alternate_chinese_name.json")
)

# # 初始化 OpenCC 转换器，繁体转简体
# converter_t2s = opencc.OpenCC("t2s")  # 繁体转简体
# converter_s2t = opencc.OpenCC("s2t")  # 简体转繁体


def is_chinese(text):
    for char in text:
        if "\u4e00" <= char <= "\u9fff":
            continue  # 中文字符范围
        else:
            return False
    return True


# # 判断是否为简体中文
# def is_simplified_chinese(text):
#     return is_chinese(text) and text == converter_t2s.convert(text)


# # 判断是否为繁体中文
# def is_traditional_chinese(text):
#     return is_chinese(text) and text == converter_s2t.convert(text)


def load_geodata_list(folder_path):
    geo_dict = {}

    # 遍历文件夹中的所有文件
    for file_name in os.listdir(folder_path):
        if file_name.endswith(".csv"):
            # 去掉文件扩展名作为 key
            key = os.path.splitext(file_name)[0]
            file_path = os.path.join(folder_path, file_name)
            geo_dict[key] = load_geo_data(file_path)

    return geo_dict


# def translate_cities500():
#     geodata_folder = "./data"
#     geodata = load_geodata_list(geodata_folder)

#     input_file = "./geoname_data/cities500.txt"
#     if not os.path.exists(input_file):
#         logger.error(f"输入文件 {input_file} 不存在")
#         return

#     output_file = f"output/{os.path.basename(input_file)}"
#     ensure_folder_exists(output_file)

#     with open(input_file, "r", encoding="utf-8") as infile, open(
#         output_file, "w", encoding="utf-8"
#     ) as outfile:
#         reader = csv.reader(infile, delimiter="\t")
#         writer = csv.writer(outfile, delimiter="\t")

#         for row in reader:
#             country_code = row[8]  # 国家码
#             latitude = str(row[4])  # 纬度
#             longitude = str(row[5])  # 经度

#             if (
#                 country_code in geodata
#                 and (longitude, latitude) in geodata[country_code]
#             ):
#                 location = geodata[country_code][(longitude, latitude)]
#                 res = location["admin_2"]
#                 res = convert(res, "zh-cn")

#                 # 处理同时存在简繁名字的场景，例如 	东京都/東京都
#                 # 如果简化后都一样，就只取简化名
#                 if "/" in res:
#                     t = res.split("/")
#                     t = [i.strip() for i in t]
#                     if len(set(t)) == 1:
#                         res = t[0]
#                 if res:
#                     row[1] = res
#                     row[2] = res

#             elif row[0] in alternate_name:
#                 name = alternate_name[row[0]]
#                 name = convert(name, "zh-cn")
#                 row[1] = name
#                 row[2] = name

#             else:
#                 candidates = row[3].split(",")
#                 simplified_word = next(
#                     (word for word in candidates if is_simplified_chinese(word)), None
#                 )
#                 traditional_word = next(
#                     (word for word in candidates if is_traditional_chinese(word)), None
#                 )

#                 # 替换第二列内容，优先简体中文词，其次繁体中文词
#                 if simplified_word:
#                     row[1] = simplified_word
#                     row[2] = row[1]
#                 elif traditional_word:
#                     row[1] = convert(traditional_word, "zh-cn")
#                     row[2] = row[1]

#             # 写入处理后的行到输出文件
#             writer.writerow(row)


def translate_admin1(input_file):
    logger.info(f"Processing {input_file}")

    if not os.path.exists(input_file):
        logger.error(f"The input file {input_file} does not exist")
        sys.exit(1)

    output_file = os.path.join(output_folder, os.path.basename(input_file))
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

    logger.info(f"Processed file saved to {output_file}")


if __name__ == "__main__":
    # translate_cities500()

    translate_admin1(os.path.join(source_folder, "admin1CodesASCII.txt"))
    translate_admin1(os.path.join(source_folder, "admin2Codes.txt"))
