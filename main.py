"""
main.py - 專案主要接口

透過下列子命令可操作不同功能：
  cleanup    清理舊資料
  prepare    下載並處理 geoname 資料
  enhance    優化 cities500 資料
  locationiq 使用 LocationIQ API 取得 metadata
  translate  翻譯地名資料
  modify     修改臺灣行政區代碼
  pack       產生 release 壓縮檔
  release    依序執行所有步驟

使用方式:
    python main.py <子命令> [選項]

例如：
    python main.py prepare --update
    python main.py enhance --min-population 100
    python main.py release --overwrite
"""

import argparse
import os
import sys

# 從 core 模組引入工具與各子模組
from core.utils import logger, rebuild_folder
from core import (
    prepare_geoname,
    enhance_data,
    generate_geodata_locationiq,
    translate,
    modify_admin1,
    pack_release,
)


def cmd_cleanup(args):
    """清理舊資料 (重建 output 資料夾)"""
    rebuild_folder(args.output_folder)
    logger.info("清理完成。")


def cmd_prepare(args):
    """下載並處理 geoname 資料"""
    prepare_geoname.download(args.country_code, args.update)
    logger.info("prepare 步驟完成。")


def cmd_enhance(args):
    """優化 cities500 資料"""
    cities_file = args.cities_file or os.path.join("geoname_data", "cities500.txt")
    extra_files = [
        os.path.join("geoname_data", "extra_data", f"{cc}.txt")
        for cc in args.country_code
    ]
    output_file = args.output_file or os.path.join("output", "cities500_optimized.txt")
    enhance_data.update_cities500(
        cities_file, extra_files, output_file, args.min_population
    )
    logger.info("enhance 步驟完成。")


def cmd_locationiq(args):
    """使用 LocationIQ API 取得 metadata，支援多國家代碼"""
    # 取得 API Key 與 QPS，若未提供則嘗試從環境變數讀取
    api_key = args.locationiq_api_key or os.environ.get("LOCATIONIQ_API_KEY")
    if not api_key:
        logger.critical(
            "必須提供 LocationIQ API Key (透過 --locationiq-api-key 或環境變數 LOCATIONIQ_API_KEY)"
        )
        sys.exit(1)
    qps = (
        args.locationiq_qps
        if args.locationiq_qps is not None
        else int(os.environ.get("LOCATIONIQ_QPS", "1"))
    )

    # 設定 LocationIQ 配置
    generate_geodata_locationiq.set_locationiq_config(api_key, qps)

    output_folder = args.output_folder
    batch_size = args.batch_size
    meta_data_folder = "./meta_data"
    os.makedirs(meta_data_folder, exist_ok=True)
    cities_file = os.path.join(output_folder, "cities500_optimized.txt")

    # 處理每個國家代碼
    for cc in args.country_code:
        output_file = os.path.join(meta_data_folder, f"{cc}.csv")
        if args.overwrite and os.path.exists(output_file):
            os.remove(output_file)
        generate_geodata_locationiq.process_file(
            cities_file, output_file, cc, batch_size
        )
        logger.info(f"locationiq: 處理 {cc} 完成。")


def cmd_translate(args):
    """翻譯地名資料，包含 cities500 與 admin1/admin2"""
    source_folder = args.source_folder
    output_folder = args.output_folder
    metadata_folder = "./meta_data"

    # 處理 cities500 翻譯
    cities500_file = os.path.join(output_folder, "cities500_optimized.txt")
    output_file = os.path.join(output_folder, "cities500_translated.txt")
    alternate_name_file = args.alternate_name_file or os.path.join(
        output_folder, "alternate_chinese_name.csv"
    )
    translate.translate_cities500(
        metadata_folder, cities500_file, output_file, alternate_name_file
    )

    # 處理 admin1 與 admin2 翻譯
    admin1_file = os.path.join(output_folder, "admin1CodesASCII_optimized.txt")
    admin2_file = os.path.join(source_folder, "admin2Codes.txt")
    translate.translate_admin1(admin1_file, alternate_name_file, output_folder)
    translate.translate_admin1(admin2_file, alternate_name_file, output_folder)
    logger.info("translate 步驟完成。")


def cmd_modify(args):
    """修改臺灣行政區代碼"""
    data_folder = args.data_folder
    output_folder = args.output_folder
    admin1_path = os.path.join(data_folder, "admin1CodesASCII.txt")
    admin2_path = os.path.join(data_folder, "admin2Codes.txt")
    new_admin1_path = os.path.join(output_folder, "admin1CodesASCII_optimized.txt")
    tw_admin1_map_path = os.path.join(output_folder, "tw_admin1_map.csv")
    modify_admin1.create_new_taiwan_admin1(admin2_path, tw_admin1_map_path)
    modify_admin1.update_taiwan_admin1(admin1_path, tw_admin1_map_path, new_admin1_path)
    logger.info("modify 步驟完成。")


def cmd_pack(args):
    """產生 release 壓縮檔"""
    pack_release.pack(args.output_folder)
    logger.info("pack 步驟完成。")


