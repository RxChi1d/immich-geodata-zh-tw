# Immich 反向地理编码汉化

这是一个为 Immich 的反向地理位编码功能提供汉化支持的项目，主要解决 Immich 识别照片拍摄位置都是英文的问题，并对结果进行了优化

- 对国内的地点进行了完整的汉化，国外的地点汉化也进行了实验性支持（目前仅尝试了日本）
- 利用 高德/LocationIQ API 重新将结果标准化为 **国家、省份、城市** 三个等级，避免 Immich 原本数据中不常用的称呼
- 定期拉取最新数据并自动更新发布

以下是使用后的前后对比

![](./image/example.png)

# 如何使用

1. 在 [Release](https://github.com/ZingLix/immich-geodata-cn/releases) 中下载 geodata.zip 和 i18n-iso-countries.zip 两个文件并解压
2. 调整你的 docker-compose.yaml，volumes 中增加如下两行（或者根据不同部署方式任意方式替换掉这两个文件夹）

```
volumes:
  - ./geodata:/build/geodata
  - ./i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs
```

3. 运行 `docker compose restart` 重启 Immich
  - 可以检查一下日志，启动时候会出现 `10000 geodata records imported` 类似的日志，这表明 geodata 更新了
  - 如果没有更新，可以尝试修改 geodata/geodata-date.txt，修改成一个更新的时间，如果旧于 Immich 曾经加载过的时间 Immich 就不会更新
    ![](./image/importlog.jpg)
4. 启动完成后登录你的 Immich 管理后台，在 `系统管理-任务` 中 `提取元数据` 点击 `全部`，以触发所有照片的元数据刷新，等待任务完成后，所有照片的位置信息就都会显示成中文，后续新增的图片则无需任何额外操作，并且可以用中文进行搜索了

# License

GPL