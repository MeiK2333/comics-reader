# 路线图

## 已支持

- nhentai
- JM
- manhuagui

## 计划支持

以下平台尚未实现，按“先确认接口与抓取许可，再补搜索/详情/阅读页源与 fixture”的顺序推进：

- [包子漫画](https://www.baozimh.com/)
- [拷贝漫画](https://www.mangacopy.com/)
- [manhuabika](https://manhuabika.com/)
- [E-Hentai](https://e-hentai.org/)

## 其他计划

- JM 域名自动更新：从发布页获取最新域名，替换内置列表。
- 按章下载与导出：在现有按作品下载（存储模型已按章组织）之上支持按章勾选、CBZ/文件夹导出。

## 说明

- 这里是方向性计划，不承诺版本与时间；实际顺序取决于平台接口稳定性、登录/地区限制与抓取许可。
- 新平台按 [`src/platforms/README.md`](src/platforms/README.md) 与 [`src-tauri/src/platforms/README.md`](src-tauri/src/platforms/README.md) 的两目录流程实现，保持 `ImageProfile`/`Catalog` trait、判别联合与缓存版本约定。
