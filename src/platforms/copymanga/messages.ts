import type { AppLocale } from "../../i18n";

const en = {
  label: "CopyManga",
  authors: "Authors: {names}",
  aliases: "Also known as: {names}",
  popularity: "Popularity {count}",
  statusOngoing: "Ongoing",
  statusCompleted: "Completed",
  fieldsLatestChapter: "Latest chapter",
  fieldsUpdatedAt: "Updated",
  fieldsRegion: "Region",
  fieldsPopularity: "Popularity",
  chapterGroupsAll: "All chapters",
  chapterGroupsOther: "Other {index}",
  chapterGroupsCount: "{count} chapters",
  chapterGroupsExpandAll: "Expand all",
  chapterGroupsCollapseAll: "Collapse all",
  groupDefault: "Default",
  sectionsCategories: "Categories",
  sectionsAliases: "Also known as",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "拷贝漫画",
  authors: "作者：{names}",
  aliases: "别名：{names}",
  popularity: "热度 {count}",
  statusOngoing: "连载中",
  statusCompleted: "已完结",
  fieldsLatestChapter: "最新话",
  fieldsUpdatedAt: "更新时间",
  fieldsRegion: "地区",
  fieldsPopularity: "热度",
  chapterGroupsAll: "全部章节",
  chapterGroupsOther: "其他 {index}",
  chapterGroupsCount: "{count} 章",
  chapterGroupsExpandAll: "展开全部",
  chapterGroupsCollapseAll: "收起全部",
  groupDefault: "默认",
  sectionsCategories: "分类",
  sectionsAliases: "别名",
};

export const copymangaMessages: Record<
  AppLocale,
  Record<keyof typeof en, string>
> = {
  en,
  "zh-CN": zhCN,
};
