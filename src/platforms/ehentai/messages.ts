import type { AppLocale } from "../../i18n";

const en = {
  label: "E-Hentai",
  searchPages: "Pages: {count}",
  fieldsCategory: "Category",
  fieldsLanguage: "Language",
  fieldsRating: "Rating",
  fieldsUploader: "Uploader",
  fieldsPosted: "Posted",
  fieldsFileSize: "File size",
  expunged: "Expunged",
  tagGroupsLanguage: "Languages",
  tagGroupsArtist: "Artists",
  tagGroupsCharacter: "Characters",
  tagGroupsParody: "Parodies",
  tagGroupsGroup: "Groups",
  tagGroupsFemale: "Female",
  tagGroupsMale: "Male",
  tagGroupsMixed: "Mixed",
  tagGroupsOther: "Other",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "E-Hentai",
  searchPages: "页数：{count}",
  fieldsCategory: "分类",
  fieldsLanguage: "语言",
  fieldsRating: "评分",
  fieldsUploader: "上传者",
  fieldsPosted: "发布时间",
  fieldsFileSize: "文件大小",
  expunged: "已删除",
  tagGroupsLanguage: "语言",
  tagGroupsArtist: "画师",
  tagGroupsCharacter: "角色",
  tagGroupsParody: "原作",
  tagGroupsGroup: "社团",
  tagGroupsFemale: "女性",
  tagGroupsMale: "男性",
  tagGroupsMixed: "混合",
  tagGroupsOther: "其他",
};

export const ehentaiMessages: Record<
  AppLocale,
  Record<keyof typeof en, string>
> = {
  en,
  "zh-CN": zhCN,
};
