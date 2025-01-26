import os
import pandas as pd

from utils import load_alternate_names

taiwan_admin1 = {
    # 直轄市 (Special Municipalities)
    "臺北市": "TW.01",
    "新北市": "TW.02",
    "桃園市": "TW.03",
    "臺中市": "TW.04",
    "臺南市": "TW.05",
    "高雄市": "TW.06",
    # 省轄市 (Provincial Cities)
    "基隆市": "TW.07",
    "新竹市": "TW.08",
    "嘉義市": "TW.09",
    # 縣 (Counties)
    "宜蘭縣": "TW.10",
    "新竹縣": "TW.11",
    "苗栗縣": "TW.12",
    "彰化縣": "TW.13",
    "南投縣": "TW.14",
    "雲林縣": "TW.15",
    "嘉義縣": "TW.16",
    "屏東縣": "TW.17",
    "臺東縣": "TW.18",
    "花蓮縣": "TW.19",
    "澎湖縣": "TW.20",
    "金門縣": "TW.21",
    "連江縣": "TW.22",
}

if __name__ == "__main__":
    data_folder = "geoname_data"
    output_folder = "output"
    
    admin1_path = os.path.join(data_folder, "admin1CodesASCII.txt")

    # ID, Name, Name_ASCII, Geoname_ID
    admin1_data = pd.read_csv(
        admin1_path,
        sep="\t",
        header=None,
        names=["ID", "Name", "Name_ASCII", "Geoname_ID"],
    )
    
    alternate_names = load_alternate_names(os.path.join(output_folder, "alternate_chinese_name.json"))   # (key, value) = (Geoname_ID, Name)
    alternate_names = {v: k for k, v in alternate_names.items()} # (key, value) = (Name, Geoname_ID)

    # find the row which ID is start with "TW."
    # and replace the Name with the value in taiwan_admin1
    # because the ID in admin1CodesASCII.txt is less than taiwan_admin1
    # so we need to remove the orginal row which ID is start with "TW."
    # and insert the new row with the value in taiwan_admin1
    
    # create a new dataframe from taiwan_admin1
    # Geoname_ID can be finded in alternate_names
    new_admin1_data = pd.DataFrame(
        [
            {"ID": value, "Name": key, "Name_ASCII": key, "Geoname_ID": alternate_names[key]}
            for key, value in taiwan_admin1.items()
        ]
    )
    
    # remove the row which ID is start with "TW."
    admin1_data = admin1_data[~admin1_data["ID"].str.startswith("TW.")]
    
    # concat the new dataframe and the old dataframe
    admin1_data = pd.concat([admin1_data, new_admin1_data], ignore_index=True)
    
    print(admin1_data[-50:])