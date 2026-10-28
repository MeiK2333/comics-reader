import { definePlatform } from "../types";
import icon from "./icon.png";
import { manwaMessages } from "./messages";

export const manwa = definePlatform<"manwa">({
  id: "manwa",
  messages: manwaMessages,
  icon,
  iconStyle: "plain",
  singleChapter: false,
  numericIdDirectOpen: true,
});
