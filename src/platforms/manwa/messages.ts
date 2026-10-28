import type { AppLocale } from "../../i18n";

const en = {
  label: "Manwa",
  authors: "Authors: {names}",
  statusOngoing: "Ongoing",
  statusCompleted: "Completed",
  fieldsLatestChapter: "Latest chapter",
  fieldsUpdatedAt: "Updated",
  fieldsRegion: "Region",
  fieldsSubscribers: "Subscribers",
  sectionsCategories: "Categories",
  sectionsTags: "Tags",
  sectionsAliases: "Also known as",
  chapterGroupsAll: "All chapters",
  chapterGroupsCount: "{count} chapters",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "漫蛙",
  authors: "作者：{names}",
  statusOngoing: "连载中",
  statusCompleted: "已完结",
  fieldsLatestChapter: "最新话",
  fieldsUpdatedAt: "更新时间",
  fieldsRegion: "地区",
  fieldsSubscribers: "订阅数",
  sectionsCategories: "类别",
  sectionsTags: "标签",
  sectionsAliases: "别名",
  chapterGroupsAll: "全部章节",
  chapterGroupsCount: "{count} 章",
};

export const manwaMessages: Record<
  AppLocale,
  Record<keyof typeof en, string>
> = {
  en,
  "zh-CN": zhCN,
};
