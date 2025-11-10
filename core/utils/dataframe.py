"""Polars DataFrame 輔助工具。"""

import polars as pl


def fill_admin_columns(df: pl.DataFrame) -> pl.DataFrame:
    """將 geodata DataFrame 的 admin 欄位缺值轉為空字串。"""

    return df.with_columns(
        [
            pl.col("admin_1").fill_null(""),
            pl.col("admin_2").fill_null(""),
            pl.col("admin_3").fill_null(""),
            pl.col("admin_4").fill_null(""),
        ]
    )

