import type { AppLocale } from "../../i18n";

const en = {
  label: "Manhuagui",
  authors: "Authors: {names}",
  latestChapter: "Latest: {chapter}",
  updatedAt: "Updated: {date}",
  statusOngoing: "Ongoing",
  statusCompleted: "Completed",
  score: "★ {score}",
  ratings: "{count} ratings",
  chapterGroupsAll: "All chapters",
  chapterGroupsOther: "Other {index}",
  chapterGroupsCount: "{count} chapters",
  chapterGroupsExpandAll: "Expand all",
  chapterGroupsCollapseAll: "Collapse all",
  fieldsLatestChapter: "Latest chapter",
  fieldsUpdatedAt: "Updated",
  fieldsYear: "Year",
  fieldsRegion: "Region",
  sectionsCategories: "Categories",
  sectionsAliases: "Also known as",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "漫画柜",
  authors: "作者：{names}",
  latestChapter: "更新至 {chapter}",
  updatedAt: "更新于 {date}",
  statusOngoing: "连载中",
  statusCompleted: "已完结",
  score: "★ {score}",
  ratings: "{count} 人评分",
  chapterGroupsAll: "全部章节",
  chapterGroupsOther: "其他 {index}",
  chapterGroupsCount: "{count} 章",
  chapterGroupsExpandAll: "展开全部",
  chapterGroupsCollapseAll: "收起全部",
  fieldsLatestChapter: "最新话",
  fieldsUpdatedAt: "更新时间",
  fieldsYear: "年份",
  fieldsRegion: "地区",
  sectionsCategories: "分类",
  sectionsAliases: "别名",
};

export const manhuaguiMessages: Record<
  AppLocale,
  Record<keyof typeof en, string>
> = {
  en,
  "zh-CN": zhCN,
};