def cmd_release(args):
    """依序執行所有步驟，支援跳過個別步驟"""
    output_folder = args.output_folder or "output"
    data_folder = args.data_folder or "geoname_data"
    enhanced_output = os.path.join(output_folder, "cities500_optimized.txt")

    # 1. cleanup
    if not args.pass_cleanup:
        logger.info("=== 執行 cleanup 步驟 ===")
        rebuild_folder(output_folder)
    else:
        logger.info("跳過 cleanup 步驟")

    # 2. prepare
    if not args.pass_prepare:
        logger.info("=== 執行 prepare 步驟 ===")
        prepare_geoname.download(args.country_code, args.update_prepare)
    else:
        logger.info("跳過 prepare 步驟")

    # 3. modify
    if not args.pass_modify:
        logger.info("=== 執行 modify 步驟 ===")
        admin1_path = os.path.join(data_folder, "admin1CodesASCII.txt")
        admin2_path = os.path.join(data_folder, "admin2Codes.txt")
        new_admin1_path = os.path.join(output_folder, "admin1CodesASCII_optimized.txt")
        tw_admin1_map_path = os.path.join(output_folder, "tw_admin1_map.csv")
        modify_admin1.create_new_taiwan_admin1(admin2_path, tw_admin1_map_path)
        modify_admin1.update_taiwan_admin1(
            admin1_path, tw_admin1_map_path, new_admin1_path
        )
    else:
        logger.info("跳過 modify 步驟")

    # 4. enhance
    if not args.pass_enhance:
        logger.info("=== 執行 enhance 步驟 ===")
        cities_file = os.path.join("geoname_data", "cities500.txt")
        extra_files = [
            os.path.join("geoname_data", "extra_data", f"{cc}.txt")
            for cc in args.country_code
        ]
        enhance_data.update_cities500(cities_file, extra_files, enhanced_output, 100)
    else:
        logger.info("跳過 enhance 步驟")

    # 5. locationiq
    if not args.pass_locationiq:
        logger.info("=== 執行 locationiq 步驟 ===")
        api_key = args.locationiq_api_key or os.environ.get("LOCATIONIQ_API_KEY")
        if not api_key:
            logger.critical(
                "必須提供 LocationIQ API Key (透過 --locationiq-api-key 或環境變數)"
            )
            sys.exit(1)
        qps = (
            args.locationiq_qps
            if args.locationiq_qps is not None
            else int(os.environ.get("LOCATIONIQ_QPS", "1"))
        )
        generate_geodata_locationiq.set_locationiq_config(api_key, qps)

        meta_data_folder = "./meta_data"
        os.makedirs(meta_data_folder, exist_ok=True)
        for cc in args.country_code:
            locationiq_output = os.path.join(meta_data_folder, f"{cc}.csv")
            if args.overwrite and os.path.exists(locationiq_output):
                os.remove(locationiq_output)
            generate_geodata_locationiq.process_file(
                enhanced_output, locationiq_output, cc, args.batch_size
            )
    else:
        logger.info("跳過 locationiq 步驟")

    # 6. translate
    if not args.pass_translate:
        logger.info("=== 執行 translate 步驟 ===")
        metadata_folder = "./meta_data"
        translated_cities = os.path.join(output_folder, "cities500_translated.txt")
        alternate_name_file = os.path.join(output_folder, "alternate_chinese_name.csv")
        translate.translate_cities500(
            metadata_folder, enhanced_output, translated_cities, alternate_name_file
        )
        admin1_file = os.path.join(output_folder, "admin1CodesASCII_optimized.txt")
        admin2_file = os.path.join("geoname_data", "admin2Codes.txt")
        translate.translate_admin1(admin1_file, alternate_name_file, output_folder)
        translate.translate_admin1(admin2_file, alternate_name_file, output_folder)
    else:
        logger.info("跳過 translate 步驟")

    # 7. pack (release 壓縮檔)
    if not args.pass_pack:
        logger.info("=== 執行 pack 步驟 ===")
        pack_release.pack(output_folder)
    else:
        logger.info("跳過 pack 步驟")

    logger.info("所有步驟完成！")


