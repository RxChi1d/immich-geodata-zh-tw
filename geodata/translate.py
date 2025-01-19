import csv
from utils import load_geo_data, ensure_folder_exists, logger, load_alternate_name
import os
import opencc
from zhconv import convert

alternate_name_folder = "./geoname_data"
alternate_name = load_alternate_name(
    os.path.join(alternate_name_folder, "alternateNamesV2.txt")
)

# 初始化 OpenCC 转换器，繁体转简体
converter_t2s = opencc.OpenCC("t2s")  # 繁体转简体
converter_s2t = opencc.OpenCC("s2t")  # 简体转繁体


def is_chinese(text):
    for char in text:
        if "\u4e00" <= char <= "\u9fff":
            continue  # 中文字符范围
        else:
            return False
    return True


# 判断是否为简体中文
def is_simplified_chinese(text):
    return is_chinese(text) and text == converter_t2s.convert(text)


# 判断是否为繁体中文
def is_traditional_chinese(text):
    return is_chinese(text) and text == converter_s2t.convert(text)


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


def translate_cities500():
    geodata_folder = "./data"
    geodata = load_geodata_list(geodata_folder)

    input_file = "./geoname_data/cities500.txt"
    if not os.path.exists(input_file):
        logger.error(f"输入文件 {input_file} 不存在")
        return

    output_file = f"output/{os.path.basename(input_file)}"
    ensure_folder_exists(output_file)

    with open(input_file, "r", encoding="utf-8") as infile, open(
        output_file, "w", encoding="utf-8"
    ) as outfile:
        reader = csv.reader(infile, delimiter="\t")
        writer = csv.writer(outfile, delimiter="\t")

        for row in reader:
            country_code = row[8]  # 国家码
            latitude = str(row[4])  # 纬度
            longitude = str(row[5])  # 经度

            if (
                country_code in geodata
                and (longitude, latitude) in geodata[country_code]
            ):
                location = geodata[country_code][(longitude, latitude)]
                res = location["admin_2"]
                res = convert(res, "zh-cn")

                # 处理同时存在简繁名字的场景，例如 	东京都/東京都
                # 如果简化后都一样，就只取简化名
                if "/" in res:
                    t = res.split("/")
                    t = [i.strip() for i in t]
                    if len(set(t)) == 1:
                        res = t[0]
                if res:
                    row[1] = res
                    row[2] = res

            elif row[0] in alternate_name:
                name = alternate_name[row[0]]
                name = convert(name, "zh-cn")
                row[1] = name
                row[2] = name

            else:
                candidates = row[3].split(",")
                simplified_word = next(
                    (word for word in candidates if is_simplified_chinese(word)), None
                )
                traditional_word = next(
                    (word for word in candidates if is_traditional_chinese(word)), None
                )

                # 替换第二列内容，优先简体中文词，其次繁体中文词
                if simplified_word:
                    row[1] = simplified_word
                    row[2] = row[1]
                elif traditional_word:
                    row[1] = convert(traditional_word, "zh-cn")
                    row[2] = row[1]

            # 写入处理后的行到输出文件
            writer.writerow(row)


def translate_admin1():
    logger.info("加载 alternateNamesV2 完成")

    # input_file = './geoname_data/admin1CodesASCII.txt'
    def process(input_file):
        if not os.path.exists(input_file):
            logger.error(f"输入文件 {input_file} 不存在")
            return

        output_file = f"output/{os.path.basename(input_file)}"
        ensure_folder_exists(output_file)

        with open(input_file, "r", encoding="utf-8") as infile, open(
            output_file, "w", encoding="utf-8"
        ) as outfile:
            reader = csv.reader(infile, delimiter="\t")
            writer = csv.writer(outfile, delimiter="\t")

            for row in reader:
                code = row[3]

                if code in alternate_name:
                    res = alternate_name[code]
                    res = convert(res, "zh-cn")

                    if res:
                        row[1] = res
                        row[2] = res

                # 写入处理后的行到输出文件
                writer.writerow(row)

    process("./geoname_data/admin1CodesASCII.txt")
    logger.info("admin1CodesASCII.txt 处理完成")

    process("./geoname_data/admin2Codes.txt")
    logger.info("admin2Codes.txt 处理完成")


if __name__ == "__main__":
    translate_cities500()
    translate_admin1()
