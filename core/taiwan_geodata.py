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
          版本/日期：1131128 (或更新版本)
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
    # 測試用的程式碼
    import sys
    from pathlib import Path
    import os

    # 獲取當前工作目錄
    current_working_dir = Path(os.getcwd())
    logger.info(f"當前工作目錄: {current_working_dir}")

    # 設定測試用的檔案路徑
    shapefile_path = (
        current_working_dir
        / "geoname_data"
        / "VILLAGE_NLSC_1131128"
        / "VILLAGE_NLSC_1131128.shp"
    )
    output_path = current_working_dir / "meta_data" / "taiwan_geodata.csv"

    # 檢查輸入檔案是否存在
    if not shapefile_path.exists():
        logger.error(f"找不到 Shapefile: {shapefile_path}")
        sys.exit(1)

    try:
        # 執行處理
        logger.info("開始處理台灣行政區劃資料...")
        process_taiwan_geodata(str(shapefile_path), str(output_path))
        logger.info("處理完成！")

        # 顯示處理後的資料預覽
        df = pl.read_csv(output_path)
        logger.info("\n處理後的資料預覽：")
        logger.info(df.head())

    except Exception as e:
        logger.error(f"處理過程中發生錯誤: {e}")
        sys.exit(1)
