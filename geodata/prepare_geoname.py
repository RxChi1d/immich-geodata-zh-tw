import os
import shutil
import requests
import zipfile
import argparse
from utils import logger

def download_file(url, output_path):
    try:
        response = requests.get(url, stream=True)
        response.raise_for_status()
        with open(output_path, 'wb') as file:
            for chunk in response.iter_content(chunk_size=1024):
                file.write(chunk)
        logger.info(f"下載完成: {output_path}")
    except requests.RequestException as e:
        logger.error(f"下載失敗: {url} - {e}")
        exit(1)

def unzip_file(zip_path, extract_to):
    try:
        with zipfile.ZipFile(zip_path, 'r') as zip_ref:
            zip_ref.extractall(extract_to)
        logger.info(f"解壓完成: {zip_path}")
    except zipfile.BadZipFile:
        logger.error(f"解壓失敗: {zip_path}")
        exit(1)

def remove_file(file_path):
    if os.path.exists(file_path):
        os.remove(file_path)
        logger.info(f"已刪除: {file_path}")

def remove_folder(folder_path):
    if os.path.exists(folder_path):
        shutil.rmtree(folder_path)
        logger.info(f"已刪除資料夾: {folder_path}")

def main(update=False):
    TARGET_DIR = "geoname_data"
    ZIP_FILE = os.path.join(TARGET_DIR, "cities500.zip")
    TXT_FILE = os.path.join(TARGET_DIR, "cities500.txt")
    ADMIN1_FILE = os.path.join(TARGET_DIR, "admin1CodesASCII.txt")
    ADMIN2_FILE = os.path.join(TARGET_DIR, "admin2Codes.txt")
    GEOJSON_FILE = os.path.join(TARGET_DIR, "ne_10m_admin_0_countries.geojson")
    EXTRA_DATA_DIR = os.path.join(TARGET_DIR, "extra_data")
    
    if update:
        remove_folder(TARGET_DIR)
    
    os.makedirs(TARGET_DIR, exist_ok=True)
    os.makedirs(EXTRA_DATA_DIR, exist_ok=True)
    
    download_file("https://download.geonames.org/export/dump/cities500.zip", ZIP_FILE)
    unzip_file(ZIP_FILE, TARGET_DIR)
    remove_file(ZIP_FILE)
    
    countries = ["TW"]
    for country in countries:
        country_zip = os.path.join(EXTRA_DATA_DIR, f"{country}.zip")
        country_txt = os.path.join(EXTRA_DATA_DIR, f"{country}.txt")
        download_file(f"https://download.geonames.org/export/dump/{country}.zip", country_zip)
        unzip_file(country_zip, EXTRA_DATA_DIR)
        if not os.path.exists(country_txt):
            logger.error(f"未找到 {country_txt}")
            exit(1)
        remove_file(country_zip)
    
    download_file("https://download.geonames.org/export/dump/admin1CodesASCII.txt", ADMIN1_FILE)
    download_file("https://download.geonames.org/export/dump/admin2Codes.txt", ADMIN2_FILE)
    download_file("https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_admin_0_countries.geojson", GEOJSON_FILE)
    
    alternate_zip = os.path.join(TARGET_DIR, "alternateNamesV2.zip")
    alternate_txt = os.path.join(TARGET_DIR, "alternateNamesV2.txt")
    
    if not os.path.exists(alternate_txt):
        download_file("https://download.geonames.org/export/dump/alternateNamesV2.zip", alternate_zip)
        unzip_file(alternate_zip, TARGET_DIR)
        remove_file(alternate_zip)
    else:
        logger.info(f"{alternate_txt} 已存在，跳過下載。")
    
    logger.info("地理名稱數據下載完成")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="下載並處理地理名稱數據。")
    parser.add_argument("--update", action="store_true", help="刪除現有資料夾並重新下載所有數據")
    args = parser.parse_args()
    
    main(update=args.update)
