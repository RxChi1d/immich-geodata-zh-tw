"""
臺灣村里級行政區界資料處理工具。

輸入資料說明
------------
- 檔案來源：國土測繪中心（NLSC）
  https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx
- 使用資料：「村(里)界（TWD97經緯度）」
- 解壓縮：請將下載的檔案完整解壓縮到
  `geoname_data/VILLAGE_NLSC_YYYMMDD`（YYYMMDD 為資料日期）。

目錄結構範例（以 1140620 為例）：

    geoname_data
    └── VILLAGE_NLSC_1140620
        ├── 修正清單_1140620.xlsx
        ├── TW-07-301000100G-613995.xml
        ├── VILLAGE_NLSC_1140620.CPG
        ├── VILLAGE_NLSC_1140620.dbf
        ├── VILLAGE_NLSC_1140620.prj
        ├── VILLAGE_NLSC_1140620.shp
        ├── VILLAGE_NLSC_1140620.shx
        ├── Village_Sanhe.CPG
        ├── Village_Sanhe.dbf
        ├── Village_Sanhe.prj
        ├── Village_Sanhe.shp
        └── Village_Sanhe.shx

使用提示
------
 - 透過 `--shapefile` 參數指定 `.shp` 檔路徑（未指定則使用預設路徑），例如：
  `python core/taiwan_geodata.py --shapefile geoname_data/VILLAGE_NLSC_1140620/VILLAGE_NLSC_1140620.shp`。
 - 本腳本會計算多邊形中心點，並輸出 WGS84 經緯度到 CSV，
   以利後續反向地理編碼使用。
"""

import sys
import os
from pathlib import Path

# 添加專案根目錄到 Python 路徑
sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

import polars as pl
import geopandas as gpd
from core.utils import logger

# 設定 GDAL 選項，允許自動重建 .shx 檔案
os.environ["SHAPE_RESTORE_SHX"] = "YES"


class TaiwanGeoData:
    """
    處理台灣地理資料的類別，主要從 Shapefile 讀取資料、
    計算中心點並轉換為指定的 CSV 格式。

    注意：
        - 此模組預期使用的 Shapefile 圖資來源為：
          國土測繪中心 (https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
          資料集名稱：村(里)界 (TWD97經緯度)
          版本/日期：1140620 (或更新版本)
        - 請確保已安裝 geopandas 及其相關依賴。
        - 處理過程會將座標系統轉換，最終輸出 WGS84 格式的中心點座標。
    """

    def __init__(self, shapefile_path: str):
        """
        初始化 TaiwanGeoData 類別

        Args:
            shapefile_path: Shapefile 檔案路徑
        """
        self.shapefile_path = Path(shapefile_path)
        self.df = None

    def load_shapefile(self) -> pl.DataFrame:
        """讀取 Shapefile 並轉換為 Polars DataFrame"""
        try:
            logger.info(f"正在讀取 Shapefile: {self.shapefile_path}")

            # 使用 geopandas 讀取 Shapefile
            gdf = gpd.read_file(self.shapefile_path)
            logger.info(
                f"成功讀取 Shapefile，資料集大小: {gdf.shape[0]} 行 x {gdf.shape[1]} 列"
            )

            # 檢查原始座標系統
            logger.info(f"原始座標系統: {gdf.crs}")

            # 先轉換到投影座標系統計算中心點
            logger.info("正在轉換到投影座標系統 (TWD97 / TM2 zone 121)...")
            gdf = gdf.to_crs(epsg=3826)

            # 在投影座標系統下計算中心點
            logger.info("正在計算中心點...")
            centroids = gdf.geometry.centroid

            # 將中心點轉換回 WGS84
            logger.info("正在將中心點轉換回 WGS84...")
            centroids = centroids.to_crs(epsg=4326)
            gdf["longitude"] = centroids.x
            gdf["latitude"] = centroids.y

            # 移除 geometry 欄位
            gdf = gdf.drop(columns=["geometry"])

            # 將所有 object 類型的欄位轉換為字串
            for col in gdf.columns:
                if gdf[col].dtype == "object":
                    gdf[col] = gdf[col].astype(str)

            # 轉換為 Polars DataFrame
            self.df = pl.from_pandas(gdf)

            # 選擇需要的欄位
            self.df = self.df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("COUNTYNAME").alias("admin_1"),  # 縣市
                    pl.col("TOWNNAME").alias("admin_2"),  # 鄉鎮市區
                    pl.col("VILLNAME").alias("admin_3"),  # 村里
                    pl.lit("").alias("admin_4"),  # 鄰 - 設為空字串
                    pl.lit("臺灣").alias("country"),  # 國家
                ]
            )

            # 按照 country, admin_1, admin_2 進行排序
            self.df = self.df.sort(["country", "admin_1", "admin_2"])

            # 移除無效的資料點
            self.df = self.df.filter(
                pl.col("longitude").is_not_null() & pl.col("latitude").is_not_null()
            )

            logger.info(f"成功處理資料，資料集大小: {len(self.df)} 筆")
            return self.df

        except Exception as e:
            logger.error(f"處理 Shapefile 時發生錯誤: {e}")
            raise

    def save_to_csv(self, output_path: str) -> None:
        """
        將處理後的資料儲存為 CSV 檔案

        Args:
            output_path: 輸出的 CSV 檔案路徑
        """
        if self.df is None:
            raise ValueError("請先處理資料")

        try:
            output_path = Path(output_path)
            output_path.parent.mkdir(parents=True, exist_ok=True)

            logger.info(f"正在儲存 CSV 檔案: {output_path}")
            self.df.write_csv(output_path)
            logger.info(f"成功儲存 CSV 檔案，共 {len(self.df)} 筆資料")

        except Exception as e:
            logger.error(f"儲存 CSV 檔案時發生錯誤: {e}")
            raise

    def process(self, output_path: str) -> None:
        """
        執行完整的處理流程

        Args:
            output_path: 輸出的 CSV 檔案路徑
        """
        self.load_shapefile()
        self.save_to_csv(output_path)


