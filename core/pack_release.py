import os
import sys
import shutil
from datetime import datetime

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.utils import logger


def remove_old_releases(output_dir):
    for item in os.listdir(output_dir):
        item_path = os.path.join(output_dir, item)
        if item.startswith("release"):
            if os.path.isdir(item_path):
                shutil.rmtree(item_path)
                logger.info(f"已刪除資料夾: {item_path}")
            elif os.path.isfile(item_path):
                os.remove(item_path)
                logger.info(f"已刪除檔案: {item_path}")


def pack(output_dir):
    current_date = datetime.now().strftime("%Y-%m-%d")
    release_name = "release"
    release_dir = os.path.join(output_dir, release_name)
    geodata_dir = os.path.join(release_dir, "geodata")
    zip_file = os.path.join(output_dir, f"{release_name}.zip")

    # 清理舊的 release 目錄和 zip 檔案
    remove_old_releases(output_dir)

    os.makedirs(geodata_dir, exist_ok=True)

    # 需要複製的檔案
    files_to_copy = {
        "geoname_data/ne_10m_admin_0_countries.geojson": os.path.join(
            geodata_dir, "ne_10m_admin_0_countries.geojson"
        ),
        "output/admin1CodesASCII_translated.txt": os.path.join(
            geodata_dir, "admin1CodesASCII.txt"
        ),
        "output/admin2Codes_translated.txt": os.path.join(
            geodata_dir, "admin2Codes.txt"
        ),
        "output/cities500_translated.txt": os.path.join(geodata_dir, "cities500.txt"),
    }

    for src, dst in files_to_copy.items():
        try:
            shutil.copy(src, dst)
            logger.info(f"複製 {src} 到 {dst} 成功")
        except IOError:
            logger.error(f"複製 {src} 失敗！退出。")
            exit(1)

    # 建立 geodata-date.txt 檔案
    date_file = os.path.join(geodata_dir, "geodata-date.txt")
    with open(date_file, "w") as f:
        f.write(current_date)
    logger.info(f"建立 {date_file}")

    # 複製 i18n-iso-countries 目錄
    shutil.copytree(
        "i18n-iso-countries", os.path.join(release_dir, "i18n-iso-countries")
    )

    # 壓縮 release 目錄
    shutil.make_archive(os.path.join(output_dir, release_name), "zip", release_dir)
    logger.info(f"打包完成: {zip_file}")

def test():
    output_folder = "output"
    pack(output_folder)

if __name__ == "__main__":
    logger.error("請使用 main.py 作為主要接口，而非直接執行 generate_release.py")
