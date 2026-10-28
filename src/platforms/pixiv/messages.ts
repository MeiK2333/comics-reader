import type { AppLocale } from "../../i18n";

const en = {
  label: "pixiv",
  searchBy: "by {name}",
  searchPages: "Pages: {count}",
  typeIllust: "Illustration",
  typeManga: "Manga",
  typeUgoira: "Ugoira",
  tagGroup: "Tags",
  fieldsType: "Type",
  fieldsSize: "Size",
  fieldsBookmarks: "Bookmarks",
  fieldsLikes: "Likes",
  fieldsViews: "Views",
  fieldsPosted: "Posted",
  fieldsSeries: "Series",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "pixiv",
  searchBy: "作者：{name}",
  searchPages: "页数：{count}",
  typeIllust: "插画",
  typeManga: "漫画",
  typeUgoira: "动图",
  tagGroup: "标签",
  fieldsType: "类型",
  fieldsSize: "尺寸",
  fieldsBookmarks: "收藏",
  fieldsLikes: "喜欢",
  fieldsViews: "浏览",
  fieldsPosted: "发布时间",
  fieldsSeries: "系列",
};

export const pixivMessages: Record<
  AppLocale,
  Record<keyof typeof en, string>
> = {
  en,
  "zh-CN": zhCN,
};