def main():
    parser = argparse.ArgumentParser(
        description="專案主要接口：透過子命令操作不同功能模組",
        epilog="請使用各子命令來執行特定功能。",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    # cleanup 子命令
    parser_cleanup = subparsers.add_parser("cleanup", help="清理舊資料")
    parser_cleanup.add_argument(
        "--output-folder", type=str, default="./output", help="輸出資料夾"
    )
    parser_cleanup.set_defaults(func=cmd_cleanup)

    # prepare 子命令
    parser_prepare = subparsers.add_parser("prepare", help="下載並處理 geoname 資料")
    parser_prepare.add_argument(
        "--country-code",
        type=str,
        nargs="+",
        default=["TW"],
        help="國家代碼，可提供多個代碼，如: TW JP",
    )
    parser_prepare.add_argument(
        "--update", action="store_true", help="刪除現有資料並重新下載"
    )
    parser_prepare.set_defaults(func=cmd_prepare)

    # enhance 子命令
    parser_enhance = subparsers.add_parser("enhance", help="優化 cities500 資料")
    parser_enhance.add_argument(
        "--cities-file", type=str, help="原始 cities500.txt 路徑"
    )
    parser_enhance.add_argument(
        "--country-code",
        type=str,
        nargs="+",
        default=["TW"],
        help="國家代碼，可提供多個代碼，如: TW JP",
    )
    parser_enhance.add_argument("--output-file", type=str, help="輸出優化後的檔案路徑")
    parser_enhance.add_argument(
        "--min-population", type=int, default=100, help="最小人口數"
    )
    parser_enhance.set_defaults(func=cmd_enhance)

    # locationiq 子命令
    parser_locationiq = subparsers.add_parser(
        "locationiq", help="使用 LocationIQ 取得 metadata"
    )
    parser_locationiq.add_argument(
        "--country-code",
        type=str,
        nargs="+",
        default=["TW"],
        help="國家代碼，可提供多個代碼，如: TW JP",
    )
    parser_locationiq.add_argument(
        "--output-folder", type=str, default="./", help="輸出資料夾"
    )
    parser_locationiq.add_argument(
        "--overwrite", action="store_true", help="覆蓋已存在的資料"
    )
    parser_locationiq.add_argument(
        "--batch-size", type=int, default=100, help="每批次寫入數量"
    )
    parser_locationiq.add_argument(
        "--locationiq-api-key", type=str, help="LocationIQ API Key"
    )
    parser_locationiq.add_argument(
        "--locationiq-qps", type=int, default=1, help="LocationIQ 每秒查詢次數限制"
    )
    parser_locationiq.set_defaults(func=cmd_locationiq)

    # translate 子命令
    parser_translate = subparsers.add_parser("translate", help="翻譯地名資料")
    parser_translate.add_argument(
        "--source-folder", type=str, default="./geoname_data", help="原始資料夾"
    )
    parser_translate.add_argument(
        "--output-folder", type=str, default="./output", help="輸出資料夾"
    )
    parser_translate.add_argument(
        "--alternate-name-file", type=str, help="替代名稱檔案路徑"
    )
    parser_translate.set_defaults(func=cmd_translate)

    # modify 子命令
    parser_modify = subparsers.add_parser("modify", help="修改臺灣行政區代碼")
    parser_modify.add_argument(
        "--data-folder", type=str, default="./geoname_data", help="原始資料夾"
    )
    parser_modify.add_argument(
        "--output-folder", type=str, default="./output", help="輸出資料夾"
    )
    parser_modify.set_defaults(func=cmd_modify)

    # pack 子命令
    parser_pack = subparsers.add_parser("pack", help="產生 release 壓縮檔")
    parser_pack.add_argument(
        "--output-folder", type=str, default="./output", help="輸出資料夾"
    )
    parser_pack.set_defaults(func=cmd_pack)

    # release 子命令 (依序執行所有步驟，可設定跳過部份步驟)
    parser_release = subparsers.add_parser("release", help="依序執行所有步驟")
    parser_release.add_argument(
        "--update-prepare", action="store_true", help="prepare 時重新下載資料"
    )
    parser_release.add_argument(
        "--data-folder", type=str, default="./geoname_data", help="原始資料夾"
    )
    parser_release.add_argument(
        "--output-folder", type=str, default="./output", help="輸出資料夾"
    )
    parser_release.add_argument(
        "--country-code",
        type=str,
        nargs="+",
        default=["TW"],
        help="國家代碼，可提供多個代碼，如: TW JP",
    )
    parser_release.add_argument(
        "--overwrite", action="store_true", help="覆蓋已存在的資料"
    )
    parser_release.add_argument(
        "--batch-size", type=int, default=100, help="每批次寫入數量"
    )
    parser_release.add_argument(
        "--locationiq-api-key", type=str, help="LocationIQ API Key"
    )
    parser_release.add_argument(
        "--locationiq-qps", type=int, default=1, help="LocationIQ 每秒查詢次數限制"
    )
    parser_release.add_argument(
        "--pass-cleanup", action="store_true", help="跳過 cleanup 步驟"
    )
    parser_release.add_argument(
        "--pass-prepare", action="store_true", help="跳過前置處理"
    )
    parser_release.add_argument(
        "--pass-modify", action="store_true", help="跳過 modify 步驟"
    )
    parser_release.add_argument(
        "--pass-enhance", action="store_true", help="跳過 enhance 步驟"
    )
    parser_release.add_argument(
        "--pass-locationiq", action="store_true", help="跳過 locationiq 步驟"
    )
    parser_release.add_argument(
        "--pass-translate", action="store_true", help="跳過 translate 步驟"
    )
    parser_release.add_argument(
        "--pass-pack", action="store_true", help="跳過 pack 步驟"
    )
    parser_release.set_defaults(func=cmd_release)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
