import type { AppLocale } from "../../i18n";

const en = {
  label: "nhentai",
  searchPages: "Pages: {count}",
  searchFavorites: "Favorites: {count}",
  fieldsFavorites: "Favorites",
  fieldsUploadedAt: "Uploaded",
  fieldsScanlator: "Scanlator",
  tagGroupsLanguage: "Languages",
  tagGroupsArtist: "Artists",
  tagGroupsTag: "Tags",
  tagGroupsParody: "Parodies",
  tagGroupsCharacter: "Characters",
  tagGroupsGroup: "Groups",
  tagGroupsCategory: "Categories",
  tagGroupsOther: "Other",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "nhentai",
  searchPages: "页数：{count}",
  searchFavorites: "收藏：{count}",
  fieldsFavorites: "收藏",
  fieldsUploadedAt: "上传日期",
  fieldsScanlator: "汉化组",
  tagGroupsLanguage: "语言",
  tagGroupsArtist: "画师",
  tagGroupsTag: "标签",
  tagGroupsParody: "原作",
  tagGroupsCharacter: "角色",
  tagGroupsGroup: "社团",
  tagGroupsCategory: "分类",
  tagGroupsOther: "其他",
};

export const nhentaiMessages: Record<
  AppLocale,
  Record<keyof typeof en, string>
> = {
  en,
  "zh-CN": zhCN,
};