def process_taiwan_geodata(shapefile_path: str, output_path: str) -> None:
    """
    處理台灣行政區劃資料的便捷函數

    Args:
        shapefile_path: Shapefile 檔案路徑
        output_path: 輸出的 CSV 檔案路徑
    """
    processor = TaiwanGeoData(shapefile_path)
    processor.process(output_path)


if __name__ == "__main__":
    import sys
    import os
    from pathlib import Path
    import argparse

    parser = argparse.ArgumentParser(description="處理臺灣村里界 Shapefile 產生 CSV。")
    parser.add_argument(
        "-s",
        "--shapefile",
        type=str,
        default=str(
            Path("geoname_data") / "VILLAGE_NLSC_1140620" / "VILLAGE_NLSC_1140620.shp"
        ),
        help=(
            "Shapefile 檔案路徑。預設：geoname_data/VILLAGE_NLSC_1140620/VILLAGE_NLSC_1140620.shp"
        ),
    )
    parser.add_argument(
        "-o",
        "--output",
        type=str,
        default=str(Path("meta_data") / "taiwan_geodata.csv"),
        help="輸出的 CSV 檔案路徑（預設：meta_data/taiwan_geodata.csv）",
    )

    args = parser.parse_args()

    # Resolve CWD and absolute paths.
    cwd = Path(os.getcwd())
    shp_path = Path(args.shapefile)
    if not shp_path.is_absolute():
        shp_path = cwd / shp_path
    # 僅接受 .shp 檔案，且必須存在
    if not (shp_path.is_file() and shp_path.suffix.lower() == ".shp"):
        logger.error(f"請提供存在的 .shp 檔案路徑：{shp_path}")
        sys.exit(1)

    output_path = Path(args.output)
    if not output_path.is_absolute():
        output_path = cwd / output_path

    try:
        logger.info(f"使用 Shapefile：{shp_path}")
        logger.info(f"輸出 CSV：{output_path}")
        logger.info("開始處理台灣行政區劃資料...")
        process_taiwan_geodata(str(shp_path), str(output_path))
        logger.info("處理完成！")

        # Preview head rows
        df = pl.read_csv(output_path)
        logger.info("\n處理後的資料預覽：")
        logger.info(df.head())

    except Exception as e:
        logger.error(f"處理過程中發生錯誤: {e}")
        sys.exit(1)
