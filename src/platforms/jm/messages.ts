import type { AppLocale } from "../../i18n";

const en = {
  label: "JM",
  noTags: "No tags",
  moreTags: "+{count}",
  fieldsPublishedAt: "Published",
  fieldsUpdatedOn: "Updated",
  fieldsLikes: "Likes",
  fieldsViews: "Views",
  fieldsComments: "Comments",
  sectionsWorks: "Works",
  sectionsActors: "Characters",
  sectionsTags: "Tags",
} as const;

const zhCN: Record<keyof typeof en, string> = {
  label: "JM",
  noTags: "暂无标签",
  moreTags: "+{count}",
  fieldsPublishedAt: "上架日期",
  fieldsUpdatedOn: "更新日期",
  fieldsLikes: "点赞",
  fieldsViews: "观看",
  fieldsComments: "评论",
  sectionsWorks: "作品",
  sectionsActors: "角色",
  sectionsTags: "标签",
};

export const jmMessages: Record<AppLocale, Record<keyof typeof en, string>> = {
  en,
  "zh-CN": zhCN,
};
